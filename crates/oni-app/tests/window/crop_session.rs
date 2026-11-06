//! The crop as one session on the headless window: Escape or its Cancel puts the photo back as the
//! tool found it and leaves, DONE or Return keeps its work, De-dust alike; the arrows move the frame,
//! and the crop's readout and the status bar say one kept size.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{Canvas, Framing, PanelState};
use slint::platform::Key;
use slint::{ComponentHandle, SharedString};
use std::time::Duration;

use crate::common;
use crate::panel_bindings::opened;

pub(crate) fn escape_or_cancel_puts_back_what_a_tool_changed_and_done_keeps_it() {
    let Some((rig, shell, root)) = opened("crop-session") else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (panel, framing, c) = (w.global::<PanelState>(), w.global::<Framing>(), w.global::<Canvas>());
    let escape: SharedString = Key::Escape.into();
    let geometry = || ctl.state.borrow().store().settings().geometry.clone();
    let crop = |rig: &common::Rig| {
        ctl.act(oni_app::Act::Framing);
        rig.settle();
        assert_eq!(panel.get_tab(), 1, "the crop opens");
    };
    let entry = geometry();

    // Escape: the ratio and the turn made in the crop are put back, and the crop is left.
    crop(&rig);
    framing.invoke_ratio_picked(3);
    framing.invoke_act(oni_app::Act::Rotate);
    rig.settle();
    let worked = geometry();
    assert_ne!(worked, entry);
    rig.key(&escape);
    assert_eq!(panel.get_tab(), 2, "Escape leaves the crop");
    assert_eq!(geometry(), entry, "and puts back the frame the crop found");
    // The putting back is a step of its own: Undo brings the crop's work again.
    ctl.act(oni_app::Act::Undo);
    rig.settle();
    assert_eq!(geometry(), worked, "Undo takes the cancel back");

    // The header shows both ways out: Cancel does as Escape, DONE keeps.
    crop(&rig);
    let found = geometry();
    framing.invoke_act(oni_app::Act::Mirror);
    rig.settle();
    common::click(&rig, w, common::the(w, Role::Button, "Cancel").middle());
    assert_eq!((panel.get_tab(), geometry()), (2, found), "Cancel puts the frame back and leaves");
    crop(&rig);
    framing.invoke_act(oni_app::Act::Mirror);
    rig.settle();
    common::click(&rig, w, common::the(w, Role::Button, "DONE").middle());
    assert_eq!(panel.get_tab(), 2, "DONE leaves the crop");
    assert_ne!(geometry(), found, "twin: DONE keeps the mirror");
    let kept = geometry();
    crop(&rig);
    framing.invoke_act(oni_app::Act::Rotate);
    rig.settle();
    rig.key("\r");
    assert_eq!(panel.get_tab(), 2, "Return leaves the crop");
    assert_ne!(geometry(), kept, "twin: Return keeps the turn");

    // De-dust by the same rule: Escape takes back the correction painted, Return keeps one.
    let corrections = || ctl.state.borrow().store().settings().corrections.len();
    let paint = |rig: &common::Rig| {
        ctl.act(oni_app::Act::Correction);
        rig.settle();
        assert_eq!(panel.get_tab(), 4, "De-dust opens");
        let (fx, fy, fw, fh) = (c.get_frame_x(), c.get_frame_y(), c.get_frame_width(), c.get_frame_height());
        c.invoke_paint_start(fx + fw * 0.4, fy + fh * 0.4);
        c.invoke_paint_move(fx + fw * 0.5, fy + fh * 0.4);
        c.invoke_paint_end();
        rig.until("the stroke", Duration::from_secs(10), || corrections() == 1);
    };
    paint(&rig);
    rig.key(&escape);
    assert_eq!((panel.get_tab(), corrections()), (2, 0), "Escape leaves De-dust, its correction taken back");
    paint(&rig);
    rig.key("\r");
    assert_eq!((panel.get_tab(), corrections()), (2, 1), "twin: Return keeps the correction");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}

pub(crate) fn the_arrows_move_the_crop_and_every_readout_says_one_kept_size() {
    let Some((rig, shell, root)) = opened("crop-arrows") else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (panel, framing, c) = (w.global::<PanelState>(), w.global::<Framing>(), w.global::<Canvas>());
    let crop = || ctl.state.borrow().store().settings().geometry.crop;
    let right: SharedString = Key::RightArrow.into();

    // A frame never balanced opens with its measuring window drawn: the arrows move that frame first,
    // as Return applies it first.
    let geometry = || ctl.state.borrow().store().settings().geometry.clone();
    ctl.act(oni_app::Act::Framing);
    framing.invoke_ratio_picked(3);
    rig.settle();
    assert!(framing.get_armed(), "the window is drawn");
    let (at, window) = (crop(), geometry().measure_window);
    rig.key(&right);
    assert!(geometry().measure_window != window && crop() == at, "the arrow moves the window, the crop kept");
    rig.key("\r");
    rig.key("\r");
    assert_eq!(panel.get_tab(), 2, "Return applies the window, then leaves the crop");
    // 1:1 on the 3:2 photo leaves the crop room sideways; on Colour an arrow walks the photos, the frame kept.
    rig.key(&right);
    assert_eq!(crop(), at, "twin: beside a pane the arrow leaves the frame");

    // In the crop an arrow moves the frame, Shift ten times as far.
    ctl.act(oni_app::Act::Framing);
    rig.settle();
    rig.key(&right);
    let one = crop().x - at.x;
    assert!(one > 0.0 && (crop().width - at.width).abs() < 1e-9, "the arrow moves the frame right, its size kept: {one}");
    rig.chord(Key::Shift, &right);
    let ten = crop().x - at.x - one;
    assert!((ten / one - 10.0).abs() < 0.01, "Shift moves it ten times as far: {ten} against {one}");
    let left: SharedString = Key::LeftArrow.into();
    rig.key(&left);
    assert!((crop().x - at.x - ten).abs() < 1e-9, "the left arrow brings one step back");

    // Free, the right edge drawn in until the crop ends past half a pixel, which the render's cut drops.
    framing.invoke_ratio_picked(0);
    rig.settle();
    let width = || crop().width * 96.0;
    c.invoke_frame_grabbed(false, 6);
    let mut travel = 0.0;
    while !(0.55..0.95).contains(&width().fract()) {
        travel -= 0.25;
        assert!(travel > -60.0, "no drag left a fraction of a pixel: {}", width());
        c.invoke_frame_dragged(false, 6, travel, 0.0);
    }
    c.invoke_frame_released(false);
    let kept = framing.get_kept().to_string();
    assert_eq!(kept, format!("{:.0} × 64 px", width().round()), "the crop's readout rounds the crop");
    let rendered = ctl.state.borrow().renders();
    rig.key("\r");
    rig.until("the cropped render", Duration::from_secs(60), || ctl.state.borrow().renders() > rendered);
    rig.settle();
    let status = c.get_status().to_string();
    assert!(status.ends_with(kept.trim_end_matches(" px")), "the status bar says the crop's size: {status} against {kept}");
    // Twin: the render's own extent, which the status bar once scaled back, is a pixel short.
    let full = ctl.state.borrow().placed().expect("a placed render").full;
    assert_ne!(format!("{:.0} × {:.0} px", full.0, full.1), kept, "the render's extent rounds otherwise");
    assert_eq!(panel.get_tab(), 2);
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
