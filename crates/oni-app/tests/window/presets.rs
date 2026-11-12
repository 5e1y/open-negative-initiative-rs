//! The presets on the headless window: Save as Preset… opens on the rows the photo moved, a card
//! previews under the pointer without writing, a click applies to the selection as one gallery
//! step, the editor writes once the hand rests, the share code comes back as a copy, a delete asks.

use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{Advice, App, Confirm, Library, Menus, Presets, Store as StoreGlobal};
use oni_core::{Archive, EditorStore};
use oni_settings::sidecar;
use i_slint_backend_testing::AccessibleRole;
use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

fn exposure_of(path: &Path) -> f32 {
    let loaded = sidecar::read(path, false, true).expect("the sidecar reads");
    Field::Exposure.knob().get(&loaded.settings)
}

/// `Space.m`: a footer's keys stand this far under its rule, and the form ends further over it.
const STEP: f32 = 16.0;

/// A sheet sized to its content: the last box of each column ends a step over the footer's rule, the
/// footer inside the window.
fn fits(w: &oni_app::MainWindow, commit: &str) {
    let key = common::the(w, AccessibleRole::Button, commit);
    let size = w.window().size().to_logical(w.window().scale_factor());
    assert!(key.y + key.height <= size.height, "{commit} in the window: {key:?}");
    let lowest = ["Saturation Curve", "Scanner Keypad"].map(|name| common::the(w, AccessibleRole::Checkbox, name)).into_iter().fold(0.0, |y: f32, b| y.max(b.y + b.height));
    assert!(lowest + 2.0 * STEP < key.y, "every box a step over the rule above {commit}: {lowest} against {key:?}");
    // Twin: a card taller than its content would leave the boxes far above its footer.
    assert!(key.y - lowest < 100.0, "the card ends with its content: {lowest} against {key:?}");
}

