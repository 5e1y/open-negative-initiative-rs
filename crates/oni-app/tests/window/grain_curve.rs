//! The Grain Curve under Film Effects through the window's pointer: it opens folded, a drag of a point
//! writes the curve and renders the grain again, a double click returns the point to its rest and the
//! block's Reset the whole curve. Twin: with no Grain the same drag leaves the photo's pixels as they were.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::editors::{self, Plot};
use oni_app::{Canvas, Editors};
use oni_settings::blocks::GrainCurve;
use slint::{ComponentHandle, Model};
use std::time::Duration;

use crate::common;
use crate::editors::{drag, press, release};

/// The widest gap of two frames' codes, the display's dither being two at most.
fn worst(a: &[slint::Rgba8Pixel], b: &[slint::Rgba8Pixel]) -> u8 {
    a.iter().zip(b).map(|(a, b)| a.r.abs_diff(b.r).max(a.g.abs_diff(b.g)).max(a.b.abs_diff(b.b))).max().unwrap_or(0)
}

pub(crate) fn a_drag_on_the_grain_curve_moves_the_grain_and_a_double_click_rests_its_point() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the Grain Curve is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-grain-curve-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    common::graded_tiff(&root.join("Scans/A.tif"), 600, 400, 0.7);
    let (rig, shell) = crate::film_effects::opened(&gpu, &root);
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    rig.window.set_size(slint::PhysicalSize::new(1280, 2400));
    rig.settle();
    let g = w.global::<Editors>();
    let curve = || ctl.state.borrow().store().settings().grain.curve;
    let frame = || w.global::<Canvas>().get_frame().to_rgba8().expect("the canvas shows the photo").as_slice().to_vec();
    let pause = || std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));

    let title = common::placed(w, Role::Text, "Grain Curve");
    assert_eq!(title.len(), 1, "a digital photo lays the Grain Curve");
    assert!(g.get_grain_folded() && curve().rests() && !g.get_grain_modified(), "it opens folded, at rest");
    common::click(&rig, w, title[0].middle());
    rig.settle();
    assert!(!g.get_grain_folded(), "its title unfolds it");
    let size = editors::size(&ctl, Plot::Grain);
    assert!(size.w > 200.0 && size.h > 40.0, "the plot is laid out: {size:?}");
    assert_eq!(g.get_grain_dots().row_count(), 5, "its five points drawn");
    let origin = editors::origin(&ctl, Plot::Grain);
    let mid = GrainCurve::REST.points[2];
    let grab = (origin.0 + mid.x * size.w, origin.1 + (1.0 - mid.y) * size.h + 1.0);

    // Twin first: with no Grain a drag of the curve writes it and renders the photo its rest renders,
    // within the dither each render draws anew.
    press(&rig, grab);
    drag(&rig, grab, (0.0, 40.0), 10);
    release(&rig, (grab.0, grab.1 + 40.0));
    rig.settle();
    assert!(curve().points[2].y < 0.7 && g.get_grain_modified(), "the drag wrote the curve: {:?}", curve().points[2]);
    let bent = frame();
    let lowered = editors::origin(&ctl, Plot::Grain);
    let at = (lowered.0 + curve().points[2].x * size.w, lowered.1 + (1.0 - curve().points[2].y) * size.h);
    pause();
    for _ in 0..2 {
        press(&rig, at);
        release(&rig, at);
    }
    rig.settle();
    assert!(curve().rests() && !g.get_grain_modified(), "the double click rests the point: {:?}", curve());
    let gap = worst(&frame(), &bent);
    assert!(gap <= 2, "a curve with no Grain moved the photo by {gap} codes");

    // With Grain, the same drag renders the grain again.
    let renders = ctl.state.borrow().renders();
    crate::film_effects::press_rail(&rig, w, "Grain", 0.8);
    rig.until("the grain rendered", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    rig.settle();
    let (grained, renders) = (frame(), ctl.state.borrow().renders());
    pause();
    press(&rig, grab);
    drag(&rig, grab, (0.0, 60.0), 10);
    release(&rig, (grab.0, grab.1 + 60.0));
    rig.until("the curve rendered", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    rig.settle();
    assert!(curve().points[2].y < 0.5, "{:?}", curve().points[2]);
    let gap = worst(&frame(), &grained);
    println!("the curve moves the grained photo by {gap} codes at most");
    assert!(gap > 8, "the curve left the grain as it was: {gap} codes");
    println!("shot {}", rig.shot("grain-curve-moved").display());
    let reset: Vec<_> = common::placed(w, Role::Button, "Reset").into_iter().filter(|k| title[0].level_with(k)).collect();
    assert_eq!(reset.len(), 1, "one Reset on the row of Grain Curve");
    common::click(&rig, w, reset[0].middle());
    rig.settle();
    assert!(curve().rests() && !g.get_grain_modified(), "the Reset rests the curve");
    assert!(ctl.state.borrow().store().settings().grain.amount > 0.7, "and leaves Grain");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
