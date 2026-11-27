//! The arrows walking the editor in a burst: every step starts its walk from the picture the canvas shows,
//! the next photo's thumbnail read or not, each step taking over the one still moving.

use oni_app::{Canvas, Motion, Walk};
use slint::ComponentHandle;
use slint::platform::Key;

use crate::common;

pub(crate) fn every_arrow_in_a_burst_walks_from_the_picture_the_canvas_shows() {
    let root = common::Root::new("motion-walk");
    let folder = root.scans();
    for (i, name) in ["A.tif", "B.tif", "C.tif", "D.tif", "E.tif", "F.tif"].iter().enumerate() {
        common::graded_tiff(&folder.join(name), 600, 400, 0.2 + 0.12 * i as f32);
    }
    let Some(app) = common::launch(root).size(1400, 900).show() else { return };
    let common::App { rig, shell, .. } = &app;
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    app.edit_first();
    rig.settle();
    let (walk, canvas) = (w.global::<Walk>(), w.global::<Canvas>());
    let right: slint::SharedString = Key::RightArrow.into();
    // No thumbnail in memory, as on a photo the filmstrip has not asked for yet.
    ctl.state.borrow_mut().forget_tiles();
    rig.freeze();
    for step in 1..=5 {
        let (serial, shown) = (walk.get_serial(), canvas.get_frame().size());
        w.invoke_refocus();
        rig.key(&right);
        assert!(walk.get_serial() == serial + 1 && walk.get_walking(), "step {step}: the walk starts");
        assert_eq!(walk.get_picture().size(), shown, "step {step}: what steps aside is what the canvas showed");
        rig.advance(std::time::Duration::from_millis(60));
    }
    rig.thaw();
    rig.settle();
    assert!(ctl.state.borrow().open_path().is_some_and(|p| p.ends_with("F.tif")));
    // Twins: the roll's end walks nowhere, and under Reduce Motion a step lands at once.
    let serial = walk.get_serial();
    w.invoke_refocus();
    rig.key(&right);
    assert_eq!(walk.get_serial(), serial, "past the last photo nothing starts");
    w.global::<Motion>().set_reduced(true);
    let left: slint::SharedString = Key::LeftArrow.into();
    w.invoke_refocus();
    rig.key(&left);
    assert!(ctl.state.borrow().open_path().is_some_and(|p| p.ends_with("E.tif")) && walk.get_serial() == serial, "Reduce Motion walks without the slide");
}
