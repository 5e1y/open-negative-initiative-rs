//! The curves plot under the pointer: a drag, a double click and a folded block.

use super::*;

pub(crate) fn a_point_dragged_on_the_curves_reaches_the_store_as_one_step() {
    let Some(o) = opened("curves") else { return };
    let (rig, w, ctl): (&common::Rig, &MainWindow, _) = (&o.rig, &o.shell.window, o.shell.ctl.clone());
    let g = w.global::<Editors>();
    on_tab(&o, 2, |g| g.set_curves_folded(false));
    let origin = editors::origin(&ctl, Plot::Curves);
    let size = editors::size(&ctl, Plot::Curves);
    assert!(size.w > 200.0 && (size.w - size.h).abs() < 0.5, "the curves plot is a square: {size:?}");
    assert!(origin.1 + size.h < 2200.0, "the plot is on screen: {origin:?} {size:?}");
    println!("shot {}", rig.shot("editors-curves").display());

    // A press at the middle adds a point there; the drag carries it up by a fifth of the plot.
    let start = (origin.0 + size.w / 2.0, origin.1 + size.h / 2.0);
    let writes = editors::writes(&ctl);
    press(&rig, start);
    let curves = || ctl.state.borrow().store().settings().manual_curves.linked.clone();
    assert_eq!(curves().points.len(), 3, "the press added a point");
    // The editor draws the hand's value at once, ahead of the store's cadence.
    let lift = size.h / 5.0;
    rig.window.dispatch_event(WindowEvent::PointerMoved {
        position: LogicalPosition::new(start.0 as f32, (start.1 - lift / 2.0) as f32),
    });
    let drawn = g.get_curve_dots().row_data(1).unwrap();
    assert!((f64::from(drawn.y) - (size.h / 2.0 - lift / 2.0)).abs() < 0.5, "the dot follows the pointer: {}", drawn.y);
    assert!((curves().points[1].y - 0.5).abs() < 1e-3, "the store waits for the hand's cadence");
    // Thirty moves as fast as the pointer sends them: the store takes one per hand interval.
    let moves = 30;
    let began = std::time::Instant::now();
    for k in 1..=moves {
        let y = start.1 - lift / 2.0 - lift / 2.0 * k as f64 / moves as f64;
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(start.0 as f32, y as f32) });
        rig.pump();
    }
    let elapsed = began.elapsed();
    release(&rig, (start.0, start.1 - lift));
    rig.settle();
    let p = curves().points[1];
    assert!((p.x - 0.5).abs() < 1e-3 && (p.y - 0.7).abs() < 1e-3, "the point lands under the pointer: {p:?}");
    let written = editors::writes(&ctl) - writes;
    let paced = (elapsed.as_secs_f64() / oni_app::track::HAND_INTERVAL.as_secs_f64()).ceil() as usize + 3;
    assert!(written >= 2 && written <= paced, "{written} writes for {moves} moves in {elapsed:?}");
    // Twin: a write per pointer event would have reached the store once per move.
    assert!(written < moves, "{written} writes for {moves} moves in {elapsed:?}");
    assert!(g.get_curves_modified() && g.get_curve_marked().row_data(0) == Some(true), "the block shows its reset");

    // One undo takes the press and its drag back; a second finds nothing of theirs.
    w.global::<Bar>().invoke_act("undo".into());
    rig.settle();
    assert!(curves().is_neutral(), "one step took the whole drag back");
    assert!(!ctl.state.borrow().store().can_undo(), "the drag was a single step");

    let _ = std::fs::remove_dir_all(&o.dir);
}

pub(crate) fn a_double_click_on_a_point_removes_it_with_its_move_as_one_step() {
    let Some(o) = opened("double-click") else { return };
    let (rig, w, ctl): (&common::Rig, &MainWindow, _) = (&o.rig, &o.shell.window, o.shell.ctl.clone());
    on_tab(&o, 2, |g| g.set_curves_folded(false));
    let origin = editors::origin(&ctl, Plot::Curves);
    let size = editors::size(&ctl, Plot::Curves);
    assert!(size.w > 200.0 && (size.w - size.h).abs() < 0.5, "the curves plot is a square: {size:?}");
    assert!(origin.1 + size.h < 2200.0, "the plot is on screen: {origin:?} {size:?}");
    println!("shot {}", rig.shot("editors-curves").display());

    // A press at the middle adds a point there; the drag carries it up by a fifth of the plot.
    let start = (origin.0 + size.w / 2.0, origin.1 + size.h / 2.0);
    let curves = || ctl.state.borrow().store().settings().manual_curves.linked.clone();
    // A double click on a point: the second press lands inside the settle delay, so the move the
    // first made and the removal are one step, and one undo puts the point back where it stood.
    let undo = || {
        w.global::<Bar>().invoke_act("undo".into());
        rig.settle();
    };
    press(&rig, start);
    release(&rig, start);
    rig.settle();
    let off = (start.0 + 3.0, start.1);
    for _ in 0..2 {
        press(&rig, off);
        release(&rig, off);
    }
    assert!(curves().is_neutral(), "the double click removed the point");
    rig.settle();
    undo();
    assert!((curves().points[1].x - 0.5).abs() < 1e-6, "the move went with the removal: {:?}", curves().points);
    undo();
    assert!(curves().is_neutral());
    // Twin: the delay running out between the two presses leaves the move a step of its own.
    press(&rig, start);
    release(&rig, start);
    rig.settle();
    press(&rig, off);
    release(&rig, off);
    let rest = std::time::Instant::now();
    rig.until("the settle delay", Duration::from_secs(1), || rest.elapsed() > Duration::from_millis(250));
    press(&rig, off);
    release(&rig, off);
    assert!(curves().is_neutral(), "still a double click at 250 ms");
    rig.settle();
    undo();
    let moved = curves().points[1].x;
    assert!((moved - (off.0 - origin.0) / size.w).abs() < 1e-3, "the undo stopped at the move: {moved}");
    undo();
    undo();
    assert!(curves().is_neutral());

    let _ = std::fs::remove_dir_all(&o.dir);
}

pub(crate) fn a_folded_curves_block_takes_no_press() {
    let Some(o) = opened("folded") else { return };
    let (rig, w, ctl): (&common::Rig, &MainWindow, _) = (&o.rig, &o.shell.window, o.shell.ctl.clone());
    let g = w.global::<Editors>();
    on_tab(&o, 2, |g| g.set_curves_folded(false));
    let origin = editors::origin(&ctl, Plot::Curves);
    let size = editors::size(&ctl, Plot::Curves);
    assert!(size.w > 200.0 && (size.w - size.h).abs() < 0.5, "the curves plot is a square: {size:?}");
    assert!(origin.1 + size.h < 2200.0, "the plot is on screen: {origin:?} {size:?}");
    println!("shot {}", rig.shot("editors-curves").display());

    // A press at the middle adds a point there; the drag carries it up by a fifth of the plot.
    let start = (origin.0 + size.w / 2.0, origin.1 + size.h / 2.0);
    let curves = || ctl.state.borrow().store().settings().manual_curves.linked.clone();
    // Twin: a press on the folded block's place reaches no editor.
    g.set_curves_folded(true);
    rig.settle();
    press(&rig, start);
    release(&rig, start);
    rig.settle();
    assert!(curves().is_neutral(), "a folded plot took a press");
    let _ = std::fs::remove_dir_all(&o.dir);
}
