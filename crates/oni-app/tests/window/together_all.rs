//! Edit Together over five frames opened every way a hand opens them: each frame takes each step.

use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{Canvas, Library, Store as StoreGlobal};
use oni_core::{Archive, EditorStore};
use oni_settings::sidecar;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

fn exposure(path: &Path) -> f32 {
    sidecar::read(path, false, true).map(|l| l.settings.print.exposure).unwrap_or_default()
}

pub(crate) fn every_frame_together_takes_every_step_whichever_way_they_were_opened() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: Edit Together is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-together-all-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    let names = ["A", "B", "C", "D", "E"];
    for (i, n) in names.iter().enumerate() {
        common::graded_tiff(&folder.join(format!("{n}.tif")), 150, 100, 0.3 + 0.1 * i as f32);
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
    let (lib, canvas, g) = (w.global::<Library>(), w.global::<Canvas>(), w.global::<StoreGlobal>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    let tile = |name: &str| oni_app::app::tiles(w).iter().find(|t| t.caption == name).unwrap_or_else(|| panic!("{name}")).index;
    let path = |name: &str| folder.join(format!("{name}.tif"));
    let field = Field::Exposure.index();
    let mut notch = 0.0f32;
    let mut drag = |label: &str| {
        notch += 25.0;
        g.invoke_press(field, false);
        for k in 1..=5 {
            g.invoke_drag(field, notch * k as f32 / 5.0, 300.0, false);
            rig.pump();
        }
        g.invoke_release(field);
        rig.pump();
        shell.flush();
        let set = ctl.state.borrow().store().settings().print.exposure;
        let open = ctl.state.borrow().open_path().map(str::to_string).unwrap_or_default();
        let lagging: Vec<&str> = names.iter().copied().filter(|n| !open.ends_with(&format!("{n}.tif")) && (exposure(&path(n)) - set).abs() > 1e-6).collect();
        println!("{label}: open {open}, set {set}, lagging {lagging:?}, {:?}", names.iter().map(|n| exposure(&path(n))).collect::<Vec<_>>());
        lagging.len()
    };

    // The gallery: a click then ⌘-clicks, a double click on the third.
    lib.invoke_tile_clicked(tile("A"), false, false);
    for n in ["B", "C", "D", "E"] {
        lib.invoke_tile_clicked(tile(n), true, false);
    }
    rig.pump();
    lib.invoke_tile_opened(tile("C"));
    rig.until("five together", Duration::from_secs(30), || canvas.get_status().as_str() == "5 frames together");
    let first = drag("gallery, opened on C");
    let second = drag("gallery, a second drag");
    // Back to the gallery and in again with the editor's key.
    ctl.act(oni_app::Act::ToggleMode);
    rig.pump();
    ctl.act(oni_app::Act::ToggleMode);
    rig.until("five together again", Duration::from_secs(30), || canvas.get_status().as_str() == "5 frames together");
    let third = drag("back in by the key");
    // The column: D alone, then ⌘ on A and E.
    let row = |n: &str| lib.get_rows().iter().position(|r| r.kind == 2 && r.title == n).unwrap_or_else(|| panic!("{n}'s row")) as i32;
    lib.invoke_row_clicked(row("D"), false, false);
    rig.until("D alone", Duration::from_secs(30), || canvas.get_status().contains(" × "));
    lib.invoke_row_clicked(row("A"), true, false);
    lib.invoke_row_clicked(row("E"), true, false);
    rig.until("three together", Duration::from_secs(30), || canvas.get_status().as_str() == "3 frames together");
    let fourth = drag("column, D then ⌘A ⌘E");
    assert_eq!((first, second, third, fourth), (0, 0, 0, 0), "a frame together lags behind the step");
    let _ = std::fs::remove_dir_all(&root);
}
