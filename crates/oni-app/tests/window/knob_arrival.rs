//! A block's reset springs every knob it moves home on one timer, each landing on its target within
//! `settle`'s settling time, the timer stopped once the last rests.

use oni_app::fields::{Block, Field};
use oni_app::motion::Spring;
use oni_app::slider_track::{position_on, track_on};
use oni_app::Store;
use slint::ComponentHandle;
use std::time::{Duration, Instant};

pub(crate) fn a_blocks_reset_springs_every_knob_home_on_one_timer() {
    let Some((rig, shell, root)) = crate::panel_bindings::opened("knob-arrival") else { return };
    let (ctl, g) = (shell.ctl.clone(), shell.window.global::<Store>());
    let lighting = Block::ALL.iter().position(|b| *b == Block::Lighting).unwrap() as i32;
    let stepped = [Field::Exposure, Field::Contrast, Field::Shadows];
    for f in stepped {
        g.invoke_step(f.index(), 3);
    }
    rig.settle();
    let target = |f: Field| {
        let settings = ctl.state.borrow().store().settings().clone();
        position_on(f, f.knob().get(&settings), &track_on(f, &settings))
    };
    let away: Vec<(Field, f32)> = stepped.iter().map(|f| (*f, oni_app::app::fraction(&g, *f))).collect();
    assert!(!ctl.arriving(), "the steps' springs have rested");

    let before = ctl.state.borrow().arrivals();
    let asked = Instant::now();
    g.invoke_reset(lighting);
    let moved: Vec<(Field, f32)> = away.iter().map(|(f, _)| (*f, target(*f))).filter(|(f, to)| away.iter().any(|(k, from)| k == f && from != to)).collect();
    assert_eq!(moved.len(), stepped.len(), "the reset moves every stepped knob");
    assert!(ctl.arriving(), "the knobs spring home");
    // Twin: knobs placed at once would already stand on their targets.
    assert!(moved.iter().any(|(f, to)| oni_app::app::fraction(&g, *f) != *to), "the reset placed the knobs instead of springing them");
    let started = ctl.state.borrow().arrivals() - before;
    assert_eq!(started, 1, "one timer springs every knob of the block");
    // Twin: a timer per knob would have started once for each knob moved.
    assert_ne!(started, moved.len() as u32);

    let settled = Duration::from_secs_f64(Spring::SETTLE.settled_after());
    while ctl.arriving() && asked.elapsed() < settled + Duration::from_millis(250) {
        rig.pump();
        std::thread::sleep(Duration::from_millis(4));
    }
    assert!(!ctl.arriving(), "the timer stops once every knob rests, {:?} after the reset", asked.elapsed());
    for (f, to) in &moved {
        assert_eq!(oni_app::app::fraction(&g, *f), *to, "{f:?} lands on its target");
    }
    assert_eq!(ctl.state.borrow().arrivals() - before, 1, "no second timer started while they flew");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
