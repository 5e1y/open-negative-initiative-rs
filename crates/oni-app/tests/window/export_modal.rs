//! The export surface covers the editor and the gallery: no key, menu item or roll asked for before it
//! reaches what it hides, Escape leaves it, and once it is gone the same keys act again.

use oni_app::app::{Options, Shell, export_card};
use oni_app::{Act, App, Bar, Framing, Library, Menus, Preparing};
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
use slint::platform::{Key, WindowEvent};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::common;

const RIGHT: &str = "\u{F703}";
const ESCAPE: &str = "\u{1b}";

pub(crate) fn the_export_surface_lets_nothing_reach_what_it_covers() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the export surface's cover is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-export-modal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 0.8), ("B.tif", 0.6), ("C.tif", 0.4)] {
        common::graded_tiff(&folder.join(name), 300, 200, tone);
    }
    let names: Vec<String> = ["A.tif", "B.tif", "C.tif"].iter().map(|n| oni_library::paths::canonical(&folder.join(n))).collect();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (app, lib, menus, framing, preparing) = (w.global::<App>(), w.global::<Library>(), w.global::<Menus>(), w.global::<Framing>(), w.global::<Preparing>());
    let shown = || export_card::card(&ctl).shown;
    let open_path = || ctl.state.borrow().open_path().map(str::to_string);
    let picked = || ctl.state.borrow().picked().clone();
    let settings = || ctl.state.borrow().store().settings().clone();
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    rig.pump();
    // A mirrored, so ⌘Z has a step to take back; A opens with its measuring window armed.
    rig.chord_with(&[Key::Control, Key::Shift], "m");
    assert!(settings().geometry.mirrored && framing.get_armed() && menus.get_live().undo);

    // Beside the photo: the surface shown, every gesture on the editor goes dark and its keys act on nothing.
    ctl.act(Act::Export);
    assert!(shown());
    let live = menus.get_live();
    let behind = [live.undo, live.redo, live.reset, live.apply_preset, live.paste_settings, live.mirror, live.measure_roll, live.toggle_mode];
    let more = [live.medal, live.copy_settings, live.save_preset, live.framing, live.correction, live.compare, live.larger];
    assert!(behind.iter().chain(more.iter()).all(|on| !on), "nothing reaches the editor the surface covers: {live:?}");
    assert!(live.export && live.screen_gamut && !w.global::<Bar>().get_can_prepare());
    let (kept, path, chosen) = (settings(), open_path(), picked());
    rig.key(RIGHT);
    rig.key("\r");
    rig.key(" ");
    rig.chord(Key::Control, "a");
    rig.chord(Key::Control, "z");
    rig.chord_with(&[Key::Control, Key::Shift], "m");
    rig.chord_with(&[Key::Control, Key::Shift], "r");
    rig.settle();
    assert_eq!(open_path(), path, "the arrow walks to no other frame");
    assert_eq!(picked(), chosen, "⌘A picks nothing");
    assert!(framing.get_armed(), "Return applies no window");
    assert!(settings() == kept, "neither ⌘Z, Mirror nor Reset moves the frame the export captured");
    assert!(shown() && !app.get_gallery(), "Space leaves for no gallery");
    rig.key(ESCAPE);
    assert!(!shown() && !app.get_gallery(), "Escape leaves the surface for the editor");
    // Twin: the surface gone, the same keys act, so they reached the window and the cover stopped them.
    assert!(menus.get_live().undo);
    rig.chord(Key::Control, "z");
    assert!(!settings().geometry.mirrored, "⌘Z takes the mirror back");
    rig.key("\r");
    assert!(!framing.get_armed(), "Return applies the window");
    rig.key(RIGHT);
    rig.until("B open", Duration::from_secs(60), || open_path().as_deref() == Some(names[1].as_str()));

    // In the gallery: the keys open nothing, and a roll asked for before the surface lands under none.
    ctl.act(Act::ToggleMode);
    assert!(app.get_gallery());
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    lib.invoke_tile_clicked(0, false, false);
    rig.pump();
    let chosen = picked();
    assert_eq!(chosen, HashSet::from([names[0].clone()]));
    ctl.act(Act::PrepareRoll);
    ctl.act(Act::Export);
    assert!(shown());
    for key in ["\r", " ", RIGHT] {
        rig.key(key);
    }
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(1500) {
        rig.pump();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!preparing.get_shown() && shown(), "the roll asked for lands under no surface");
    assert!(app.get_gallery() && picked() == chosen, "Return, Space and the arrow open and move nothing");
    rig.key(ESCAPE);
    assert!(!shown() && app.get_gallery());
    // Twin: the roll asked for alone comes within the same wait.
    let start = Instant::now();
    ctl.act(Act::PrepareRoll);
    rig.until("the roll's surface", Duration::from_millis(1500), || preparing.get_shown());
    println!("the roll came in {:.0} ms", start.elapsed().as_secs_f64() * 1e3);
    ctl.act(Act::LeaveRoll);
    rig.until("the roll left", Duration::from_secs(10), || !preparing.get_shown());
    rig.key("\r");
    assert!(!app.get_gallery(), "twin: Return on the grid opens the editor once the surface is gone");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
