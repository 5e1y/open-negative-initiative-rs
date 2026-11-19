//! Compare Before/After zoomed in: a pan, a resize and a pinch move the before with the after in the
//! same frame, while the pan inside the rendered margin asks no render that could carry it.

use i_slint_backend_testing::ElementHandle;
use oni_app::app::{Options, Shell};
use oni_app::{Canvas, Library};
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// Where the layout put the canvas's picture `id` (`after` or `before`), in window points.
fn rect(w: &oni_app::MainWindow, id: &str) -> [f32; 4] {
    let e = ElementHandle::find_by_element_id(w, &format!("CanvasCard::{id}")).next().unwrap_or_else(|| panic!("the {id} picture"));
    let (at, size) = (e.absolute_position(), e.size());
    [at.x, at.y, size.width, size.height]
}

fn near(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(&b).all(|(p, q)| (p - q).abs() <= 0.5)
}

pub(crate) fn a_pan_a_resize_and_a_pinch_move_the_before_with_the_after() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: Compare's pan is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-compare-pan-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    common::graded_tiff(&folder.join("A.tif"), 3000, 2000, 0.6);
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let canvas = w.global::<Canvas>();
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    let lib = w.global::<Library>();
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);

    // Compare on at 100 %, the full resolution built and every render landed.
    ctl.act(oni_app::Act::Compare);
    rig.until("the before", Duration::from_secs(60), || canvas.get_before().size().width > 0);
    canvas.invoke_double_clicked();
    rig.until("the full resolution", Duration::from_secs(120), || ctl.state.borrow().full_built());
    let quiet = || loop {
        let n = ctl.state.borrow().renders();
        std::thread::sleep(Duration::from_millis(400));
        rig.settle();
        if ctl.state.borrow().renders() == n {
            break;
        }
    };
    quiet();
    assert_eq!(ctl.state.borrow().viewport().scale, Some(1.0));
    assert!(ctl.state.borrow().placed().is_some_and(|p| !p.whole), "a part of the frame, which a pan moves");
    let (after, before) = (rect(w, "after"), rect(w, "before"));
    assert!(near(after, before), "at rest the before lies on the after: {after:?} {before:?}");

    // A pan inside the margin: no render lands, the before moves with the after all the same.
    let renders = ctl.state.borrow().renders();
    canvas.invoke_grabbed(400.0, 300.0);
    for k in 1..=4 {
        canvas.invoke_dragged(400.0 - 10.0 * k as f32, 300.0 - 5.0 * k as f32);
    }
    rig.pump();
    assert_eq!(ctl.state.borrow().renders(), renders, "the pan is covered: nothing landed");
    let (panned, panned_before) = (rect(w, "after"), rect(w, "before"));
    assert!((panned[0] - after[0]).abs() >= 30.0 && (panned[1] - after[1]).abs() >= 15.0, "twin: the after moved {after:?} → {panned:?}");
    assert!(!near(panned_before, before), "twin: the before left its place");
    assert!(near(panned, panned_before), "after a pan the before lies on the after: {panned:?} {panned_before:?}");
    println!("shot {}", rig.shot("compare-pan").display());

    // A resize and a pinch step, laid by arithmetic before any render lands.
    rig.window.set_size(slint::PhysicalSize::new(1180, 760));
    rig.pump();
    let (resized, resized_before) = (rect(w, "after"), rect(w, "before"));
    assert!(near(resized, resized_before), "after a resize: {resized:?} {resized_before:?}");
    let (cx, cy) = (canvas.get_width() / 2.0, canvas.get_height() / 2.0);
    canvas.invoke_pinch_started();
    canvas.invoke_pinched(1.1, cx, cy);
    rig.pump();
    let (pinched, pinched_before) = (rect(w, "after"), rect(w, "before"));
    assert!(pinched[2] > resized[2] * 1.05, "twin: the pinch grew the picture {resized:?} → {pinched:?}");
    assert!(near(pinched, pinched_before), "after a pinch: {pinched:?} {pinched_before:?}");

    // Every render landed: the before still lies on the after, and Compare off lets it go.
    quiet();
    assert!(near(rect(w, "after"), rect(w, "before")), "after the renders landed");
    ctl.act(oni_app::Act::Compare);
    rig.pump();
    assert!(canvas.get_split() < 0.0 && canvas.get_before().size().width == 0, "Compare off lets the before go");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
