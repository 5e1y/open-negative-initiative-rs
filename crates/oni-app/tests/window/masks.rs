//! The mask library on the headless window: Load…, Rename, Folder, the sliders kept for the next
//! fresh frame, and Use This Photo as Flat-Field with its offer to move the photo out.

use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::flat_fields;
use oni_app::{Advice, App, Library, MaskDialog, PanelState, Store as StoreGlobal};
use oni_core::{Archive, EditorStore};
use oni_settings::frame::FlatFieldMask;
use slint::ComponentHandle;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use crate::common;

pub(crate) fn the_mask_library_loads_renames_shows_remembers_and_moves_a_photo_out_as_swift_does() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the mask library is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-masks-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 0.8), ("B.tif", 0.6), ("C.tif", 0.4)] {
        common::linear_tiff(&folder.join(name), 96, 64, tone);
    }
    // The panel lies outside the library; a file no reader takes stands beside it.
    let panel_file = root.join("Rig/panel.tif");
    common::linear_tiff(&panel_file, 120, 80, 0.9);
    let noise = root.join("Rig/noise.tif");
    std::fs::write(&noise, b"no picture in here").unwrap();
    let canonical = |n: &str| oni_library::paths::canonical(&folder.join(n));
    let chosen: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
    let shown: Rc<RefCell<Vec<PathBuf>>> = Rc::new(RefCell::new(Vec::new()));
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let (asked, seen) = (chosen.clone(), shown.clone());
    let options = Options {
        support: support.clone(),
        caches,
        picker: Box::new(|| None),
        mask_picker: Box::new(move || asked.borrow().clone()),
        show_folder: Box::new(move |p| seen.borrow_mut().push(p.to_path_buf())),
        ..Options::default()
    };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, panel, dialog, advice) = (w.global::<Library>(), w.global::<PanelState>(), w.global::<MaskDialog>(), w.global::<Advice>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    rig.pump();
    let settings = || ctl.state.borrow().store().settings().clone();
    assert!(panel.get_can_take_mask() && panel.get_can_load_mask(), "a frame is open, nothing under way");
    assert!(!panel.get_can_rename_mask(), "no loaded mask to rename yet");

    // Load…: a cancelled panel does nothing; the panel's picture becomes the frame's mask, no offer made.
    panel.invoke_load_mask();
    rig.pump();
    assert!(!panel.get_loading_mask() && flat_fields::entries(&support).is_empty());
    *chosen.borrow_mut() = Some(panel_file.clone());
    panel.invoke_load_mask();
    assert!(panel.get_loading_mask() && !panel.get_can_load_mask(), "Loading… while the decode runs");
    rig.until("the panel loaded", Duration::from_secs(60), || !panel.get_loading_mask());
    let masks = flat_fields::entries(&support);
    assert_eq!(masks.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["panel"]);
    let mask = flat_fields::read(&support, &masks[0].id).expect("the mask is saved in the app's folder");
    assert!(mask.width == 120 && mask.height == 80, "the preview's side of a frame smaller than it: {}x{}", mask.width, mask.height);
    assert_eq!((settings().flat_field.mask, settings().flat_field.custom.clone()), (FlatFieldMask::Custom, Some(masks[0].id.clone())));
    assert!(panel.get_masked() && panel.get_mask_title() == "panel" && panel.get_can_rename_mask());
    assert!(!dialog.get_shown(), "twin: a file outside the library is not offered to move out");

    // A file no reader takes says so under Swift's title, and changes nothing.
    *chosen.borrow_mut() = Some(noise.clone());
    panel.invoke_load_mask();
    rig.until("the noise refused", Duration::from_secs(60), || !panel.get_loading_mask());
    assert!(advice.get_shown());
    assert_eq!((advice.get_title().as_str(), advice.get_message().as_str()), ("No Mask Loaded", "This file could not be decoded as a picture."));
    advice.invoke_closed();
    assert_eq!(flat_fields::entries(&support).len(), 1);

    // Rename: the sheet opens on the current name; a blank or unchanged name renames nothing.
    panel.invoke_rename_mask();
    rig.pump();
    assert!(dialog.get_shown() && dialog.get_field());
    assert_eq!((dialog.get_title().as_str(), dialog.get_text().as_str(), dialog.get_key().as_str()), ("Rename This Flat-Field Mask", "panel", "RENAME"));
    dialog.invoke_answered(true, "   ".into());
    rig.pump();
    assert!(!dialog.get_shown() && flat_fields::entries(&support)[0].name == "panel");
    panel.invoke_rename_mask();
    dialog.set_text("  Easel ".into());
    rig.pump();
    rig.shot("masks-rename");
    rig.key("\n");
    assert_eq!(flat_fields::entries(&support)[0].name, "Easel", "Return renames, the name trimmed");
    assert_eq!(panel.get_mask_title(), "Easel");
    assert_eq!(settings().flat_field.custom.as_deref(), Some(masks[0].id.as_str()), "the id stays");

    // Folder: FlatField Presets in the app's folder, made when absent.
    panel.invoke_mask_folder();
    assert_eq!(*shown.borrow(), [flat_fields::presets(&support)]);
    assert!(flat_fields::presets(&support).is_dir());

    // The colour a hand dials is kept; an undo keeps nothing; B, never dialled, opens on it.
    let g = w.global::<StoreGlobal>();
    let field = Field::FlatTemperature.index();
    g.invoke_press(field, false);
    g.invoke_drag(field, 30.0, 300.0, false);
    g.invoke_release(field);
    rig.pump();
    let dialled = settings().flat_field.temperature;
    assert!(dialled > 0.05, "{dialled}");
    let kept = || oni_app::preferences::Preferences::load(&support).value(flat_fields::SLIDERS_KEY).cloned();
    assert_eq!(kept().and_then(|v| v.as_array().and_then(|a| a[1].as_f64())).map(|t| t as f32), Some(dialled));
    ctl.act(oni_app::Act::Undo);
    rig.pump();
    assert_eq!(settings().flat_field.temperature, 0.0);
    assert_eq!(kept().and_then(|v| v.as_array().and_then(|a| a[1].as_f64())).map(|t| t as f32), Some(dialled), "twin: an undo remembers nothing");
    ctl.act(oni_app::Act::ToggleMode);
    lib.invoke_tile_clicked(1, false, false);
    lib.invoke_tile_clicked(1, false, false);
    rig.until("B open", Duration::from_secs(60), || ctl.state.borrow().open_path() == Some(canonical("B.tif").as_str()));
    assert_eq!((settings().flat_field.temperature, settings().flat_field.amount, settings().flat_field.mask), (dialled, 1.0, FlatFieldMask::None));

    // Use This Photo as Flat-Field on B: Setting… while it decodes, then the offer to move B out.
    panel.invoke_take_mask();
    assert!(panel.get_taking_mask() && !panel.get_can_take_mask());
    rig.until("B taken", Duration::from_secs(60), || !panel.get_taking_mask());
    let b_id = flat_fields::id_of(&std::fs::read(folder.join("B.tif")).unwrap(), None);
    assert_eq!((settings().flat_field.mask, settings().flat_field.custom.as_deref(), settings().flat_field.amount), (FlatFieldMask::Custom, Some(b_id.as_str()), 1.0));
    assert!(dialog.get_shown() && !dialog.get_field());
    assert_eq!(dialog.get_title(), "Move This Photo Out of the Library?");
    assert_eq!((dialog.get_cancel().as_str(), dialog.get_key().as_str()), ("KEEP IN LIBRARY", "MOVE OUT"));
    rig.shot("masks-move-out");
    // Keep in Library: B stays where it is.
    rig.key("\u{1b}");
    assert!(!dialog.get_shown() && folder.join("B.tif").exists());
    // Again, the mask known already, then Move Out: B leaves, C opens under B's flat-field.
    panel.invoke_take_mask();
    rig.until("B taken again", Duration::from_secs(60), || !panel.get_taking_mask());
    assert_eq!(flat_fields::entries(&support).len(), 2, "the same file adds nothing");
    let carried = settings().flat_field;
    dialog.invoke_answered(true, "".into());
    rig.pump();
    assert!(!folder.join("B.tif").exists() && !oni_settings::sidecar::url_for(&folder.join("B.tif")).exists());
    assert!(flat_fields::presets(&support).join("B.tif").exists(), "the file moved, never deleted");
    assert_eq!(ctl.state.borrow().open_path(), Some(canonical("C.tif").as_str()), "the next frame of the roll opens");
    assert_eq!(settings().flat_field, carried);
    assert!(!w.global::<App>().get_gallery());
    let renders = ctl.state.borrow().renders();
    rig.until("C rendered", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    let _ = std::fs::remove_dir_all(&root);
}
