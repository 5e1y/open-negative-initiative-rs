//! Frames together, each balanced its own way: a slider reads their span, a Temperature drag moves each
//! by the hand's distance and leaves every Tint, and the crop and De-dust keys say why they wait.

use i_slint_backend_testing::AccessibleRole;
use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{Canvas, Library, Store as StoreGlobal, Tip};
use oni_core::{Archive, EditorStore, Knob};
use oni_settings::PipelineSettings;
use oni_settings::sidecar::{self, WriteOptions};
use slint::{ComponentHandle, Model};
use slint::platform::WindowEvent;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

fn read(path: &Path) -> PipelineSettings {
    sidecar::read(path, false, true).map(|l| l.settings).unwrap_or_default()
}

pub(crate) fn frames_together_read_their_span_and_move_by_one_distance() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: Edit Together's moves are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-together-moves-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    // A and C balanced each its own way, as a balance measured frame by frame leaves them; B at rest.
    let balanced = |temperature: f32, tint: f32| Knob::Tint.set(&Knob::Temperature.set(&PipelineSettings::default(), temperature), tint);
    for (name, tone, own) in [("A", 0.9, Some(balanced(0.6, -0.3))), ("B", 0.7, None), ("C", 0.5, Some(balanced(-0.4, 0.2)))] {
        let path = folder.join(format!("{name}.tif"));
        common::graded_tiff(&path, 150, 100, tone);
        if let Some(own) = own {
            sidecar::write(&own, &path, WriteOptions::default()).unwrap();
        }
    }
    let folder = PathBuf::from(oni_library::paths::canonical(&folder));
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1728, 1017));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, canvas, g, tip) = (w.global::<Library>(), w.global::<Canvas>(), w.global::<StoreGlobal>(), w.global::<Tip>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    let tile = |name: &str| oni_app::app::tiles(w).iter().find(|t| t.caption == name).unwrap_or_else(|| panic!("{name}")).index;
    let path = |name: &str| folder.join(format!("{name}.tif"));
    lib.invoke_tile_clicked(tile("A"), false, false);
    lib.invoke_tile_clicked(tile("B"), true, false);
    lib.invoke_tile_clicked(tile("C"), true, false);
    rig.pump();
    lib.invoke_tile_opened(tile("B"));
    rig.until("the grid", Duration::from_secs(60), || canvas.get_status().as_str() == "3 frames together");
    rig.settle();

    // Before any change, a slider whose frames read apart shows their span; one they share, its value.
    let open = |k: Knob| k.get(ctl.state.borrow().store().settings());
    assert_eq!(g.get_temperature_readout().as_str(), Field::Temperature.span_readout(-0.4, 0.6), "B at rest between C and A");
    assert_eq!(g.get_tint_readout().as_str(), Field::Tint.span_readout(-0.3, 0.2));
    assert_eq!(g.get_vignette_readout().as_str(), Field::Vignette.readout(open(Knob::Vignette)), "twin: one value where the frames agree");

    // B at rest beside A and C balanced each its own way: Colour's Reset is live for them alone.
    let live = |block: &str| crate::help_card::reset_of(w, block).is_some();
    assert!(open(Knob::Temperature) == 0.0 && live("Colour"), "the open frame at rest, the others not");

    let field = Field::Temperature.index();
    g.invoke_press(field, false);
    for k in 1..=8 {
        g.invoke_drag(field, k as f32 * 5.0, 300.0, false);
        rig.pump();
    }
    g.invoke_release(field);
    rig.pump();
    let moved = open(Knob::Temperature);
    assert!(moved > 0.05, "the hand moved B: {moved}");
    let near = |a: f32, b: f32| (a - b).abs() < 1e-4;
    rig.until("the others written", Duration::from_secs(10), || near(Knob::Temperature.get(&read(&path("A"))), 0.6 + moved));
    let (a, c) = (read(&path("A")), read(&path("C")));
    assert!(near(Knob::Temperature.get(&c), -0.4 + moved), "C moved as far: {}", Knob::Temperature.get(&c));
    assert!(near(Knob::Tint.get(&a), -0.3) && near(Knob::Tint.get(&c), 0.2), "no frame's tint moved: {} {}", Knob::Tint.get(&a), Knob::Tint.get(&c));
    assert!(!near(Knob::Temperature.get(&a), moved), "twin: A keeps its offset, never B's value");
    assert_eq!(g.get_temperature_readout().as_str(), Field::Temperature.span_readout(-0.4 + moved, 0.6 + moved), "the span moved whole");
    println!("shot {}", rig.shot("together-moves").display());

    // Colour's Reset lays each frame's own rest on all three, then leaves.
    common::click(&rig, w, crate::help_card::reset_of(w, "Colour").expect("Colour's Reset").middle());
    rig.until("the others at rest", Duration::from_secs(10), || near(Knob::Temperature.get(&read(&path("A"))), 0.0));
    let (a, c) = (read(&path("A")), read(&path("C")));
    assert!(near(Knob::Tint.get(&a), 0.0) && near(Knob::Temperature.get(&c), 0.0) && near(Knob::Tint.get(&c), 0.0));
    rig.settle();
    assert!(!live("Colour"), "twin: every frame at rest, the key leaves");

    // The crop and De-dust, each frame's own, say under the hand why they wait.
    let hover = |label: &str| {
        let key = common::placed(w, AccessibleRole::Button, label).into_iter().find(|p| p.y >= 0.0 && p.width > 0.0).unwrap_or_else(|| panic!("the {label} key"));
        let (x, y) = key.middle();
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: slint::LogicalPosition::new(x, y) });
        rig.pump();
        let told = tip.get_text().to_string();
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: slint::LogicalPosition::new(x, y + 400.0) });
        rig.pump();
        told
    };
    assert_eq!(hover("Crop"), "Select one photo to crop it");
    assert_eq!(hover("De-dust"), "Select one photo to de-dust it");
    // Twin: B alone again, the crop's key tells what it does.
    let row = |n: &str| lib.get_rows().iter().position(|r| r.kind == 2 && r.title == n).unwrap_or_else(|| panic!("{n}'s row")) as i32;
    lib.invoke_row_clicked(row("B"), false, false);
    rig.until("B alone", Duration::from_secs(60), || canvas.get_status().contains(" \u{d7} "));
    assert!(hover("Crop").starts_with("Crop and Straighten"));
    assert_eq!(g.get_temperature_readout().as_str(), Field::Temperature.readout(open(Knob::Temperature)), "alone, one value again");
    let _ = std::fs::remove_dir_all(&root);
}
