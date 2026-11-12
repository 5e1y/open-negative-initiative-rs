//! ⌘Z and ⌘⇧Z typed in a field step the field's typing, never the photos' history the menu bar holds;
//! with the photos holding the keyboard, they step the history.

use oni_app::app::{Options, Shell};
use oni_app::Library;
use oni_core::{Archive, EditorStore};
use slint::platform::{Key, WindowEvent};
use slint::ComponentHandle;
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

pub(crate) fn undo_in_a_field_steps_its_typing_and_leaves_the_photos_history() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: a field's undo is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-field-undo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("Scans");
    common::plain_tiff(&folder.join("A.tif"), 96, 64, 0.5);
    common::plain_tiff(&folder.join("B.tif"), 96, 64, 0.6);
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let lib = w.global::<Library>();
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    rig.settle();
    let medal = || oni_app::app::tiles(w).into_iter().find(|t| t.caption == "A").map_or(-1, |t| t.medal);
    let fields = || [lib.get_month(), lib.get_day(), lib.get_year(), lib.get_camera_body(), lib.get_description()];

    // One step in the photos' history: A labelled silver.
    rig.key("2");
    assert_eq!(medal(), 2, "the key labels the selection");
    let before = fields();
    for _ in 0..80 {
        if !w.get_keys_held() {
            break;
        }
        rig.key("\t");
    }
    assert!(!w.get_keys_focused() && !w.get_keys_held(), "Tab reaches the sidebar's first field");
    rig.key("1");
    assert_ne!(fields(), before, "the key landed in the field");

    // ⌘Z in the field takes its typing back, the selection it replaced in a step of its own; the
    // photo keeps its silver.
    rig.chord(Key::Control, "z");
    rig.chord(Key::Control, "z");
    assert_eq!(medal(), 2, "⌘Z typed in a field leaves the photos' history");
    assert_eq!(fields(), before, "the field's own undo took the typing back");

    // Twin: the photos holding the keyboard, ⌘Z steps the history and ⌘⇧Z steps it again.
    w.invoke_refocus();
    rig.pump();
    rig.chord(Key::Control, "z");
    assert_eq!(medal(), 0, "with the photos holding the keyboard, ⌘Z undoes the silver");
    rig.chord_with(&[Key::Control, Key::Shift], "z");
    assert_eq!(medal(), 2, "and ⌘⇧Z redoes it");
    let _ = std::fs::remove_dir_all(&root);
}
