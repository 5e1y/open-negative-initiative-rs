//! A frame or a stroke held on the canvas ends with the overlay that draws it and with its photo:
//! Slint drops a grab with its overlay and sends no release, so nothing held outlives what shows it.

use oni_app::PanelTab;
use oni_app::{Act, Canvas, Framing, PanelState};
use slint::platform::Key;
use slint::{ComponentHandle, SharedString};
use std::time::Duration;

use crate::common;
use crate::panel_bindings::opened_with;

pub(crate) fn a_frame_held_ends_with_its_overlay_and_its_photo() {
    let seed = |support: &std::path::Path| common::plain_tiff(&support.with_file_name("Scans").join("B.tif"), 96, 64, 0.3);
    let Some((rig, shell, root)) = opened_with("frame-gesture", seed) else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (panel, framing, c) = (w.global::<PanelState>(), w.global::<Framing>(), w.global::<Canvas>());
    let escape: SharedString = Key::Escape.into();
    let right: SharedString = Key::RightArrow.into();
    let geometry = || ctl.state.borrow().store().settings().geometry.clone();

    // A stroke under way shows; twin of what follows: De-dust left mid-stroke and opened again shows none.
    ctl.act(Act::Correction);
    rig.settle();
    let (fx, fy, fw, fh) = (c.get_frame_x(), c.get_frame_y(), c.get_frame_width(), c.get_frame_height());
    c.invoke_paint_start(fx + fw * 0.4, fy + fh * 0.4);
    c.invoke_paint_move(fx + fw * 0.6, fy + fh * 0.4);
    assert_ne!(c.get_stroke(), "", "twin: the stroke under the hand is drawn");
    rig.key("\r");
    assert_eq!(panel.get_tab(), PanelTab::Color, "Return leaves De-dust, the hand still down");
    ctl.act(Act::Correction);
    rig.settle();
    assert_eq!(c.get_stroke(), "", "the stroke went with its overlay");
    rig.key("\r");

    // The crop alone drawn, its window applied.
    ctl.act(Act::Framing);
    rig.settle();
    if framing.get_armed() {
        rig.key("\r");
    }
    assert!(c.get_cropping() && !c.get_window_shown());
    // Twin: a frame held and drawn, Esc puts it back where the press found it, as the checks below would see.
    let found = geometry();
    c.invoke_frame_grabbed(false, 6);
    c.invoke_frame_dragged(false, 6, -20.0, 0.0);
    assert_ne!(geometry(), found, "the right edge follows the hand");
    rig.key(&escape);
    assert_eq!((panel.get_tab(), geometry()), (PanelTab::Crop, found.clone()), "twin: Esc under the hand puts the crop back");
    c.invoke_frame_released(false);

    // Return leaves the crop with the hand still down: the drag is its own step, nothing is held after.
    c.invoke_frame_grabbed(false, 6);
    c.invoke_frame_dragged(false, 6, -20.0, 0.0);
    let dragged = geometry();
    rig.key("\r");
    assert_eq!(panel.get_tab(), PanelTab::Color, "Return leaves the crop");
    // Past the history's coalescing window, so only a store still held could join the mirror to the drag.
    std::thread::sleep(Duration::from_secs_f64(oni_library::history::COALESCE_WINDOW + 0.1));
    framing.invoke_act(Act::Mirror);
    rig.settle();
    ctl.act(Act::Undo);
    rig.settle();
    assert_eq!(geometry(), dragged, "Undo takes back the mirror alone, the drag recorded as its overlay went");
    rig.key(&escape);
    assert_eq!(geometry(), dragged, "Esc beside the photo puts back no crop");
    ctl.act(Act::ToggleMode);
    ctl.act(Act::Framing);
    rig.settle();
    assert_eq!((panel.get_tab(), c.get_crop_badge()), (PanelTab::Crop, "".into()), "the crop again, no kept size above a frame no hand holds");
    rig.key("\r");

    // The window held on Color as the arrow walks to B: B keeps its own window under Esc.
    if !framing.get_armed() {
        framing.invoke_act(Act::WindowArm);
    }
    rig.settle();
    assert!(c.get_window_shown(), "the window is drawn on Color");
    // The first drag since the crop's Esc moves the window: that Esc spent no later drag.
    let proposed = geometry().measure_window;
    c.invoke_frame_grabbed(true, 0);
    c.invoke_frame_dragged(true, 0, 12.0, 8.0);
    c.invoke_frame_released(true);
    assert_ne!(geometry().measure_window, proposed, "the window follows the hand");
    c.invoke_frame_grabbed(true, 0);
    c.invoke_frame_dragged(true, 0, 4.0, 4.0);
    let rendered = ctl.state.borrow().renders();
    rig.key(&right);
    rig.until("B open", Duration::from_secs(60), || ctl.state.borrow().open_path().is_some_and(|p| p.ends_with("B.tif")));
    rig.until("B rendered", Duration::from_secs(120), || ctl.state.borrow().renders() > rendered);
    rig.settle();
    let own = geometry();
    rig.key(&escape);
    assert_eq!(geometry(), own, "Esc lays nothing of A's window on B");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
