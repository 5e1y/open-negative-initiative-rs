//! Lighting's colour compensation in the headless window: on at rest, its switch shown on a legacy print's own curve
//! alone, turning it off stores it and renders the frame again; the Saturation Curve's plot draws no print's own curve.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::fields::Field;
use oni_app::{Editors, PanelState, Store};
use slint::{ComponentHandle, Model};
use std::time::Duration;

use crate::common;

pub(crate) fn the_switch_renders_again_and_lays_no_dashed_guide() {
    let root = common::Root::new("compensation");
    let folder = root.scans();
    common::plain_tiff(&folder.join("A.tif"), 96, 64, 0.6);
    let Some(app) = common::launch(root).size(1280, 1600).show() else { return };
    let common::App { rig, shell, .. } = &app;
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (panel, editors) = (w.global::<PanelState>(), w.global::<Editors>());
    app.edit_first();
    panel.invoke_film_type_picked(0);
    editors.set_saturation_folded(false);
    rig.pump();
    let settings = || ctl.state.borrow().store().settings().clone();
    // The neutral line and the hand's curve: Contrast acts on the negative and lays no curve of its own.
    let lines = || editors.get_saturation_lines().row_count();

    // At rest the compensation is on; on a paper, which every frame but a legacy print prints on, its switch is not shown.
    assert!(settings().print.colour_compensation && panel.get_colour_compensation());
    assert!(common::placed(w, Role::Checkbox, "Colour compensation").is_empty(), "no switch on a paper");
    w.global::<Store>().invoke_step(Field::Contrast.index(), 6);
    rig.pump();
    assert_ne!(settings().print.contrast, 0.0);
    assert_eq!(lines(), 2, "no dashed line: the negative lays Contrast's colour");

    let renders = ctl.state.borrow().renders();
    panel.invoke_colour_compensation_toggled(false);
    rig.pump();
    assert!(!settings().print.colour_compensation && !panel.get_colour_compensation());
    rig.until("the frame rendered without the compensation", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    assert_eq!(lines(), 2);
    // Twin: back on, the frame renders again.
    let renders = ctl.state.borrow().renders();
    panel.invoke_colour_compensation_toggled(true);
    rig.pump();
    assert!(settings().print.colour_compensation);
    rig.until("the frame rendered with the compensation", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
}
