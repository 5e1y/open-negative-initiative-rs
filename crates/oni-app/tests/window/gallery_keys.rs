//! The gallery's keyboard through the window's real event path, its pointer hook first: a click on a
//! folder's row gives the rows the keys unringed and a key rings them, a photo's own press hands the
//! arrows and Return to the photos, and ⌘ and ⇧ gather photos in the filmstrip beside a photo.

use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use oni_app::app::{Options, Shell};
use oni_app::{App, Library, Navigator};
use oni_core::{Archive, EditorStore};
use oni_library::navigator::Computer;
use oni_library::paths;
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model, SharedString};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// A click as winit hands it over, ⌘ or ⇧ held or none; unhooked, Slint alone sees it, as a tool
/// dispatching events straight to Slint does.
fn click(rig: &common::Rig, w: &oni_app::MainWindow, (x, y): (f32, f32), held: Option<Key>, hooked: bool) {
    let held: Option<SharedString> = held.map(Into::into);
    if let Some(k) = &held {
        rig.window.dispatch_event(WindowEvent::KeyPressed { text: k.clone() });
    }
    let position = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    if hooked {
        oni_app::outside::pressed(w);
    }
    rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
    if let Some(k) = held {
        rig.window.dispatch_event(WindowEvent::KeyReleased { text: k });
    }
    rig.pump();
}

/// A key as winit hands it over: the window's key hook first, then Slint.
fn key(rig: &common::Rig, w: &oni_app::MainWindow, k: Key) {
    oni_app::outside::keyed(w.window(), || w.invoke_refocus());
    rig.key(&SharedString::from(k));
}

/// Past the double-click interval, so the next click is a first one.
fn rest(rig: &common::Rig) {
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    rig.pump();
}

/// The leftmost element so named, as the library column lies left of the card.
fn leftmost(w: &oni_app::MainWindow, role: AccessibleRole, label: &str) -> (f32, f32) {
    let found = common::placed(w, role, label);
    found.iter().min_by(|a, b| a.x.total_cmp(&b.x)).unwrap_or_else(|| panic!("{label} is shown")).middle()
}

fn tile(w: &oni_app::MainWindow, caption: &str) -> (f32, f32) {
    let lib = w.global::<Library>();
    let t = oni_app::app::tiles(w).into_iter().find(|t| t.caption == caption).unwrap_or_else(|| panic!("{caption} is a tile"));
    (lib.get_grid_x() + t.x + t.width / 2.0, lib.get_grid_y() + t.y - lib.get_scroll() + t.height / 2.0)
}

fn selected(w: &oni_app::MainWindow) -> Vec<String> {
    let mut tiles = oni_app::app::tiles(w);
    tiles.sort_by_key(|t| t.index);
    tiles.into_iter().filter(|t| t.selected).map(|t| t.caption.to_string()).collect()
}

/// The navigator's picked row.
fn picked(w: &oni_app::MainWindow) -> Option<String> {
    w.global::<Navigator>().get_rows().iter().find(|r| r.picked).map(|r| r.title.to_string())
}

/// The filmstrip's photos lit, by name.
fn marked(w: &oni_app::MainWindow) -> Vec<String> {
    let mut lit: Vec<String> = w.global::<Library>().get_rows().iter().filter(|r| r.kind == 2 && r.selected).map(|r| r.title.to_string()).collect();
    lit.sort();
    lit
}

fn rings(w: &oni_app::MainWindow) -> usize {
    ElementHandle::find_by_element_type_name(w, "FocusRing").count()
}

