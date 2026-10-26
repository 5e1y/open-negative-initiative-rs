//! The Dust & Scratches mark plays its own demo on its card; the Info sidebar's Film Stock mark, a note no
//! wordless clip can show, opens its card with no clip box.

use oni_app::app::{Options, Shell};
use oni_app::{PanelState, Tip};
use oni_core::{Archive, EditorStore};
use oni_settings::Film;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, LogicalPosition};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::common;

const DUST: &str = "Paint over a defect; the tool duplicates a matching patch of the same photo over it.";
const FILM_STOCK: &str = "What this roll was shot on — never guessed, only ever what a hand picks.";

/// Sweeps `width` points right of `left`, from `from` down to `to`, until the card saying `note` opens.
fn hover_note(rig: &common::Rig, tip: &Tip, (left, width): (f32, f32), (from, to): (f32, f32), note: &str) -> bool {
    let mut y = from;
    while y < to {
        let mut x = left;
        while x < left + width {
            rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
            rig.pump();
            if tip.get_shown() && tip.get_card() && tip.get_text() == note {
                return true;
            }
            x += 3.0;
        }
        y += 3.0;
    }
    false
}

fn clip_pixels(tip: &Tip) -> Option<Vec<u8>> {
    tip.get_clip().to_rgba8().map(|b| b.as_bytes().to_vec())
}

pub(crate) fn the_dust_mark_plays_its_demo_and_the_film_stock_mark_shows_no_clip() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the help demo is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-help-demo-details-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    common::plain_tiff(&folder.join("A.tif"), 96, 64, 0.6);
    oni_library::folders::set_roll(Some(&Film::new("Roll 7")), &oni_library::paths::canonical(&folder)).unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, tip) = (w.global::<oni_app::Library>(), w.global::<Tip>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());

    // The gallery's Info sidebar, right of the window, on the roll a selected tile stands on.
    lib.invoke_tile_clicked(0, false, false);
    rig.settle();
    assert_eq!(lib.get_info_kind(), 1, "the selection stands on a roll");
    assert!(hover_note(&rig, &tip, (990.0, 290.0), (0.0, 420.0), FILM_STOCK), "the Film Stock mark opens its card");
    rig.pump();
    assert_eq!(tip.get_demo(), "", "the mark names no topic");
    assert!(!tip.get_has_clip() && clip_pixels(&tip).is_none(), "a note without a demo has no clip box");
    let without = w.get_tip_box();

    // The editor's De-dust tab, by a double click: the Dust & Scratches mark's card plays its demo.
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    w.global::<PanelState>().set_tab(4);
    w.global::<PanelState>().invoke_tab_picked(4);
    rig.settle();
    let (left, foot) = (w.get_panel_x() + 4.0, w.get_tabs_foot() + 2.0);
    assert!(hover_note(&rig, &tip, (left, 280.0), (foot, 790.0), DUST), "the Dust & Scratches mark opens its card");
    assert_eq!(tip.get_demo(), "dust", "the mark names its topic");
    assert!(tip.get_has_clip(), "the card holds a clip box");
    rig.until("the demo's first frame", Duration::from_secs(5), || clip_pixels(&tip).is_some());
    let first = clip_pixels(&tip).unwrap();
    // The hand reaches the speck within a second and paints: past that, the picture has changed.
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(1500) {
        rig.pump();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_ne!(first, clip_pixels(&tip).expect("still playing"), "the demo's frame advances while the card is open");
    let with_clip = w.get_tip_box();
    assert!(with_clip.height > with_clip.width, "the square clip stands above the note: {with_clip:?}");
    assert!(without.height < with_clip.height - with_clip.width / 2.0, "the Film Stock card had no box above its note: {without:?}");
    rig.shot("help-demo-dust");
    let _ = std::fs::remove_dir_all(&root);
}
