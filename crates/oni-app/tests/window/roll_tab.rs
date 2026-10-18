//! The Roll tab's settings blocks on the headless window: Source Encoding on a file naming no
//! profile reads it again, the dye family and Source Balance write and reset as their blocks do,
//! and the flat-field's toggle, mask picker and Apply to Selection reach the frames.

use oni_app::app::{Options, Shell};
use oni_app::{Library, PanelState, Store as StoreGlobal};
use oni_core::{Archive, EditorStore};
use oni_settings::SourceEncoding;
use oni_settings::blocks::SeparationFamily;
use oni_settings::frame::FlatFieldMask;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, Model};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

pub(crate) fn the_roll_tabs_blocks_write_what_swifts_write() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the Roll tab is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-roll-tab-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 0.8), ("B.tif", 0.5)] {
        common::plain_tiff(&folder.join(name), 96, 64, tone);
    }
    // One mask loaded before, as `CustomFlatFields` lists it.
    std::fs::create_dir_all(support.join("FlatFields")).unwrap();
    std::fs::write(support.join("FlatFields/masks.json"), r#"[{"id":"a1","name":"Rig"}]"#).unwrap();
    let canonical = |n: &str| PathBuf::from(oni_library::paths::canonical(&folder.join(n)));
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
    let (lib, panel, g) = (w.global::<Library>(), w.global::<PanelState>(), w.global::<StoreGlobal>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    rig.pump();
    let settings = || ctl.state.borrow().store().settings().clone();

    // A TIFF naming no profile offers its encodings; Linear reads the file again.
    assert!(panel.get_undeclared(), "a file naming no profile");
    assert_eq!(panel.get_encoding(), 0);
    let renders = ctl.state.borrow().renders();
    panel.invoke_encoding_picked(1);
    rig.pump();
    assert_eq!(settings().source_encoding, Some(SourceEncoding::Linear));
    assert!(panel.get_encoding_modified());
    rig.until("the frame read again", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    panel.invoke_encoding_picked(0);
    rig.pump();
    assert_eq!(settings().source_encoding, None, "twin: sRGB is no encoding written");

    // The dye family names its make; the block's reset clears it with the dose.
    assert_eq!(panel.get_dye_family(), 2, "a negative rests on Generic");
    panel.invoke_dye_family_picked(0);
    rig.pump();
    assert_eq!(settings().separation_family, Some(SeparationFamily::KodakC41));
    assert!(g.get_dye_separation_modified());
    g.invoke_reset(5);
    rig.pump();
    assert_eq!((settings().separation_family, g.get_dye_separation_modified()), (None, false));

    // Source Balance: a negative names each row hue first; a drag moves the gain; the reset lands home.
    assert!(panel.get_gains_inverted());
    let field = oni_app::fields::Field::GainRed.index();
    g.invoke_press(field, false);
    g.invoke_drag(field, 30.0, 300.0, false);
    g.invoke_release(field);
    rig.pump();
    assert!(settings().gains.stops[0] > 0.1, "{:?}", settings().gains.stops);
    assert!(g.get_source_balance_modified());
    g.invoke_reset(10);
    rig.pump();
    assert_eq!(settings().gains.stops, [0.0; 3]);
    panel.invoke_film_type_picked(1);
    rig.pump();
    assert!(!panel.get_gains_inverted(), "twin: a positive's rows run the other way");
    panel.invoke_film_type_picked(2);
    rig.pump();
    assert!(panel.get_monochrome(), "black and white hides Chroma Denoise and the flat-field's colour pair");
    panel.invoke_film_type_picked(0);
    rig.pump();
    assert!(!panel.get_monochrome());

    // The flat-field: on takes a mask to choose, the sliders wait for it; the picker lists the one loaded.
    assert!(!panel.get_flat_field_on());
    panel.invoke_flat_field_toggled(true);
    rig.pump();
    assert_eq!((settings().flat_field.mask, settings().flat_field.custom.clone()), (FlatFieldMask::Custom, None));
    assert!(panel.get_flat_field_on() && !panel.get_masked());
    let lines: Vec<String> = (0..panel.get_mask_lines().row_count()).filter_map(|i| panel.get_mask_lines().row_data(i)).map(|l| l.text.to_string()).collect();
    assert_eq!(lines, ["None selected", "Rig"]);
    panel.invoke_mask_chosen(1);
    rig.pump();
    assert_eq!(settings().flat_field.custom.as_deref(), Some("a1"));
    assert!(panel.get_masked() && panel.get_mask_title() == "Rig");

    // Apply to Selection writes A's flat-field on B, one gallery step.
    lib.invoke_tile_clicked(1, true, false);
    rig.pump();
    assert_eq!(panel.get_flat_picked(), 1);
    panel.invoke_flat_apply_selection();
    rig.pump();
    let b = oni_settings::sidecar::read(&canonical("B.tif"), false, true).unwrap().settings;
    assert_eq!(b.flat_field.custom.as_deref(), Some("a1"));
    let _ = std::fs::remove_dir_all(&root);
}
