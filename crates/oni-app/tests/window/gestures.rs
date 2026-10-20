//! The menus and the gestures on several photos, on the headless window: Swift's titles and live
//! rules, copy and paste across rolls with its confirm, the ONI Trash, the filter, the gallery's
//! own undo naming its step, the film type, the framing's copy and paste, mirror and ⌘K.

use oni_app::app::{self, Options, Shell};
use oni_app::fields::Field;
use oni_app::{App, Confirm, Framing, Library, Menus, PanelState};
use oni_core::{Archive, EditorStore};
use oni_library::index::LibraryIndex;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::frame::RollReference;
use oni_settings::{ConversionMode, Id, PipelineSettings};
use slint::ComponentHandle;
use slint::platform::{Key, WindowEvent};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

fn settings_of(path: &Path) -> PipelineSettings {
    sidecar::read(path, false, true).unwrap().settings
}

fn exposure(path: &Path) -> f32 {
    Field::Exposure.knob().get(&settings_of(path))
}

/// A sidecar naming roll `id`, so a paste can cross rolls.
fn on_roll(path: &Path, id: Id) {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, false);
    s.roll_reference = Some(RollReference { id, stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None });
    sidecar::write(&s, path, WriteOptions::default()).unwrap();
}

pub(crate) fn the_menus_name_what_they_do_and_the_gestures_reach_every_target() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the gestures are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-gestures-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    let (x, y) = (Id::new(), Id::new());
    for (name, roll) in [("A.tif", x), ("B.tif", y), ("C.tif", y)] {
        common::linear_tiff(&folder.join(name), 96, 64, 0.8);
        on_roll(&folder.join(name), roll);
    }
    let path = |n: &str| PathBuf::from(oni_library::paths::canonical(&folder.join(n)));
    let (a, b, c) = (path("A.tif"), path("B.tif"), path("C.tif"));
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches: caches.clone(), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 2.0 });
    rig.window.set_size(slint::PhysicalSize::new(2 * 1280, 2 * 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let menus = w.global::<Menus>();
    let lib = w.global::<Library>();
    shell.open_folder(&folder);
    rig.until("the library read", Duration::from_secs(30), || ctl.state.borrow().read_done());

    // The gallery's face of every dynamic title, and nothing to take into the editor yet.
    let t = menus.get_titles();
    assert_eq!((t.mode.as_str(), t.smaller.as_str(), t.larger.as_str()), ("Editor", "Smaller Thumbnails", "Larger Thumbnails"));
    assert_eq!((t.undo.as_str(), t.filter.as_str(), t.paste.as_str()), ("Undo", "Filter: All", "Paste Settings"));
    assert!(!menus.get_live().toggle_mode && !menus.get_live().medal && !menus.get_live().copy_settings);
    // ⌘G takes the ringed photo into the editor, where the titles change face.
    lib.invoke_tile_clicked(0, false, false);
    rig.pump();
    assert!(menus.get_live().toggle_mode && menus.get_live().medal);
    rig.chord(Key::Control, "g");
    assert_eq!(ctl.state.borrow().open_path(), Some(a.to_str().unwrap()));
    let t = menus.get_titles();
    assert_eq!((t.mode.as_str(), t.smaller.as_str(), t.library.as_str()), ("Gallery", "Zoom Out", "Hide the Library"));
    assert!(menus.get_live().copy_settings && menus.get_live().framing && !menus.get_live().paste_settings);
    // The framing keys act on a decoded frame only.
    rig.until("the frame decoded", Duration::from_secs(60), || ctl.state.borrow().renders() >= 1);

    // ⌥⌘] hides the panel and the title says how to bring it back; ⌘K opens the Crop tab.
    // Windows reads a bare Ctrl+Alt as AltGr when the event names no unmodified text, as this rig's do.
    let hide_panel = || if cfg!(windows) { ctl.act("hide-panel") } else { rig.chord_with(&[Key::Control, Key::Alt], "]") };
    hide_panel();
    assert!(w.global::<App>().get_panel_hidden());
    assert_eq!(menus.get_titles().panel.as_str(), "Show the Panel");
    hide_panel();
    w.global::<PanelState>().set_tab(2);
    rig.chord(Key::Control, "k");
    assert_eq!(w.global::<PanelState>().get_tab(), 1);
    rig.chord(Key::Control, "k");
    assert_eq!(w.global::<PanelState>().get_tab(), 1, "`openCrop()`: ⌘K again stays in the crop");
    // A, never balanced, opened with its window armed: Return applies it, a second leaves the crop.
    assert!(w.global::<Framing>().get_armed());
    rig.key("\r");
    assert_eq!((w.global::<PanelState>().get_tab(), w.global::<Framing>().get_armed()), (1, false));
    rig.key("\r");
    assert_eq!(w.global::<PanelState>().get_tab(), 2, "twin: the second Return is the crop's way out");
    // A bare bracket is the brush's only on its own tab: here it reaches nothing.
    let revision = ctl.state.borrow().store().revision();
    rig.key("]");
    assert_eq!(ctl.state.borrow().store().revision(), revision);
    assert!(!menus.get_live().bigger_brush);

    // ⇧⌘M mirrors the open frame, and the item turns into its undoing.
    rig.chord_with(&[Key::Control, Key::Shift], "m");
    assert!(ctl.state.borrow().store().settings().geometry.mirrored);
    assert_eq!(menus.get_titles().mirror.as_str(), "Unmirror");

    // The film type's tabs carry the open frame's mode, as its own edit.
    w.global::<PanelState>().invoke_film_type_picked(1);
    rig.pump();
    assert_eq!(ctl.state.borrow().store().settings().mode, ConversionMode::Positive);
    assert_eq!(w.global::<PanelState>().get_film_type(), 1);
    rig.chord(Key::Control, "z");
    assert_eq!(ctl.state.borrow().store().settings().mode, ConversionMode::Negative, "the editor's ⌘Z takes the mode back");

    // The framing travels by its own Copy and Paste, never with the settings.
    assert!(!w.global::<Framing>().get_can_paste());
    w.global::<Framing>().invoke_act("copy-framing".into());
    rig.pump();
    assert!(w.global::<Framing>().get_can_paste());

    // Copy Settings on an exposure moved, then the gallery with two photos of another roll selected.
    w.global::<oni_app::Store>().invoke_step(Field::Exposure.index(), 5);
    rig.pump();
    let donor = ctl.state.borrow().store().get(Field::Exposure);
    rig.chord_with(&[Key::Control, Key::Shift], "c");
    assert!(menus.get_live().paste_to_roll);
    rig.chord(Key::Control, "g");
    assert!(w.global::<App>().get_gallery());
    lib.invoke_tile_clicked(1, false, false);
    lib.invoke_tile_clicked(2, true, false);
    rig.pump();
    let t = menus.get_titles();
    assert_eq!((t.paste.as_str(), t.remove.as_str()), ("Paste Settings to 2 Photos", "Move 2 Photos to ONI Trash"));
    let (b0, c0) = (exposure(&b), exposure(&c));
    assert!((b0 - donor).abs() > 1e-3);

    // Another roll's look asks first; Escape leaves both frames as they were.
    rig.chord_with(&[Key::Control, Key::Shift], "v");
    let confirm = w.global::<Confirm>();
    assert!(confirm.get_shown());
    assert_eq!((confirm.get_title().as_str(), confirm.get_key().as_str()), ("Another Roll", "PASTE SETTINGS"));
    assert_eq!(confirm.get_message().as_str(), "You are trying to apply settings from one roll to another: results may vary.");
    rig.settle();
    println!("shot {}", rig.shot("another-roll").display());
    rig.key("\u{1b}");
    assert!(!confirm.get_shown());
    assert_eq!((exposure(&b), exposure(&c)), (b0, c0), "a cancelled paste writes nothing");
    // Twin: Return goes ahead, both frames take the look, and ⌘Z names the gallery's step.
    rig.chord_with(&[Key::Control, Key::Shift], "v");
    rig.key("\r");
    assert!((exposure(&b) - donor).abs() < 1e-5 && (exposure(&c) - donor).abs() < 1e-5);
    assert_eq!(menus.get_titles().undo.as_str(), "Undo Paste settings to 2 photos");
    rig.chord(Key::Control, "z");
    assert_eq!((exposure(&b), exposure(&c)), (b0, c0), "one undo takes both frames back");
    assert_eq!(menus.get_titles().redo.as_str(), "Redo Paste settings to 2 photos");
    assert_eq!(menus.get_titles().undo.as_str(), "Undo", "nothing else on the gallery's stack");
    rig.chord_with(&[Key::Control, Key::Shift], "z");
    assert!((exposure(&b) - donor).abs() < 1e-5, "the redo lays it again");

    // Reset to Imported State on the two: one step, named by its count.
    rig.chord_with(&[Key::Control, Key::Shift], "r");
    assert!((exposure(&b) - b0).abs() < 1e-5 && (exposure(&c) - c0).abs() < 1e-5);
    assert_eq!(menus.get_titles().undo.as_str(), "Undo Reset 2 photos");

    // The framing pasted on the selection: geometry alone, the exposure untouched.
    w.global::<Framing>().invoke_act("paste-framing".into());
    rig.pump();
    assert!(settings_of(&b).geometry.mirrored && settings_of(&c).geometry.mirrored);
    assert_eq!(menus.get_titles().undo.as_str(), "Undo Paste framing to 2 photos");

    // ⌘⌫ marks C in the ONI Trash: it leaves the gallery and the index's listing; ⌘Z brings it back.
    lib.invoke_tile_clicked(2, false, false);
    rig.pump();
    assert_eq!(menus.get_titles().remove.as_str(), "Move to ONI Trash");
    let backspace: slint::SharedString = Key::Backspace.into();
    rig.chord(Key::Control, &backspace);
    assert_eq!(app::tiles(w).len(), 2);
    assert!(app::tiles(w).iter().all(|t| t.caption != "C"));
    let row = |p: &Path| LibraryIndex::open(&LibraryIndex::default_path(&caches)).unwrap().all().into_iter().find(|e| e.path == p.to_str().unwrap()).unwrap();
    assert!(row(&c).trashed_at.is_some() && c.exists(), "marked, never moved");
    assert_eq!(menus.get_titles().undo.as_str(), "Undo Move to ONI Trash");
    rig.chord(Key::Control, "z");
    assert_eq!(app::tiles(w).len(), 3);
    assert!(row(&c).trashed_at.is_none());

    // ⌘L cycles one medal at a time, and the gallery keeps exactly that medal.
    lib.invoke_tile_clicked(1, false, false);
    rig.key("2");
    let filtered = |m: &str| {
        rig.chord(Key::Control, "l");
        assert_eq!(menus.get_titles().filter.as_str(), format!("Filter: {m}"));
        app::tiles(w).iter().map(|t| t.caption.to_string()).collect::<Vec<_>>()
    };
    assert!(filtered("Bronze").is_empty());
    assert_eq!(filtered("Silver"), vec!["B".to_string()]);
    assert!(filtered("Gold").is_empty(), "twin: a silver photo is not a gold one");
    assert_eq!(filtered("All").len(), 3);

    // ⇧⌘G moves the selection into a dated roll folder beside it, one gallery step ⌘Z walks back.
    // The double-click interval first: a second click on B within it would open B.
    std::thread::sleep(oni_app::system::double_click() + std::time::Duration::from_millis(50));
    lib.invoke_tile_clicked(1, false, false);
    lib.invoke_tile_clicked(2, true, false);
    rig.pump();
    assert_eq!(menus.get_titles().roll.as_str(), "New Roll from 2 Photos");
    assert!(menus.get_live().new_roll);
    rig.chord_with(&[Key::Control, Key::Shift], "g");
    let roll = PathBuf::from(oni_library::paths::canonical(&folder)).join(oni_library::roll_folder::dated(oni_settings::Date::now()));
    assert!(roll.join("B.tif").exists() && roll.join("C.tif").exists(), "{roll:?}");
    assert!(sidecar::url_for(&roll.join("B.tif")).exists(), "the sidecar moves with its photo");
    assert!(a.exists() && !b.exists(), "twin: the unselected photo stays where it was");
    assert_eq!(menus.get_titles().undo.as_str(), "Undo New Roll");
    rig.until("the roll read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    assert_eq!(app::tiles(w).len(), 3, "the moved photos stay in the gallery");
    rig.chord(Key::Control, "z");
    assert!(b.exists() && c.exists() && !roll.exists(), "the undo puts them back and takes the folder away");
    rig.until("the folder read again", Duration::from_secs(30), || ctl.state.borrow().read_done());
    assert_eq!(app::tiles(w).len(), 3);

    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
