//! The crop as one session on the headless window: Escape or its Cancel puts the photo back as the
//! tool found it and leaves, DONE or Return keeps its work, De-dust alike; the arrows move the frame,
//! the crop's readout and the status bar say one kept size, and its handles reach past their lines.

use oni_app::PanelTab;
use i_slint_backend_testing::{AccessibleRole as Role, ElementHandle};
use oni_app::{Canvas, Framing, PanelState};
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, SharedString};
use std::time::Duration;

use crate::common;
use crate::panel_bindings::opened;

/// A press as winit hands it over, moved by `by` on both axes in six steps, as a hand, then let go.
fn drag(rig: &common::Rig, w: &oni_app::MainWindow, (x, y): (f32, f32), by: f32) {
    let at = |step: f32| LogicalPosition::new(x + by * step / 6.0, y + by * step / 6.0);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: at(0.0) });
    oni_app::outside::pressed(w);
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: at(0.0), button: PointerEventButton::Left });
    for step in 1..=6 {
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: at(step as f32) });
        rig.pump();
    }
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: at(6.0), button: PointerEventButton::Left });
    rig.pump();
}

pub(crate) fn the_crops_handles_take_a_hand_past_their_slim_lines() {
    let Some((rig, shell, root)) = opened("crop-reach") else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (framing, c) = (w.global::<Framing>(), w.global::<Canvas>());
    let crop = || ctl.state.borrow().store().settings().geometry.crop;
    ctl.act(oni_app::Act::Framing);
    framing.invoke_ratio_picked(0);
    rig.settle();
    // The measuring window applied, so the crop's handles alone lie on the canvas.
    if framing.get_armed() {
        rig.key("\r");
    }
    assert!(c.get_cropping() && !c.get_window_shown());
    let origin = ElementHandle::find_by_element_type_name(w, "FrameOverlay").next().expect("the crop's frame").absolute_position();
    let frame = || (origin.x + c.get_crop_x(), origin.y + c.get_crop_y(), c.get_crop_width(), c.get_crop_height());
    // The drawn corner keeps its 22 points; twin: the grab around it reaches 12 past its line.
    let sizes = |name: &str| ElementHandle::find_by_element_type_name(w, name).map(|e| e.size()).collect::<Vec<_>>();
    let brackets = sizes("Bracket");
    assert!(brackets.len() == 4 && brackets.iter().all(|s| s.width <= 22.0 && s.height <= 22.0), "slim corners: {brackets:?}");
    let corners = sizes("Grab").into_iter().filter(|s| (s.width - 34.0).abs() < 0.5 && (s.height - 34.0).abs() < 0.5).count();
    assert_eq!(corners, 4, "each corner's grab is wider than its drawn bracket");

    // Ten points outside the top-left corner, within the hand's 12 past its line, the corner is held and
    // follows the hand all the way; the fraction counts up from the bottom, as Swift's.
    let found = crop();
    let (x, y, ..) = frame();
    drag(&rig, w, (x - 16.0, y - 16.0), 30.0);
    assert_eq!(crop(), found, "twin: beyond the hand's reach the press holds nothing");
    let (x, y, wd, ht) = frame();
    let points = |f: oni_settings::cg::Rect| (f.x * f64::from(wd) / found.width, f.height * f64::from(ht) / found.height);
    drag(&rig, w, (x - 10.0, y - 10.0), 30.0);
    let cornered = crop();
    let travel = (points(cornered).0 - points(found).0, points(found).1 - points(cornered).1);
    assert!((travel.0 - 30.0).abs() < 0.5 && (travel.1 - 30.0).abs() < 0.5, "the corner follows the hand: {travel:?}");
    assert!((cornered.x + cornered.width - found.x - found.width).abs() < 1e-6 && cornered.y == found.y, "its opposite corner stays");

    // Ten points outside the right edge's middle, its edge is held and follows every step of the hand,
    // the left one kept, and let go it says its kept size no more.
    let (x, y, wd, ht) = frame();
    drag(&rig, w, (x + wd + 16.0, y + ht / 2.0), -30.0);
    assert_eq!(crop(), cornered, "twin: beyond the hand's reach the press holds nothing");
    let (x, y, wd, ht) = frame();
    drag(&rig, w, (x + wd + 10.0, y + ht / 2.0), -30.0);
    let edged = crop();
    let travel = (cornered.width - edged.width) * f64::from(wd) / cornered.width;
    assert!((travel - 30.0).abs() < 0.5 && (edged.x - cornered.x).abs() < 1e-6, "the right edge follows the hand: {travel}");
    rig.settle();
    assert_eq!(c.get_crop_badge(), "", "let go, the frame's kept size leaves");
    rig.shot("crop-reach");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}

