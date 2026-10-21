//! Edit Together on the headless window: three frames selected open six to a row, a drag on one sets
//! the same value on all three as one step, a frame left out keeps its own.

use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{Canvas, Library, Store as StoreGlobal};
use oni_core::{Archive, EditorStore};
use oni_settings::sidecar;
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

fn exposure(path: &Path) -> f32 {
    sidecar::read(path, false, true).map(|l| l.settings.print.exposure).unwrap_or_default()
}

pub(crate) fn frames_opened_together_take_one_drag_as_one_step_and_one_left_out_keeps_its_own() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: Edit Together is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-together-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 0.9), ("B.tif", 0.7), ("C.tif", 0.5), ("D.tif", 0.3)] {
        common::graded_tiff(&folder.join(name), 150, 100, tone);
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
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, canvas) = (w.global::<Library>(), w.global::<Canvas>());
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
    // Each tile drawn once its source lands: the third cell's centre is a picture, not the ground the
    // gap between two tiles shows.
    let at = |x: usize, y: usize| canvas.get_frame().to_rgba8().filter(|b| b.width() as usize > x).map(|b| b.as_slice()[y * b.width() as usize + x]);
    // Within the dither's step: two grounds differ by a code at most.
    let near = |a: Option<slint::Rgba8Pixel>, b: Option<slint::Rgba8Pixel>| a.zip(b).is_some_and(|(a, b)| [a.r.abs_diff(b.r), a.g.abs_diff(b.g), a.b.abs_diff(b.b)].iter().all(|d| *d <= 2));
    rig.until("the grid", Duration::from_secs(60), || canvas.get_status().as_str() == "3 frames together" && !near(at(2 * 312 + 150, 150), at(306, 150)));
    assert_eq!(canvas.get_frame().size().width, 3 * 300 + 2 * 12, "three tiles on one row");
    assert!(near(at(306, 150), at(150, 10)), "twin: a gap and a cell's empty edge show the same ground");
    rig.settle();
    println!("shot {}", rig.shot("together-grid").display());

    let g = w.global::<StoreGlobal>();
    let field = Field::Exposure.index();
    let left_out = exposure(&path("D"));
    g.invoke_press(field, false);
    for k in 1..=8 {
        g.invoke_drag(field, k as f32 * 5.0, 300.0, false);
        rig.pump();
    }
    g.invoke_release(field);
    rig.pump();
    let set = ctl.state.borrow().store().settings().print.exposure;
    rig.until("the others written", Duration::from_secs(10), || ["A", "C"].iter().all(|n| (exposure(&path(n)) - set).abs() < 1e-6));
    assert_eq!(exposure(&path("D")), left_out, "twin: a frame not selected keeps its own");
    ctl.act("undo");
    rig.pump();
    let back = ctl.state.borrow().store().settings().print.exposure;
    assert!((back - set).abs() > 1e-6, "the drag was undone");
    for n in ["A", "C"] {
        assert!((exposure(&path(n)) - back).abs() < 1e-6, "{n}: one undo takes every frame back");
    }

    // Back to the gallery and into one frame: the grid gives the canvas back to that frame alone.
    ctl.act("toggle-mode");
    rig.pump();
    lib.invoke_tile_clicked(tile("D"), false, false);
    lib.invoke_tile_opened(tile("D"));
    rig.until("D alone", Duration::from_secs(60), || canvas.get_status().contains(" × "));
    assert!(ctl.state.borrow().store().editor_ref().is_some_and(|e| e.together().is_empty()));
    let _ = std::fs::remove_dir_all(&root);
}
