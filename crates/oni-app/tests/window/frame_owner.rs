//! The open frame owns what was decoded and rendered of it: a frame trashed leaves no place or size
//! behind, and a preview decode landing for a frame left changes nothing of the open one.

use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::render::{Reading, Shared};
use oni_app::{App, Library, Store};
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::common;

/// `frames` written as plain TIFFs (name, width, height, tone), the first open in the editor and rendered.
fn opened(gpu: &oni_app::render::Device, label: &str, frames: &[(&str, usize, usize, f32)]) -> (common::Rig, Shell, PathBuf, Shared) {
    let root = std::env::temp_dir().join(format!("oni-app-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, w, h, tone) in frames {
        common::plain_tiff(&folder.join(name), *w, *h, *tone);
    }
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, engine.clone(), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    shell.window.global::<Library>().invoke_tile_opened(0);
    rig.until("the first frame rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    (rig, shell, root, engine)
}

pub(crate) fn a_trashed_frame_leaves_no_place_or_size_behind() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: a trashed frame's leftovers are not exercised"),
    };
    let (rig, shell, root, _) = opened(&gpu, "frame-trashed", &[("A.tif", 600, 400, 0.5)]);
    let ctl = shell.ctl.clone();
    let left = || {
        let s = ctl.state.borrow();
        (s.placed().is_some(), s.full_size().is_some())
    };
    assert_eq!(left(), (true, true), "twin: the open frame's render is placed and sized");
    ctl.act(oni_app::Act::Remove);
    rig.landed(&ctl);
    assert!(shell.window.global::<App>().get_gallery() && ctl.state.borrow().open_path().is_none(), "the last frame of its roll trashed, the gallery");
    assert_eq!(left(), (false, false), "nothing of the trashed frame's render stays");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}

pub(crate) fn a_preview_landing_for_a_frame_left_changes_nothing() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: a stale preview's landing is not exercised"),
    };
    // A narrow enough that its decode is its full resolution; B wider, so it has none, and of another shape.
    let frames = [("A.tif", 400, 300, 0.5), ("B.tif", oni_app::render::PREVIEW_SIDE + 48, 300, 0.5)];
    let (rig, shell, root, engine) = opened(&gpu, "frame-left", &frames);
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let store = w.global::<Store>();
    let pump_for = |ms: u64| {
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(ms) {
            rig.pump();
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    // The size the canvas lays after a notch and its way back, so it renders the negative the frame holds now.
    let notched = || {
        for by in [1, -1] {
            let n = ctl.state.borrow().renders();
            store.invoke_step(Field::Exposure.index(), by);
            rig.until("a notch's render", Duration::from_secs(60), || ctl.state.borrow().renders() > n);
        }
        ctl.state.borrow().full_size()
    };
    let a = ctl.state.borrow().open_path().unwrap().to_string();
    let a_reading = Reading::of(ctl.state.borrow().store().settings());
    let decoded = || {
        let (negative, full) = engine.prepare_keeping(oni_app::render::decode(Path::new(&a), a_reading.clone()).unwrap()).unwrap();
        assert!(full.is_some(), "A's decode is its full resolution");
        Ok((Arc::new(negative), full))
    };
    let (stale, misplaced) = (decoded(), decoded());

    ctl.act(oni_app::Act::ToggleMode);
    w.global::<Library>().invoke_tile_opened(1);
    rig.until("B rendered", Duration::from_secs(120), || ctl.state.borrow().open_path().is_some_and(|p| p.ends_with("B.tif")) && ctl.state.borrow().renders() >= 2);
    // Every decode place held, so B's own full resolution, built once it stayed a second, waits.
    let held = [oni_app::decode_gate::slot(), oni_app::decode_gate::slot()];
    let (a_size, b_size) = (Some((400.0, 300.0)), Some(((oni_app::render::PREVIEW_SIDE + 48) as f64, 300.0)));
    assert_eq!(ctl.state.borrow().full_size(), b_size, "B's render lays B's size");
    assert!(!ctl.state.borrow().full_resident(), "B's preview is no full resolution");

    ctl.opening_decoded(&a, &a_reading, stale);
    pump_for(200);
    assert!(!ctl.state.borrow().full_resident(), "A's full resolution is not kept under B");
    assert_eq!(notched(), b_size, "the canvas renders B's own negative");

    // Twin: the same decode landing under the open frame's path and reading is installed and seen.
    let b = ctl.state.borrow().open_path().unwrap().to_string();
    let b_reading = Reading::of(ctl.state.borrow().store().settings());
    ctl.opening_decoded(&b, &b_reading, misplaced);
    pump_for(200);
    assert!(ctl.state.borrow().full_resident(), "twin: a landing for the open frame keeps its full resolution");
    assert_eq!(notched(), a_size, "twin: a landing for the open frame reaches the canvas");
    drop(held);
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
