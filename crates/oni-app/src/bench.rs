//! `ONI_BENCH=1`: once the open frame is on the canvas, sixty notches dragged on Exposure at a
//! trackpad's pace through the slider's own callbacks, then the notch centiles, without a hand.

use crate::Store as StoreGlobal;
use crate::app::Ctl;
use crate::fields::Field;
use slint::{ComponentHandle, Timer, TimerMode};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

pub const NOTCHES: usize = 60;
/// A trackpad reports at 120 Hz.
const PACE: Duration = Duration::from_micros(8_333);
/// Two points a notch on a 300-point track: the drag crosses two fifths of it, inside the range.
const STEP: f32 = 2.0;
const SPAN: f32 = 300.0;
/// The first render waited for, decode included, before the bench gives up.
const LOADING: Duration = Duration::from_secs(600);

pub fn requested() -> bool {
    std::env::var_os("ONI_BENCH").is_some_and(|v| v == "1")
}

#[derive(Clone, Copy)]
enum Phase {
    Loading,
    Dragging { notch: usize, from: usize },
    Landing { revision: u64, from: usize },
}

/// Drives the drag on the window's timers; `done` receives the report once the last notch is shown
/// and the drag is undone, so the frame's sidecar reads as it did.
pub fn start(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let phase = Cell::new(Phase::Loading);
    let done = RefCell::new(Some(done));
    let started = Instant::now();
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, PACE, move || {
        let timer = &held;
        let Some(ctl) = me.upgrade() else { return timer.stop() };
        let Some(w) = ctl.window() else { return };
        let g = w.global::<StoreGlobal>();
        let field = Field::Exposure.index();
        let finish = |text: String| {
            timer.stop();
            if let Some(done) = done.borrow_mut().take() {
                done(text);
            }
        };
        match phase.get() {
            Phase::Loading => {
                if ctl.state.borrow().renders() >= 1 {
                    let from = ctl.state.borrow().notches().count();
                    g.invoke_press(field, false);
                    phase.set(Phase::Dragging { notch: 0, from });
                } else if started.elapsed() > LOADING {
                    finish(format!("bench: no frame rendered in {} s", LOADING.as_secs()));
                }
            }
            Phase::Dragging { notch, from } => {
                let notch = notch + 1;
                g.invoke_drag(field, notch as f32 * STEP, SPAN, false);
                if notch < NOTCHES {
                    phase.set(Phase::Dragging { notch, from });
                } else {
                    g.invoke_release(field);
                    phase.set(Phase::Landing { revision: ctl.state.borrow().store().revision(), from });
                }
            }
            Phase::Landing { revision, from } => {
                if ctl.state.borrow().rendered() < revision {
                    return;
                }
                let report = ctl.state.borrow().notches().summary(from, &format!("bench, {NOTCHES} notches dragged on Exposure"));
                ctl.act("undo");
                ctl.flush();
                finish(report);
            }
        }
    });
}
