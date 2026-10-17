//! `ONI_BENCH=1`: once the open frame is on the canvas, sixty notches dragged on Exposure at a
//! trackpad's pace through the slider's own callbacks, then the notch centiles, without a hand.
//! `ONI_BENCH=resize` also resizes the window on every notch, so the surface is reconfigured mid-render.

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

/// The two window sizes the resize bench alternates between, in logical points.
const SIZES: [(f32, f32); 2] = [(1100.0, 712.0), (900.0, 600.0)];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Drag,
    Resize,
}

pub fn requested() -> Option<Mode> {
    match std::env::var("ONI_BENCH").ok()?.as_str() {
        "1" => Some(Mode::Drag),
        "resize" => Some(Mode::Resize),
        _ => None,
    }
}

#[derive(Clone, Copy)]
enum Phase {
    Loading,
    Dragging { notch: usize, from: usize },
    Landing { revision: u64, from: usize },
}

/// Drives the drag on the window's timers; `done` receives the report once the last notch is shown
/// and the drag is undone, so the frame's sidecar reads as it did.
pub fn start(ctl: &Rc<Ctl>, mode: Mode, done: impl FnOnce(String) + 'static) {
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
                if mode == Mode::Resize {
                    let (width, height) = SIZES[notch % SIZES.len()];
                    w.window().set_size(slint::LogicalSize::new(width, height));
                }
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
                let title = match mode {
                    Mode::Drag => format!("bench, {NOTCHES} notches dragged on Exposure"),
                    Mode::Resize => format!("bench, {NOTCHES} notches dragged on Exposure, the window resized on each"),
                };
                let report = ctl.state.borrow().notches().summary(from, &title);
                ctl.act("undo");
                ctl.flush();
                finish(report);
            }
        }
    });
}
