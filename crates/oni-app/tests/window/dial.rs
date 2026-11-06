//! The straighten's dial by the pointer: a press anywhere on its ring leaves the angle, and a drag
//! carries the scale with the hand, a drag right bringing the lower degrees under the index.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::PanelState;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition};

use crate::common;
use crate::panel_bindings::opened;

fn pointer(rig: &common::Rig, event: WindowEvent) {
    rig.window.dispatch_event(event);
    rig.pump();
}

pub(crate) fn the_straighten_dial_turns_with_the_hand_and_a_press_never_jumps_it() {
    let Some((rig, shell, root)) = opened("dial") else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    w.global::<PanelState>().set_tab(1);
    w.global::<PanelState>().invoke_tab_picked(1);
    rig.settle();
    let angle = || ctl.state.borrow().store().settings().geometry.angle;
    // The ring is the slider's lower part, under its name and value, as wide as the slider.
    let slider = common::the(w, Role::Slider, "Angle");
    let (middle, y) = (slider.x + slider.width / 2.0, slider.y + slider.height * 0.75);
    let start = LogicalPosition::new(middle + 40.0, y);
    rig.shot("dial-rest");

    // A press beside the index, on the ring: the angle stays where it was.
    pointer(&rig, WindowEvent::PointerMoved { position: start });
    pointer(&rig, WindowEvent::PointerPressed { position: start, button: PointerEventButton::Left });
    rig.settle();
    assert_eq!(angle(), 0.0, "a press on the ring lands nothing under the pointer");

    // The same hand moving right: the scale moves with it, a degree's marks the ring's radius times
    // its turn apart at the index, so the angle falls by the travel over that.
    let travel = 48.0;
    for step in 1..=8 {
        let at = LogicalPosition::new(start.x + travel * step as f32 / 8.0, y);
        pointer(&rig, WindowEvent::PointerMoved { position: at });
    }
    let end = LogicalPosition::new(start.x + travel, y);
    pointer(&rig, WindowEvent::PointerReleased { position: end, button: PointerEventButton::Left });
    rig.settle();
    // `DialStyle.turn`, nine degrees of the ring a degree of the photo.
    let degree = slider.width / 2.0 * 9f32.to_radians();
    let expected = -travel / degree;
    let turned = angle();
    rig.shot("dial-turned");
    assert!((turned - expected).abs() < 0.05, "the drag turned {turned}°, the scale under the hand asks {expected}°");
    // Twin: a rail's sense, its knob following the hand, would have raised the angle as much.
    assert!((turned + expected).abs() > 1.0, "a drag right must lower the angle, not raise it: {turned}°");
    let _ = std::fs::remove_dir_all(&root);
}
