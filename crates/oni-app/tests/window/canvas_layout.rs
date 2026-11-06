//! Where the shell lays its pieces, as Swift does: the card flush, a hidden Library back by its key or
//! edge, the bar's empty room, roll headers' trail and pin, the editor's medals, an undecodable file.

use oni_app::app::{self, Options, Shell};
use oni_app::{App, Bar, Canvas, Library};
use oni_core::{Archive, EditorStore};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

const WIDTH: f32 = 1280.0;
const HEIGHT: f32 = 800.0;
/// The headless window draws the menu bar in its first points, then the 56-point bar (40 pt keys).
const MENU: f32 = 40.0;
const CARD_TOP: f32 = MENU + 56.0;

fn click(rig: &common::Rig, x: f32, y: f32) {
    let position = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
    rig.pump();
}

fn hover(rig: &common::Rig, x: f32, y: f32) {
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
    rig.settle();
}

fn open(rig: &common::Rig, w: &oni_app::MainWindow, caption: &str) {
    let tile = app::tiles(w).into_iter().find(|t| t.caption == caption).unwrap_or_else(|| panic!("{caption} is shown"));
    w.global::<Library>().invoke_tile_opened(tile.index);
    rig.pump();
}

pub(crate) fn the_shell_lays_its_pieces_where_swift_does() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the shell's layout is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-canvas-layout-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for i in 0..72 {
        common::plain_tiff(&folder.join("Roll A").join(format!("A{i:02}.tif")), 48, 32, 0.3 + i as f32 / 200.0);
    }
    common::plain_tiff(&folder.join("Roll B").join("B00.tif"), 48, 32, 0.5);
    std::fs::write(folder.join("Roll B").join("Bad.tif"), b"II*\0not a picture").unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(WIDTH as u32, HEIGHT as u32));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, app, canvas, bar) = (w.global::<Library>(), w.global::<App>(), w.global::<Canvas>(), w.global::<Bar>());
    let room = std::rc::Rc::new(std::cell::Cell::new(0));
    let pressed = room.clone();
    bar.on_room_pressed(move || pressed.set(pressed.get() + 1));
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();

    // `RollHeaderContent`: the folders above the roll in secondary, then its name; no count.
    let headings: Vec<_> = lib.get_headings().iter().collect();
    let a = headings.iter().find(|h| h.title == "Roll A").expect("Roll A heads its band");
    assert_eq!(a.caption, "Scans", "the trail, the folders above the roll");
    assert!(headings.iter().all(|h| !h.caption.contains("photo")), "twin: no band counts its photos");

    // The band's header is held at the grid's top while its band scrolls, gone back in place.
    assert!(!lib.get_pinned_shown(), "at the top the header shows in place");
    lib.invoke_scrolled(200.0);
    rig.pump();
    assert!(lib.get_pinned_shown(), "scrolled into Roll A, its header is held");
    assert_eq!((lib.get_pinned_title().as_str(), lib.get_pinned_caption().as_str()), ("Roll A", "Scans"));
    assert_eq!(lib.get_pinned_y(), lib.get_scroll(), "held at the grid's top edge");
    rig.shot("layout-pinned-header");
    lib.invoke_scrolled(-200.0);
    rig.pump();
    assert!(!lib.get_pinned_shown(), "twin: back at the top, nothing is held");

    // The bar's empty room stands for the title bar; a key takes its own press.
    click(&rig, 200.0, MENU + 20.0);
    assert_eq!(room.get(), 1, "a press between the keys reaches the room");

    open(&rig, w, "A00");
    rig.until("A00 rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    rig.settle();
    assert!(!app.get_gallery());
    click(&rig, 40.0, MENU + 20.0);
    assert!(app.get_gallery(), "the GALLERY key took its press");
    assert_eq!(room.get(), 1, "twin: a key's press never reaches the room");
    open(&rig, w, "A00");
    rig.settle();

    rig.shot("layout-editor");
    // `zoomBubble`: its keys centred in the band over the image, clear of the bar above.
    let (pw, px) = rig.drawn();
    let x = (WIDTH - 340.0 - 40.0 - 12.0) as usize;
    let ground = px[(CARD_TOP as usize + 1) * pw + x].0;
    let key: Vec<usize> = (CARD_TOP as usize..CARD_TOP as usize + 40).filter(|&y| px[y * pw + x].0 != ground).collect();
    let (above, below) = (key[0] - CARD_TOP as usize, CARD_TOP as usize + 40 - 1 - key[key.len() - 1]);
    assert!(above.abs_diff(below) <= 1, "the panel key centred in the band: {above} above, {below} below");
    assert!(above >= 6, "twin: not hung at the band's top edge: {above}");
    // `card`: flush with the library and the panel, so the canvas is the column less its padding.
    let flush = WIDTH - 260.0 - 340.0 - 2.0 * 40.0;
    assert_eq!(canvas.get_width(), flush, "the card spans the column, no gutter");
    assert_ne!(canvas.get_width(), flush - 2.0 * 8.0, "twin: the old gutters are gone");

    // `statusMedalTarget` beside a photo: the picker shows for the open frame.
    assert!(lib.get_editor_medal_shown(), "the editor's status bar carries the medal picker");

    // The canvas's leading key hides the Library; the same key, now at the card's edge, brings it back.
    let tab_y = CARD_TOP + 8.0 + 4.0;
    click(&rig, 260.0 + 40.0 + 12.0, CARD_TOP + 20.0);
    assert!(app.get_library_hidden(), "the column key hid the Library");
    rig.settle();
    let hidden_width = canvas.get_width();
    assert_eq!(hidden_width, flush + 260.0, "the card takes the Library's room");

    // The hidden column slides over the card from its edge while hovered.
    let surface = lib.get_surface();
    click(&rig, 8.0 + 200.0, tab_y + 8.0);
    assert_eq!(lib.get_surface(), surface, "twin: with nothing revealed the tabs are not there");
    hover(&rig, 4.0, 400.0);
    rig.shot("layout-revealed-library");
    click(&rig, 8.0 + 200.0, tab_y + 8.0);
    assert_ne!(lib.get_surface(), surface, "the revealed column's Projects tab took the click");
    lib.invoke_surface_picked(surface);
    hover(&rig, 600.0, 400.0);

    // The gallery lists the library whatever the editor hid.
    ctl.act(oni_app::Act::ToggleMode);
    rig.settle();
    assert!(app.get_gallery() && app.get_library_hidden());
    rig.shot("layout-gallery-library-hidden");
    click(&rig, 200.0, tab_y + 8.0);
    assert_ne!(lib.get_surface(), surface, "the gallery's column is there to take the click");
    lib.invoke_surface_picked(surface);
    rig.pump();

    // Back beside the photo, the column key brings the Library back.
    open(&rig, w, "A00");
    rig.settle();
    click(&rig, 40.0 + 12.0, CARD_TOP + 20.0);
    assert!(!app.get_library_hidden(), "the key shows the Library again");

    // A file that never decodes says so on the canvas; a fine one says nothing.
    ctl.act(oni_app::Act::ToggleMode);
    rig.settle();
    open(&rig, w, "Bad");
    rig.until("Bad refused", Duration::from_secs(60), || !canvas.get_failure().is_empty());
    assert_eq!(canvas.get_failure(), "Bad.tif: could not be decoded");
    rig.shot("layout-undecodable");
    ctl.act(oni_app::Act::ToggleMode);
    rig.settle();
    open(&rig, w, "B00");
    assert_eq!(canvas.get_failure(), "", "twin: a fine frame clears the message");
    let _ = std::fs::remove_dir_all(&root);
}
