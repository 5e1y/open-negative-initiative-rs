//! `ONI_BENCH=traverse`: a double click to 100 % and at once a long pan across the frame into parts never shown,
//! the hand never resting and turning back at each edge, so each step pays whatever the view's first read costs.

use super::*;

/// Steps of `STEP` points, one a 120 Hz frame, enough to cross a 47 Mpx frame shown at 100 %.
const STEPS: usize = 480;
const STEP: f32 = 12.0;
/// The ticks the last step's render is given before the report.
const SETTLE_TICKS: usize = 60;

pub(super) fn traverse(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let (from, step) = (Cell::new(None::<usize>), Cell::new(0usize));
    // Where the hand stands and the way it goes on each axis, turned back where the view no longer moves.
    let hand = Cell::new(((1600.0f32, 1000.0f32), (-1.0f32, -0.6f32)));
    let drawn: RefCell<Rc<RefCell<Vec<(Instant, Duration)>>>> = RefCell::default();
    let began = Cell::new(Instant::now());
    let done = RefCell::new(Some(done));
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, PACE, move || {
        let Some(ctl) = me.upgrade() else { return held.stop() };
        let Some(w) = ctl.window() else { return };
        let c = w.global::<crate::Canvas>();
        let Some(start) = from.get() else {
            if w.window().size().width < (WINDOW.0 * w.window().scale_factor()) as u32 {
                w.window().set_size(slint::LogicalSize::new(WINDOW.0, WINDOW.1));
            }
            let s = ctl.state.borrow();
            let (ready, count) = (s.renders() >= 1 && s.placed().is_some(), s.views().count());
            drop(s);
            if ready && healed(&ctl) {
                *drawn.borrow_mut() = frame_clock(&ctl);
                began.set(Instant::now());
                c.invoke_double_clicked();
                c.invoke_grabbed(1600.0, 1000.0);
                from.set(Some(count));
            }
            return;
        };
        let n = step.get() + 1;
        step.set(n);
        if n <= STEPS {
            let ((x, y), (dx, dy)) = hand.get();
            let before = ctl.state.borrow().viewport().offset;
            let (x, y) = (x + dx * STEP, y + dy * STEP);
            c.invoke_dragged(x, y);
            let after = ctl.state.borrow().viewport().offset;
            let turned = |was: f64, now: f64, d: f32| if (now - was).abs() < 1e-6 { -d } else { d };
            hand.set(((x, y), (turned(before.0, after.0, dx), turned(before.1, after.1, dy))));
            return;
        }
        if n < STEPS + SETTLE_TICKS {
            return;
        }
        held.stop();
        let s = ctl.state.borrow();
        let report = format!(
            "bench, traverse: {}\n{}\n  {}",
            s.views().summary(start, &format!("{STEPS} pan steps of {STEP} points at 100 % from the double click")),
            s.views().cadence(start),
            frames_since(&drawn.borrow().borrow(), began.get())
        );
        drop(s);
        if let Some(done) = done.borrow_mut().take() {
            done(report);
        }
    });
}
