//! The arrows walking the editor in a burst: every step starts its walk from the picture the canvas shows,
//! the next photo's thumbnail read or not, each step taking over the one still moving.

use oni_app::{Canvas, Flight, Motion, Walk};
use slint::ComponentHandle;
use slint::platform::Key;

use crate::common;
use std::time::Duration;

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
        rig.advance(Duration::from_millis(60));
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

/// Red, green and blue summed, 0 to 765, averaged over the frame's inner part, clear of the walk's step either side.
fn inner_light(rig: &common::Rig, (x, y, width, height): (f32, f32, f32, f32)) -> f32 {
    let (w, px) = rig.drawn();
    let (mut sum, mut n) = (0u32, 0u32);
    for py in (((y + 60.0) as usize)..((y + height - 60.0) as usize)).step_by(20) {
        for qx in (((x + 60.0) as usize)..((x + width - 60.0) as usize)).step_by(20) {
            let v = px[py * w + qx].0;
            sum += u32::from((v >> 11) & 0x1f) * 255 / 31 + u32::from((v >> 5) & 0x3f) * 255 / 63 + u32::from(v & 0x1f) * 255 / 31;
            n += 1;
        }
    }
    sum as f32 / n.max(1) as f32
}

/// The walk as the real window draws it: its first frame comes once the loop has run its timers, and must still show
/// the walk near its start, then on its way; each instant a share of the walk's pace, `Motion.walk`.
pub(crate) fn a_walk_first_drawn_after_the_timers_ran_is_seen_from_its_start() {
    let root = common::Root::new("motion-walk-late");
    let folder = root.scans();
    // Two tones in turn: every step crosses a contrast.
    for (name, v) in [("A.tif", 0.15), ("B.tif", 0.85), ("C.tif", 0.15), ("D.tif", 0.85)] {
        common::pixels_tiff(&folder.join(name), 600, 400, |_, _| [v; 3]);
    }
    let Some(app) = common::launch(root).size(1400, 900).show() else { return };
    let common::App { rig, shell, .. } = &app;
    let w = &shell.window;
    app.edit_first();
    rig.settle();
    let (flight, canvas) = (w.global::<Flight>(), w.global::<Canvas>());
    let at = (flight.get_canvas_x() + canvas.get_frame_x(), flight.get_canvas_y() + canvas.get_frame_y(), canvas.get_frame_width(), canvas.get_frame_height());
    let right: slint::SharedString = Key::RightArrow.into();
    let left = inner_light(rig, at);
    let pace = w.global::<Motion>().get_walk() as u64;
    let ms = |share: u64| Duration::from_millis(pace * share / 100);
    // The walk to B, its first frame drawn a fifth of its pace after the key, another at half; the clock held, no
    // render lands, and at its end the canvas shows what the walk brought in.
    rig.freeze();
    w.invoke_refocus();
    rig.key(&right);
    rig.advance(ms(20));
    let early = inner_light(rig, at);
    rig.shot("motion-walk-late-early");
    rig.advance(ms(30));
    let mid = inner_light(rig, at);
    rig.shot("motion-walk-late-mid");
    rig.advance(Duration::from_millis(1000));
    let walked = inner_light(rig, at);
    println!("walk measured: left {left:.0}, a fifth {early:.0}, half {mid:.0}, walked to {walked:.0}");
    assert!((walked - left).abs() > 60.0, "the two pictures differ: {left:.0} and {walked:.0}");
    let along = |v: f32| (v - left) / (walked - left);
    let on_its_way = |v: f32| (0.1..0.9).contains(&along(v));
    assert!(along(early) < 0.25, "a fifth in, the picture left still covers the photo walked to: {:.2} of the way", along(early));
    assert!(on_its_way(mid), "halfway, both photos show, on their way: {:.2}", along(mid));
    // Twin: a walk drawn at its end at once, the photo walked to alone, is not on its way.
    assert!(!on_its_way(walked), "the photo at rest reads as on its way");
    // A burst: the walk to D, starting while the one to C still runs, begins again from the picture the canvas shows.
    w.invoke_refocus();
    rig.key(&right);
    rig.advance(ms(30));
    w.invoke_refocus();
    rig.key(&right);
    rig.advance(ms(10));
    let restarted = inner_light(rig, at);
    rig.advance(Duration::from_millis(1000));
    let ended = inner_light(rig, at);
    rig.thaw();
    println!("burst measured: a tenth into the walk to D {restarted:.0}, its end {ended:.0}");
    assert!((ended - restarted).abs() > 0.75 * (walked - left).abs(), "a tenth into the walk to D, the picture left still covers it");
}