pub(crate) fn a_photo_clicked_hands_the_grid_its_keys_and_a_ring_follows_the_keys_alone() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the gallery's keys are not exercised"),
    };
    let made = std::env::temp_dir().join(format!("oni-app-gallery-keys-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&made);
    for (file, tone) in [("Roll 1/A.tif", 1.0), ("Roll 1/B.tif", 0.8), ("Roll 1/C.tif", 0.6), ("Roll 1/D.tif", 0.4), ("Roll 2/E.tif", 0.5)] {
        common::linear_tiff(&made.join("Home/Scans").join(file), 48, 32, tone);
    }
    std::fs::create_dir_all(made.join("Volumes")).unwrap();
    let root = PathBuf::from(paths::canonical(&made));
    let computer = Computer { home: paths::canonical(&root.join("Home")), volumes: paths::canonical(&root.join("Volumes")) };
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), computer, ..Options::default() };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let opened = || ctl.state.borrow().opened().map(str::to_owned);
    shell.add(&root.join("Home/Scans/Roll 1"));
    rig.until("Roll 1 read", Duration::from_secs(30), || ctl.state.borrow().read_done() && opened().is_some_and(|o| o.ends_with("Roll 1")));
    rig.until("both rolls among the rows", Duration::from_secs(10), || {
        let rows = w.global::<Navigator>().get_rows();
        ["Roll 1", "Roll 2"].iter().all(|t| rows.iter().any(|r| r.title == *t))
    });
    rig.until("the four photos shown", Duration::from_secs(10), || oni_app::app::tiles(w).len() == 4);
    rig.settle();

    // A hand's click gives the rows the keyboard with no ring; an arrow walks them and rings the pick.
    rest(&rig);
    click(&rig, w, leftmost(w, AccessibleRole::ListItem, "Roll 1"), None, true);
    rig.until("Roll 1 read again", Duration::from_secs(30), || ctl.state.borrow().read_done());
    assert_eq!(common::focused_label(w), "Folders", "the click gave the rows the keyboard");
    assert_eq!((picked(w).as_deref(), rings(w)), (Some("Roll 1"), 0), "a click rings nothing");
    key(&rig, w, Key::DownArrow);
    assert_eq!((picked(w).as_deref(), rings(w)), (Some("Roll 2"), 1), "twin: an arrow walks the rows and rings the pick");
    key(&rig, w, Key::UpArrow);
    assert_eq!(picked(w).as_deref(), Some("Roll 1"));
    rig.settle();
    println!("shot {}", rig.shot("gallery-keys-rows").display());

    // Twin: a press beside the photos that Slint alone sees, as the evaluation's tool sends it, leaves
    // the rows the arrows; the window's pointer hook or the photos' own press takes them back.
    rest(&rig);
    let before = selected(w);
    click(&rig, w, (1280.0 - 100.0, 740.0), None, false);
    rig.key(&SharedString::from(Key::DownArrow));
    assert_eq!((picked(w).as_deref(), selected(w)), (Some("Roll 2"), before), "unhooked, the rows keep the arrows");

    // A press on a photo, even one Slint alone sees, gives the photos the keyboard and rings nothing:
    // ⇧→ extends the pick, → walks the photos, Return opens the photo the keys reached.
    rest(&rig);
    click(&rig, w, tile(w, "A"), None, false);
    assert_eq!(selected(w), ["A"]);
    assert_ne!(common::focused_label(w), "Folders", "the rows no longer hold the keyboard");
    assert_eq!(rings(w), 0, "the click rings neither the rows nor the photo");
    rig.chord(Key::Shift, &SharedString::from(Key::RightArrow));
    assert_eq!(selected(w), ["A", "B"], "⇧→ extends the pick");
    rig.key(&SharedString::from(Key::RightArrow));
    assert_eq!((selected(w), picked(w).as_deref()), (vec!["C".to_string()], Some("Roll 2")), "→ walks the photos, not the rows");
    rig.settle();
    println!("shot {}", rig.shot("gallery-keys-grid").display());
    rig.key(&SharedString::from(Key::Return));
    rig.until("C in the editor", Duration::from_secs(60), || ctl.state.borrow().open_path().is_some_and(|p| p.ends_with("C.tif")));
    assert!(!w.global::<App>().get_gallery(), "Return opens the photo");

    // The filmstrip beside the photo: ⌘ adds a photo to the open one, ⇧ runs from that anchor.
    let lib = w.global::<Library>();
    lib.invoke_tree_layout_picked(1);
    rig.until("the filmstrip's thumbnails", Duration::from_secs(10), || lib.get_rows().iter().filter(|r| r.thumb).count() == 4);
    rig.settle();
    rest(&rig);
    click(&rig, w, leftmost(w, AccessibleRole::ListItem, "A"), Some(Key::Control), true);
    assert_eq!(marked(w), ["A", "C"], "⌘ adds A to the open C");
    click(&rig, w, leftmost(w, AccessibleRole::ListItem, "D"), Some(Key::Shift), true);
    assert_eq!(marked(w), ["A", "B", "C", "D"], "⇧ runs from A to D, B included");
    assert!(ctl.state.borrow().open_path().is_some_and(|p| p.ends_with("C.tif")), "neither click opens another photo");
    rig.settle();
    println!("shot {}", rig.shot("gallery-keys-filmstrip").display());
    rest(&rig);
    click(&rig, w, leftmost(w, AccessibleRole::ListItem, "B"), None, true);
    assert_eq!(marked(w), ["B"], "twin: a plain click leaves its photo alone");
    drop(shell);
    let _ = std::fs::remove_dir_all(&made);
}
