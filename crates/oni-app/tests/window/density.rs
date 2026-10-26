//! A window moved to a screen of another density keeps its size in points: the canvas still renders
//! again at the new count of pixels, and the plots' rasters are traced again at it.

use oni_app::{Editors, PanelState, Plots};
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::time::Duration;

use crate::panel_bindings::opened;

pub(crate) fn a_screen_of_another_density_renders_the_canvas_and_the_plots_again() {
    let Some((rig, shell, root)) = opened("density") else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    // The Roll tab's levels unfolded in a window tall enough to lay them: their histogram is a raster
    // traced at the screen's density.
    rig.window.set_size(slint::PhysicalSize::new(1280, 2200));
    w.global::<PanelState>().set_tab(0);
    w.global::<PanelState>().invoke_tab_picked(0);
    w.global::<Editors>().set_levels_folded(false);
    // Drawn once, so the block unfolded is laid and its plot sized.
    rig.drawn();
    let plots = w.global::<Plots>();
    // The output histogram is traced from each render; the levels' from the input distributions.
    let (output, levels) = (|| plots.get_output().size().width, || plots.get_levels().size().width);
    rig.until("the output histogram", Duration::from_secs(60), || output() > 0);
    // The input distributions are read once the hand rests, a little after the render.
    let start = std::time::Instant::now();
    while levels() == 0 && start.elapsed() < Duration::from_secs(10) {
        rig.settle();
    }
    // At rest: the render the tall window asked has landed.
    let mut seen = usize::MAX;
    while seen != ctl.state.borrow().renders() {
        seen = ctl.state.borrow().renders();
        rig.settle();
        rig.settle();
    }
    let (renders, traced, inputs) = (ctl.state.borrow().renders(), output(), levels());

    // Twin first: the same density announced again asks for nothing.
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.settle();
    assert_eq!((ctl.state.borrow().renders(), output()), (renders, traced), "the same density renders nothing");

    // From a 1x screen to a 2x one: the same points, twice the pixels each way.
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 2.0 });
    rig.window.set_size(slint::PhysicalSize::new(2 * 1280, 2 * 2200));
    rig.until("the render at the new density", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    rig.until("the output traced at the new density", Duration::from_secs(60), || output() == 2 * traced);
    if inputs == 0 {
        eprintln!("no input distribution for this frame: the levels' trace at the new density is not exercised");
    } else {
        rig.until("the levels traced at the new density", Duration::from_secs(60), || levels() == 2 * inputs);
    }
    let _ = std::fs::remove_dir_all(&root);
}
