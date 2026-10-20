//! Prepare the Roll's last gaps on the headless window: the flat-field sliders a hand last left open
//! a fresh roll and are kept as they move; a flagged frame that cannot be read is written, as Swift's is.

use oni_app::app::{Options, Shell};
use oni_app::{Bar, Library, Preparing};
use oni_core::{Archive, EditorStore};
use oni_settings::frame::FlatField;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{ConversionMode, PipelineSettings};
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

fn loaded(path: &Path) -> sidecar::Loaded {
    sidecar::read(path, false, true).unwrap()
}

pub(crate) fn the_roll_takes_the_flat_field_sliders_kept_and_writes_a_flagged_frame_it_cannot_read() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: Prepare the Roll's leftovers are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-prepare-leftovers-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (scans, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 0.9), ("B.tif", 0.7), ("C.tif", 0.5), ("D.tif", 0.3)] {
        common::graded_tiff(&scans.join("R1").join(name), 120, 80, tone);
    }
    for (name, tone) in [("E.tif", 0.8), ("F.tif", 0.6)] {
        common::graded_tiff(&scans.join("R2").join(name), 120, 80, tone);
    }
    let path = |roll: &str, n: &str| PathBuf::from(oni_library::paths::canonical(&scans.join(roll).join(n)));
    // R2's frames had their colour dialled by hand.
    let mut dialled = PipelineSettings::rest(ConversionMode::Negative, false);
    dialled.flat_field.temperature = 0.2;
    for n in ["E.tif", "F.tif"] {
        sidecar::write(&dialled, &path("R2", n), WriteOptions::default()).unwrap();
    }
    let kept = || oni_app::preferences::Preferences::load(&support).value(oni_app::flat_fields::SLIDERS_KEY).cloned();
    oni_app::preferences::Preferences::load(&support).set(oni_app::flat_fields::SLIDERS_KEY, serde_json::json!([0.5, 0.4, -0.3])).unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support: support.clone(), caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, p) = (w.global::<Library>(), w.global::<Preparing>());
    shell.open_folder(&scans);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().photos() == 6);
    rig.settle();
    let index = |name: &str| ctl.state.borrow().order().iter().position(|p| p.ends_with(name)).unwrap_or_else(|| panic!("{name}")) as i32;
    let prepare = |name: &str, frames: usize| {
        lib.invoke_tile_clicked(index(name), false, false);
        rig.pump();
        w.global::<Bar>().invoke_act("prepare-roll".into());
        rig.until("the roll's surface", Duration::from_secs(10), || p.get_shown() && p.get_cells().row_count() == frames);
        rig.until("every frame read", Duration::from_secs(10), || p.get_capture() == 0);
    };

    // A roll whose first frame was never dialled opens on the colour kept, its Amount whole.
    prepare("A.tif", 4);
    p.invoke_advance();
    rig.until("the drafts written", Duration::from_secs(10), || loaded(&path("R1", "B.tif")).roll_draft.is_some());
    let flat = loaded(&path("R1", "B.tif")).roll_draft.unwrap().flat_field.unwrap();
    assert_eq!((flat.temperature, flat.tint), (0.4, -0.3), "the colour kept");
    assert_eq!(flat.amount, FlatField::default().amount, "twin: the Amount kept is never laid on a frame");

    // The Picker Size keeps no flat-field; the Temperature dragged keeps all three as they stand.
    let before = kept();
    assert!(!p.invoke_press(3, false));
    p.invoke_drag(3, 30.0, 200.0, false);
    p.invoke_release(3);
    rig.pump();
    assert_eq!(kept(), before, "twin: another slider keeps nothing of the flat-field");
    assert!(!p.invoke_press(1, false));
    p.invoke_drag(1, 40.0, 200.0, false);
    p.invoke_release(1);
    rig.pump();
    let now = kept().and_then(|v| v.as_array().cloned()).expect("the sliders kept");
    assert!(now[1].as_f64().is_some_and(|t| (t - 0.4).abs() > 0.01), "the Temperature dragged is kept: {now:?}");
    assert_eq!((now[0].as_f64(), now[2].as_f64()), (Some(f64::from(FlatField::default().amount)), Some(f64::from(-0.3_f32))));
    rig.key("\u{1b}");
    assert!(!p.get_shown());

    // A roll whose first frame was dialled keeps its own colour.
    prepare("E.tif", 2);
    p.invoke_advance();
    rig.until("R2's drafts written", Duration::from_secs(10), || loaded(&path("R2", "F.tif")).roll_draft.is_some());
    let own = loaded(&path("R2", "F.tif")).roll_draft.unwrap().flat_field.unwrap();
    assert_eq!((own.temperature, own.tint), (0.2, 0.0), "a frame's own sliders win over the ones kept");
    rig.key("\u{1b}");

    // C flagged and D left in, both unreadable by the measure: C is written as Swift writes it, D left.
    prepare("A.tif", 4);
    p.invoke_step_picked(3);
    rig.pump();
    p.invoke_sheet_clicked(2);
    rig.pump();
    assert!(p.get_cells().row_data(2).unwrap().flagged);
    for n in ["C.tif", "D.tif"] {
        std::fs::write(path("R1", n), b"no longer a picture").unwrap();
    }
    p.invoke_step_picked(4);
    rig.pump();
    assert_eq!(p.get_action_title().as_str(), "MEASURE 3 FRAMES");
    p.invoke_advance();
    rig.until("the measure landed", Duration::from_secs(120), || !p.get_shown());
    let advice = w.global::<oni_app::Advice>();
    assert_eq!(advice.get_message().as_str(), "1 of 4 frames could not be measured and were left as they were.");
    let id = |n: &str| loaded(&path("R1", n)).settings.roll_reference.map(|r| r.id);
    assert!(id("B.tif").is_some(), "the roll measured");
    assert_eq!(id("C.tif"), id("B.tif"), "the flagged frame rides on the roll, read or not");
    assert_eq!(id("D.tif"), None, "twin: a frame left in that cannot be read is left as it was");
    let _ = std::fs::remove_dir_all(&root);
}