pub(crate) fn a_preset_is_saved_previewed_applied_edited_shared_and_deleted_as_swift_does() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the presets are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-presets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches, trash) = (root.join("Scans"), root.join("Support"), root.join("Caches"), root.join("Trash"));
    for (name, tone) in [("A.tif", 0.9), ("B.tif", 0.6), ("C.tif", 0.4)] {
        common::linear_tiff(&folder.join(name), 96, 64, tone);
    }
    std::fs::create_dir_all(&trash).unwrap();
    let canonical = |n: &str| PathBuf::from(oni_library::paths::canonical(&folder.join(n)));
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let binned = trash.clone();
    let options = Options {
        support: support.clone(),
        caches,
        picker: Box::new(|| None),
        discard: Box::new(move |p: &Path| std::fs::rename(p, binned.join(p.file_name().unwrap()))),
        ..Options::default()
    };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let presets = w.global::<Presets>();
    let menus = w.global::<Menus>();
    let shelf = support.join("Presets");
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    assert_eq!(presets.get_cards().row_count(), 0);
    assert!(!menus.get_live().save_preset, "nothing to save with no photo open");
    assert_eq!(menus.get_presets().row_count(), 0, "the menu says No Presets");

    // A open, its Exposure moved: the save sheet proposes the moved rows under the first free name.
    let lib = w.global::<Library>();
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A on the canvas", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    assert!(!w.global::<App>().get_gallery());
    let g = w.global::<StoreGlobal>();
    let field = Field::Exposure.index();
    let rest = ctl.state.borrow().store().get(Field::Exposure);
    g.invoke_press(field, false);
    g.invoke_drag(field, 30.0, 300.0, false);
    g.invoke_release(field);
    rig.pump();
    let moved = ctl.state.borrow().store().get(Field::Exposure);
    assert!((moved - rest).abs() > 0.1, "{moved} against {rest}");
    assert!(menus.get_live().save_preset);
    ctl.act(oni_app::Act::SavePreset);
    rig.pump();
    assert!(presets.get_saving());
    assert_eq!(presets.get_form_name().as_str(), "Preset 01");
    let lines = presets.get_form_carries();
    let ticked: Vec<String> = (0..lines.row_count()).filter_map(|i| lines.row_data(i)).filter(|l| !l.title && l.on).map(|l| l.text.to_string()).collect();
    assert_eq!(ticked, ["Lighting"], "the rows Exposure moved, and only those");
    assert_eq!(presets.get_form_balance(), 0, "Not carried");
    rig.settle();
    println!("shot {}", rig.shot("presets-save-sheet").display());
    fits(w, "Save");
    // The balance choice over the blocks, which a copy goes without.
    common::the(w, AccessibleRole::Button, "Not carried");
    presets.invoke_form_named("Bright".into());
    presets.invoke_form_tinted(2);
    presets.invoke_save_confirmed();
    rig.pump();
    assert!(!presets.get_saving());
    let files = oni_core::presets::files(&shelf);
    assert_eq!(files.len(), 1, "one preset, one file");
    let saved = oni_core::presets::decode(&std::fs::read(&files[0]).unwrap()).unwrap();
    assert_eq!((saved.name.as_str(), saved.tint), ("Bright", oni_core::presets::Tint::Amber));
    assert_eq!(presets.get_cards().row_count(), 1);
    assert_eq!(presets.get_cards().row_data(0).unwrap().abbreviation.as_str(), "BRI");
    assert_eq!(menus.get_presets().row_data(0).unwrap().as_str(), "Bright \u{b7} Lighting");

    // The deck opens on its tab, and the card lifts under the pointer. The window's first move
    // only enters it, so the pointer comes in over the picture first.
    rig.settle();
    // A frame drawn first: the card's place is known once it has been laid out.
    rig.shot("presets-deck-rest");
    let first = || (presets.get_first_x() + 48.0, presets.get_first_y());
    let (x, rest_y) = first();
    let move_to = |x: f32, y: f32| {
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: slint::LogicalPosition::new(x, y) });
        rig.settle();
    };
    move_to(x, rest_y - 200.0);
    let tab = common::the(w, AccessibleRole::Button, "Presets").middle();
    move_to(tab.0, tab.1);
    assert_eq!(first().1, rest_y, "twin: the pointer on the tab leaves the deck closed");
    common::click(&rig, w, tab);
    rig.settle();
    let (_, up_y) = first();
    assert!(up_y < rest_y - 64.0, "the tab opens the deck: {up_y} against {rest_y}");
    move_to(x, up_y + 38.0);
    assert_eq!(presets.get_hovered(), 0, "the pointer on the card hovers it");
    println!("shot {}", rig.shot("presets-deck").display());
    move_to(x, rest_y - 200.0);
    assert_eq!((presets.get_hovered(), first().1), (-1, rest_y), "off the deck, it falls and previews nothing");

    // Back to rest, then a hover previews the preset on the canvas and writes nothing. The drag's
    // step is recorded once the hand rests, so the undo waits for it.
    let rested = std::time::Instant::now();
    rig.until("the drag's step", Duration::from_secs(2), || rested.elapsed() > Duration::from_millis(300));
    rig.chord(Key::Control, "z");
    rig.pump();
    assert!((ctl.state.borrow().store().get(Field::Exposure) - rest).abs() < 1e-6, "the keys came back once the sheet closed");
    presets.invoke_hover(0);
    rig.until("the preview drawn", Duration::from_secs(60), || {
        ctl.state.borrow().shown_settings().is_some_and(|s| (Field::Exposure.knob().get(s) - moved).abs() < 1e-4)
    });
    assert!((ctl.state.borrow().store().get(Field::Exposure) - rest).abs() < 1e-6, "a hover writes no setting");
    presets.invoke_hover(-1);
    rig.until("the preview left", Duration::from_secs(60), || {
        ctl.state.borrow().shown_settings().is_some_and(|s| (Field::Exposure.knob().get(s) - rest).abs() < 1e-6)
    });

    // Copy Settings asks what it carries, the look's boxes all ticked at first and none of the roll's;
    // Lighting unticked, Return copies, and the next copy opens on that choice.
    g.invoke_step(Field::Exposure.index(), 5);
    g.invoke_step(Field::Vignette.index(), -5);
    rig.pump();
    ctl.act(oni_app::Act::Copy);
    rig.pump();
    assert!(presets.get_copying() && presets.get_saving(), "Copy opens its sheet");
    let boxes = || {
        let lines = presets.get_form_carries();
        (0..lines.row_count()).filter_map(|i| lines.row_data(i).map(|l| (i, l))).filter(|(_, l)| !l.title).map(|(i, l)| (i, l.text.to_string(), l.on)).collect::<Vec<_>>()
    };
    assert!(boxes().iter().all(|b| b.2), "the look whole at first");
    assert!(!boxes().iter().any(|b| b.1 == "Flat-Field"), "no roll box");
    assert!(common::placed(w, AccessibleRole::Button, "Not carried").is_empty(), "twin: no balance choice");
    let lighting = boxes().iter().find(|b| b.1 == "Lighting").unwrap().0;
    presets.invoke_form_ticked(lighting as i32, false);
    rig.settle();
    println!("shot {}", rig.shot("presets-copy-sheet").display());
    rig.key("\n");
    assert!(!presets.get_saving(), "Return copies");
    ctl.act(oni_app::Act::Copy);
    rig.pump();
    assert_eq!(boxes().iter().filter(|b| !b.2).map(|b| b.1.as_str()).collect::<Vec<_>>(), ["Lighting"], "the next copy opens on the last one's choice");

    // "Don't ask again" holds through a copy alone: Escape drops it, and the sheet asks again. The hand
    // reaches for the box once the sheet has come in.
    let at_once = || common::the(w, AccessibleRole::Checkbox, "Don\u{2019}t ask again this session").middle();
    rig.settle();
    common::click(&rig, w, at_once());
    assert!(presets.get_copy_at_once(), "the click ticks it");
    rig.key("\u{1b}");
    assert!(!presets.get_saving());
    ctl.act(oni_app::Act::Copy);
    rig.pump();
    assert!(presets.get_saving() && !presets.get_copy_at_once(), "twin: a sheet cancelled asks again, the choice off");
    rig.settle();
    common::click(&rig, w, at_once());
    rig.key("\n");
    assert!(!presets.get_saving());
    // Then Copy takes the last boxes at once: the vignette moved since reaches the board, Lighting not.
    g.invoke_step(Field::Vignette.index(), -5);
    rig.pump();
    let vignetted = ctl.state.borrow().store().get(Field::Vignette);
    ctl.act(oni_app::Act::Copy);
    rig.pump();
    assert!(!presets.get_saving(), "Copy asks no more");

    // To the gallery, B and C picked, a click on the card applies to both as one gallery step.
    rig.chord(Key::Control, "g");
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    lib.invoke_tile_clicked(1, false, false);
    lib.invoke_tile_clicked(2, true, false);
    rig.pump();
    assert_eq!(menus.get_apply_title().as_str(), "Apply to 2 Photos");
    assert!(presets.get_can_apply());
    let own: Vec<f32> = ["A.tif", "B.tif", "C.tif"].iter().map(|n| exposure_of(&canonical(n))).collect();
    presets.invoke_apply(0);
    rig.pump();
    for name in ["B.tif", "C.tif"] {
        assert!((exposure_of(&canonical(name)) - moved).abs() < 1e-4, "{name}");
    }
    assert!((exposure_of(&canonical("A.tif")) - own[0]).abs() < 1e-6, "twin: A, not picked, keeps its own");
    ctl.act(oni_app::Act::Undo);
    rig.pump();
    for (name, was) in [("B.tif", own[1]), ("C.tif", own[2])] {
        assert!((exposure_of(&canonical(name)) - was).abs() < 1e-6, "{name}: one undo takes the apply back on both");
    }
    // The paste lays the vignette the copy carried, and leaves each photo its own Lighting.
    ctl.act(oni_app::Act::Paste);
    rig.pump();
    for (name, was) in [("B.tif", own[1]), ("C.tif", own[2])] {
        let pasted = sidecar::read(&canonical(name), false, true).unwrap().settings;
        assert!((Field::Vignette.knob().get(&pasted) - vignetted).abs() < 1e-5, "{name}: the vignette pasted");
        assert!((exposure_of(&canonical(name)) - was).abs() < 1e-6, "{name}: twin, the Lighting left out stays");
    }
    ctl.act(oni_app::Act::Undo);
    rig.pump();

    // The editor: a rename is written once the hand rests, its file following the new name.
    ctl.act(oni_app::Act::EditPresets);
    rig.pump();
    assert!(presets.get_editing() && presets.get_form_shown());
    rig.settle();
    println!("shot {}", rig.shot("presets-editor").display());
    // Short of room, the form scrolls under a footer that stays; the evaluator's window holds it whole.
    let done = common::the(w, AccessibleRole::Button, "Done");
    assert!(done.y + done.height <= 800.0, "the footer stays in the window: {done:?}");
    assert!(common::placed(w, AccessibleRole::Checkbox, "Saturation Curve").is_empty(), "twin: the form's foot scrolled out of sight");
    rig.window.set_size(slint::PhysicalSize::new(1728, 1017));
    rig.settle();
    fits(w, "Done");
    // What a preset carries reads whole under its name, wrapped in the list's column.
    let row = || common::the(w, AccessibleRole::ListItem, "Bright");
    let within = |words: &str| {
        let r = row();
        let inside = |t: &common::Placed| t.x >= r.x && t.x < r.x + r.width && t.y >= r.y && t.y < r.y + r.height;
        common::placed(w, AccessibleRole::Text, words).into_iter().find(inside).unwrap_or_else(|| panic!("{words:?} in {r:?}"))
    };
    let detail = || within(presets.get_rows().row_data(0).unwrap().carries.as_str());
    let (one, name) = (detail(), within("Bright"));
    for line in ["Saturation", "Chroma Decompression", "Vignette", "Curves"] {
        presets.invoke_form_ticked(boxes().iter().find(|b| b.1 == line).unwrap().0 as i32, true);
    }
    rig.settle();
    println!("shot {}", rig.shot("presets-editor-carries").display());
    let (long, r) = (detail(), row());
    assert!(long.height > 2.0 * one.height, "the carries wrap whole: {long:?} against {one:?}");
    assert!(long.y + long.height <= r.y + r.height && long.x + long.width <= r.x + r.width, "inside their row: {long:?} in {r:?}");
    // Twin: one carried block stays one line, as high as the name over it.
    assert!(one.height <= name.height + 1.0, "twin: one block on one line: {one:?} against {name:?}");
    rig.until("the boxes written", Duration::from_secs(5), || oni_core::presets::load(&shelf)[0].parameters.len() > saved.parameters.len());
    rig.settle();
    assert_eq!(presets.get_selected(), 0);
    presets.invoke_form_named("Brighter".into());
    presets.invoke_form_noted("For thin negatives".into());
    rig.pump();
    assert_eq!(oni_core::presets::load(&shelf)[0].name, "Bright", "nothing written mid-edit");
    rig.until("the pause's write", Duration::from_secs(5), || oni_core::presets::load(&shelf).first().is_some_and(|p| p.name == "Brighter"));
    assert_eq!(oni_core::presets::files(&shelf).len(), 1, "a rename leaves one file");
    assert_eq!(oni_core::presets::load(&shelf)[0].note, "For thin negatives");
    // A blank name is settled to the one on disk when the editor closes.
    presets.invoke_form_named("   ".into());
    presets.invoke_done();
    rig.pump();
    assert!(!presets.get_editing());
    assert_eq!(oni_core::presets::load(&shelf)[0].name, "Brighter");

    // The share code comes back as the same look, so pasting it adds nothing and says so.
    let held = oni_core::presets::load(&shelf).remove(0);
    let code = oni_core::presets::share_code(&held).unwrap();
    ctl.adopt_preset_code(&code);
    rig.pump();
    assert!(w.global::<Advice>().get_shown());
    assert_eq!(w.global::<Advice>().get_title().as_str(), "\u{201c}Brighter\u{201d} Is Already in Your Presets");
    rig.key("\n");
    assert!(!w.global::<Advice>().get_shown());
    ctl.adopt_preset_code("ONI1:garbled");
    assert_eq!(w.global::<Advice>().get_message().as_str(), "The code is incomplete or was changed on the way. Ask for it to be copied again.");
    rig.key("\u{1b}");

    // An import of the preset's own file is left out; a deck Duplicate makes a second file.
    ctl.import_preset_files(&[oni_core::presets::files(&shelf)[0].clone()]);
    assert_eq!(w.global::<Advice>().get_title().as_str(), "No Preset Imported");
    rig.key("\n");
    assert!(presets.invoke_card_menu(0));
    let menu: Vec<String> = (0..presets.get_menu_lines().row_count()).filter_map(|i| presets.get_menu_lines().row_data(i)).map(|l| l.text.to_string()).collect();
    assert_eq!(menu, ["Edit…", "Update with Current Settings", "Duplicate", "Share Preset…", "Copy Preset Code", "Delete \u{201c}Brighter\u{201d}"]);
    assert!(common::glyphed(presets.get_menu_lines().iter()), "every line of the card's menu wears its glyph");
    presets.invoke_menu_chosen(2);
    rig.pump();
    assert_eq!(oni_core::presets::files(&shelf).len(), 2);
    assert_eq!(presets.get_cards().row_count(), 2);

    // Delete a Preset asks first; Cancel keeps the file, the key sends it to the Trash.
    menus.invoke_delete_preset(1);
    rig.pump();
    assert!(w.global::<Confirm>().get_shown());
    assert_eq!(w.global::<Confirm>().get_key().as_str(), "Delete");
    rig.key("\u{1b}");
    assert_eq!(oni_core::presets::files(&shelf).len(), 2, "twin: Cancel deletes nothing");
    menus.invoke_delete_preset(1);
    assert!(w.global::<Confirm>().get_destructive(), "Delete is the red key");
    rig.key("\n");
    assert_eq!(oni_core::presets::files(&shelf).len(), 2, "a destructive key takes no Return");
    w.global::<Confirm>().invoke_answered(true);
    rig.pump();
    assert_eq!(oni_core::presets::files(&shelf).len(), 1);
    assert_eq!(std::fs::read_dir(&trash).unwrap().count(), 1, "the file went to the Trash, not away");
    assert_eq!(presets.get_cards().row_count(), 1);
    let _ = std::fs::remove_dir_all(&root);
}
