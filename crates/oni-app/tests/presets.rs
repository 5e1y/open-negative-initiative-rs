//! The presets on the headless window: Save as Preset… opens on the rows the photo moved, a card
//! previews under the pointer without writing, a click applies to the selection as one gallery
//! step, the editor writes once the hand rests, the share code comes back as a copy, a delete asks.

use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{Advice, App, Confirm, Library, Menus, Presets, Store as StoreGlobal};
use oni_core::{Archive, EditorStore};
use oni_settings::sidecar;
use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::time::Duration;

mod common;

fn exposure_of(path: &Path) -> f32 {
    let loaded = sidecar::read(path, false, true).expect("the sidecar reads");
    Field::Exposure.knob().get(&loaded.settings)
}

#[test]
fn a_preset_is_saved_previewed_applied_edited_shared_and_deleted_as_swift_does() {
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
    assert_eq!(menus.get_presets().row_count(), 0, "the menu says No presets");

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
    ctl.act("save-preset");
    rig.pump();
    assert!(presets.get_saving());
    assert_eq!(presets.get_form_name().as_str(), "Preset 01");
    let lines = presets.get_form_carries();
    let ticked: Vec<String> = (0..lines.row_count()).filter_map(|i| lines.row_data(i)).filter(|l| l.kind == 2 && l.on).map(|l| l.text.to_string()).collect();
    assert_eq!(ticked, ["Lighting"], "the rows Exposure moved, and only those");
    let balance = (0..lines.row_count()).filter_map(|i| lines.row_data(i)).find(|l| l.kind == 1).unwrap();
    assert_eq!(balance.text.as_str(), "Not carried");
    rig.settle();
    println!("shot {}", rig.shot("presets-save-sheet").display());
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

    // The deck rises under the pointer on its strip, and the card lifts under it. The window's
    // first move only enters it, so the pointer comes in over the picture first.
    rig.settle();
    // A frame drawn first: the card's place is known once it has been laid out.
    rig.shot("presets-deck-rest");
    let first = || (presets.get_first_x() + 36.0, presets.get_first_y());
    let (x, rest_y) = first();
    let move_to = |x: f32, y: f32| {
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: slint::LogicalPosition::new(x, y) });
        rig.settle();
    };
    move_to(x, rest_y - 200.0);
    move_to(x, rest_y - 40.0);
    assert_eq!(first().1, rest_y, "twin: the pointer just over the strip leaves the deck down");
    move_to(x, rest_y + 8.0);
    let (_, up_y) = first();
    assert!(up_y < rest_y - 80.0, "the strip raises the deck: {up_y} against {rest_y}");
    move_to(x, up_y + 52.0);
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
    ctl.act("undo");
    rig.pump();
    for (name, was) in [("B.tif", own[1]), ("C.tif", own[2])] {
        assert!((exposure_of(&canonical(name)) - was).abs() < 1e-6, "{name}: one undo takes the apply back on both");
    }

    // The editor: a rename is written once the hand rests, its file following the new name.
    ctl.act("edit-presets");
    rig.pump();
    assert!(presets.get_editing() && presets.get_form_shown());
    rig.settle();
    println!("shot {}", rig.shot("presets-editor").display());
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
    rig.key("\r");
    assert!(!w.global::<Advice>().get_shown());
    ctl.adopt_preset_code("ONI1:garbled");
    assert_eq!(w.global::<Advice>().get_message().as_str(), "The code is incomplete or was changed on the way. Ask for it to be copied again.");
    rig.key("\u{1b}");

    // An import of the preset's own file is left out; a deck Duplicate makes a second file.
    ctl.import_preset_files(&[oni_core::presets::files(&shelf)[0].clone()]);
    assert_eq!(w.global::<Advice>().get_title().as_str(), "No Preset Imported");
    rig.key("\r");
    assert!(presets.invoke_card_menu(0));
    let menu: Vec<String> = (0..presets.get_menu_lines().row_count()).filter_map(|i| presets.get_menu_lines().row_data(i)).map(|l| l.text.to_string()).collect();
    assert_eq!(menu, ["Edit…", "Update with Current Settings", "Duplicate", "Share Preset…", "Copy Preset Code", "Delete \u{201c}Brighter\u{201d}"]);
    presets.invoke_menu_chosen(2);
    rig.pump();
    assert_eq!(oni_core::presets::files(&shelf).len(), 2);
    assert_eq!(presets.get_cards().row_count(), 2);

    // Delete a Preset asks first; Cancel keeps the file, the key sends it to the Trash.
    menus.invoke_delete_preset(1);
    rig.pump();
    assert!(w.global::<Confirm>().get_shown());
    assert_eq!(w.global::<Confirm>().get_key().as_str(), "DELETE");
    rig.key("\u{1b}");
    assert_eq!(oni_core::presets::files(&shelf).len(), 2, "twin: Cancel deletes nothing");
    menus.invoke_delete_preset(1);
    rig.key("\r");
    rig.pump();
    assert_eq!(oni_core::presets::files(&shelf).len(), 1);
    assert_eq!(std::fs::read_dir(&trash).unwrap().count(), 1, "the file went to the Trash, not away");
    assert_eq!(presets.get_cards().row_count(), 1);
    let _ = std::fs::remove_dir_all(&root);
}
