//! ⌘Z and ⌘⇧Z typed in a field step the field's typing, never the photos' history the menu bar holds;
//! with the photos holding the keyboard, they step the history. ⌘C in a field copies no settings, as with them it does.

use oni_app::Library;
use slint::platform::Key;
use slint::ComponentHandle;

use crate::common;

pub(crate) fn undo_in_a_field_steps_its_typing_and_leaves_the_photos_history() {
    let root = common::Root::new("field-undo");
    let folder = root.join("Scans");
    common::plain_tiff(&folder.join("A.tif"), 96, 64, 0.5);
    common::plain_tiff(&folder.join("B.tif"), 96, 64, 0.6);
    let Some(app) = common::launch(root).show() else { return };
    let common::App { rig, shell, .. } = &app;
    let w = &shell.window;
    let lib = w.global::<Library>();
    app.read(&folder);
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

    // ⌘C typed in a field is the field's own copy: no settings copied. Twin: the photos holding the keyboard, it copies them.
    let presets = w.global::<oni_app::Presets>();
    for _ in 0..80 {
        if !w.get_keys_held() {
            break;
        }
        rig.key("\t");
    }
    assert!(!w.get_keys_held(), "a field holds the keyboard");
    rig.chord(Key::Control, "c");
    assert!(!presets.get_copying(), "⌘C typed in a field copies no settings");
    w.invoke_refocus();
    rig.pump();
    rig.chord(Key::Control, "c");
    assert!(presets.get_copying(), "with the photos holding the keyboard, ⌘C copies the settings");
}