pub(crate) fn escape_or_cancel_puts_back_what_a_tool_changed_and_done_keeps_it() {
    let Some((rig, shell, root)) = opened("crop-session") else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (panel, framing, c) = (w.global::<PanelState>(), w.global::<Framing>(), w.global::<Canvas>());
    let escape: SharedString = Key::Escape.into();
    let geometry = || ctl.state.borrow().store().settings().geometry;
    let crop = |rig: &common::Rig| {
        ctl.act(oni_app::Act::Framing);
        rig.settle();
        assert_eq!(panel.get_tab(), PanelTab::Crop, "the crop opens");
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
    assert_eq!(panel.get_tab(), PanelTab::Color, "Escape leaves the crop");
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
    assert_eq!((panel.get_tab(), geometry()), (PanelTab::Color, found), "Cancel puts the frame back and leaves");
    crop(&rig);
    framing.invoke_act(oni_app::Act::Mirror);
    rig.settle();
    common::click(&rig, w, common::the(w, Role::Button, "DONE").middle());
    assert_eq!(panel.get_tab(), PanelTab::Color, "DONE leaves the crop");
    assert_ne!(geometry(), found, "twin: DONE keeps the mirror");
    let kept = geometry();
    crop(&rig);
    framing.invoke_act(oni_app::Act::Rotate);
    rig.settle();
    rig.key("\r");
    assert_eq!(panel.get_tab(), PanelTab::Color, "Return leaves the crop");
    assert_ne!(geometry(), kept, "twin: Return keeps the turn");

    // De-dust by the same rule: Escape takes back the correction painted, Return keeps one.
    let corrections = || ctl.state.borrow().store().settings().corrections.len();
    let paint = |rig: &common::Rig| {
        ctl.act(oni_app::Act::Correction);
        rig.settle();
        assert_eq!(panel.get_tab(), PanelTab::Dedust, "De-dust opens");
        let (fx, fy, fw, fh) = (c.get_frame_x(), c.get_frame_y(), c.get_frame_width(), c.get_frame_height());
        c.invoke_paint_start(fx + fw * 0.4, fy + fh * 0.4);
        c.invoke_paint_move(fx + fw * 0.5, fy + fh * 0.4);
        c.invoke_paint_end();
        rig.until("the stroke", Duration::from_secs(10), || corrections() == 1);
    };
    paint(&rig);
    rig.key(&escape);
    assert_eq!((panel.get_tab(), corrections()), (PanelTab::Color, 0), "Escape leaves De-dust, its correction taken back");
    paint(&rig);
    rig.key("\r");
    assert_eq!((panel.get_tab(), corrections()), (PanelTab::Color, 1), "twin: Return keeps the correction");
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
    let geometry = || ctl.state.borrow().store().settings().geometry;
    ctl.act(oni_app::Act::Framing);
    framing.invoke_ratio_picked(3);
    rig.settle();
    assert!(framing.get_armed(), "the window is drawn");
    let (at, window) = (crop(), geometry().measure_window);
    rig.key(&right);
    assert!(geometry().measure_window != window && crop() == at, "the arrow moves the window, the crop kept");
    rig.key("\r");
    rig.key("\r");
    assert_eq!(panel.get_tab(), PanelTab::Color, "Return applies the window, then leaves the crop");
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
    assert_eq!(kept, format!("{:.0} × 64 px", width().floor()), "the crop's readout says the whole pixels the cut keeps");
    let rendered = ctl.state.borrow().renders();
    rig.key("\r");
    rig.until("the cropped render", Duration::from_secs(60), || ctl.state.borrow().renders() > rendered);
    rig.settle();
    let status = c.get_status().to_string();
    assert!(status.ends_with(kept.trim_end_matches(" px")), "the status bar says the crop's size: {status} against {kept}");
    // The render's own extent is the size every readout says; twin: the crop rounded read a pixel more.
    let full = ctl.state.borrow().placed().expect("a placed render").full;
    assert_eq!(format!("{:.0} × {:.0} px", full.0, full.1), kept, "the render cuts what the readouts say");
    assert_ne!(format!("{:.0} × 64 px", width().round()), kept);
    assert_eq!(panel.get_tab(), PanelTab::Color);
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
