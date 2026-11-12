//! The view a hand moves renders lighter, and sharp once the hand has rested `SETTLE` on the timer's
//! own clock, whatever the real one reads.

use oni_app::Canvas;
use slint::ComponentHandle;
use std::time::Duration;

use crate::common;

/// `navigation::SETTLE`.
const SETTLE: Duration = Duration::from_millis(150);

pub(crate) fn the_view_renders_sharp_once_the_hand_has_rested_on_the_timers_clock() {
    let Some((rig, shell, root)) = crate::panel_bindings::opened("hand-rest") else { return };
    let ctl = shell.ctl.clone();
    let canvas = shell.window.global::<Canvas>();
    let coarse = || ctl.state.borrow().coarse();
    let zoom = |rig: &common::Rig| {
        let before = ctl.state.borrow().viewport();
        assert!(canvas.invoke_scrolled(0.0, -40.0, 300.0, 300.0, true), "the wheel reached the canvas");
        rig.pump();
        assert_ne!(ctl.state.borrow().viewport(), before, "the wheel zoomed");
    };
    assert!(!coarse(), "the opened frame is asked sharp");
    zoom(&rig);
    assert!(coarse(), "twin: a moving hand's view is asked lighter");
    // A second move before the rest: the rest counts from it, never from the first.
    rig.advance(SETTLE - Duration::from_millis(50));
    zoom(&rig);
    rig.advance(Duration::from_millis(50));
    assert!(coarse(), "twin: SETTLE after the first move, the hand having moved since, the view stays lighter");
    // Only the timer's clock reaches the rest: the real one has barely moved, and must not be read.
    rig.advance(SETTLE - Duration::from_millis(50));
    assert!(!coarse(), "rested SETTLE on the timer's clock, the view is asked sharp");
    let _ = std::fs::remove_dir_all(&root);
}
