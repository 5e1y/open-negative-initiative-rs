//! `ONI_BENCH=1`: once the open frame is on the canvas, sixty notches dragged on Exposure at a
//! trackpad's pace through the slider's own callbacks, then the notch centiles, without a hand.
//! `ONI_BENCH=resize` also resizes the window on every notch, so the surface is reconfigured mid-render;
//! `ONI_BENCH=settled` waits for every thumbnail first, so the drag has the GPU to itself;
//! `ONI_BENCH=long` then drags slowly for ten seconds and reports the image's cadence;
//! `ONI_BENCH=zoom` double-clicks to 100 %, waits for full resolution, pans, and returns to fit;
//! `ONI_BENCH=compare` turns Compare on, drags Exposure and the split, and counts the befores.

use crate::Store as StoreGlobal;
use crate::app::Ctl;
use crate::fields::Field;
use slint::{ComponentHandle, Timer, TimerMode};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

pub const NOTCHES: usize = 60;
/// Ten seconds at the trackpad's pace, a tenth of a point a notch: one way, so no turn hands the
/// same value twice, and inside the range.
const LONG: usize = 1200;
const LONG_STEP: f32 = 0.1;
/// A trackpad reports at 120 Hz.
const PACE: Duration = Duration::from_micros(8_333);
/// Two points a notch on a 300-point track: the drag crosses two fifths of it, inside the range.
const STEP: f32 = 2.0;
const SPAN: f32 = 300.0;
/// The first render waited for, decode included, before the bench gives up.
const LOADING: Duration = Duration::from_secs(600);

/// On the main screen, whose refresh the gaps are judged against.
const ORIGIN: (f32, f32) = (80.0, 80.0);
/// The two window sizes the resize bench alternates between, in logical points.
const SIZES: [(f32, f32); 2] = [(1100.0, 712.0), (900.0, 600.0)];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Drag,
    Resize,
    Settled,
    Long,
    Zoom,
    Compare,
}

impl Mode {
    fn notches(self) -> usize {
        if self == Mode::Long { LONG } else { NOTCHES }
    }

    /// The drag's travel at `notch`, in points.
    fn travel(self, notch: usize) -> f32 {
        notch as f32 * if self == Mode::Long { LONG_STEP } else { STEP }
    }
}

pub fn requested() -> Option<Mode> {
    match std::env::var("ONI_BENCH").ok()?.as_str() {
        "1" => Some(Mode::Drag),
        "resize" => Some(Mode::Resize),
        "settled" => Some(Mode::Settled),
        "long" => Some(Mode::Long),
        "zoom" => Some(Mode::Zoom),
        "compare" => Some(Mode::Compare),
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
    if mode == Mode::Zoom {
        return zoom(ctl, done);
    }
    let timer = Rc::new(Timer::default());
    let phase = Cell::new(Phase::Loading);
    let done = RefCell::new(Some(done));
    let started = Instant::now();
    if let Some(w) = ctl.window() {
        w.window().set_position(slint::LogicalPosition::new(ORIGIN.0, ORIGIN.1));
    }
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
                let settled = !matches!(mode, Mode::Settled | Mode::Long) || ctl.state.borrow().loaded() >= ctl.state.borrow().photos();
                if ctl.state.borrow().renders() >= 1 && settled {
                    if mode == Mode::Compare && ctl.state.borrow().split().is_none() {
                        ctl.act("compare");
                        return;
                    }
                    if mode == Mode::Compare && ctl.state.borrow().befores() == 0 {
                        return;
                    }
                    let from = ctl.state.borrow().notches().count();
                    g.invoke_press(field, false);
                    phase.set(Phase::Dragging { notch: 0, from });
                } else if started.elapsed() > LOADING {
                    finish(format!("bench: no frame rendered in {} s", LOADING.as_secs()));
                }
            }
            Phase::Dragging { notch, from } => {
                let notch = notch + 1;
                g.invoke_drag(field, mode.travel(notch), SPAN, false);
                if mode == Mode::Compare {
                    let width = w.global::<crate::Canvas>().get_width();
                    w.global::<crate::Canvas>().invoke_split_dragged(width * (0.2 + 0.6 * notch as f32 / NOTCHES as f32));
                }
                if mode == Mode::Resize {
                    let (width, height) = SIZES[notch % SIZES.len()];
                    w.window().set_size(slint::LogicalSize::new(width, height));
                }
                if notch < mode.notches() {
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
                    Mode::Settled => format!("bench, {NOTCHES} notches dragged on Exposure, every thumbnail read"),
                    Mode::Long | Mode::Zoom => format!("bench, {LONG} notches dragged on Exposure for ten seconds"),
                    Mode::Compare => format!("bench, {NOTCHES} notches dragged on Exposure beside Compare's before"),
                };
                let notches = ctl.state.borrow().notches().summary(from, &title);
                let mut report = format!("{notches}\n{}", ctl.state.borrow().notches().cadence(from));
                if mode == Mode::Compare {
                    let s = ctl.state.borrow();
                    report.push_str(&format!("\ncompare: {} before rendered over the drag, split at {:.2}", s.befores(), s.split().unwrap_or(-1.0)));
                }
                ctl.act("undo");
                ctl.flush();
                finish(report);
            }
        }
    });
}

/// Sixty pan steps of 6 points, a trackpad's glide, once at 100 %.
const PANS: usize = 60;
const PAN_STEP: f32 = 6.0;
/// What full resolution may take to arrive, decode included.
const FULL: Duration = Duration::from_secs(20);

#[derive(Clone, Copy)]
enum Zoomed {
    Loading,
    In { asked: Instant },
    Panning { step: usize, from: usize },
    Out { from: usize, full_after: Duration },
}

/// The canvas's own callbacks driven as a hand would: the double click, the drag, the double click back.
fn zoom(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let phase = Cell::new(Zoomed::Loading);
    let done = RefCell::new(Some(done));
    let full_after = Cell::new(Duration::ZERO);
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, PACE, move || {
        let timer = &held;
        let Some(ctl) = me.upgrade() else { return timer.stop() };
        let Some(w) = ctl.window() else { return };
        let c = w.global::<crate::Canvas>();
        let finish = |text: String| {
            timer.stop();
            if let Some(done) = done.borrow_mut().take() {
                done(text);
            }
        };
        let s = ctl.state.borrow();
        match phase.get() {
            Zoomed::Loading => {
                if s.renders() >= 1 && s.loaded() >= s.photos() && s.placed().is_some() {
                    drop(s);
                    c.invoke_double_clicked();
                    phase.set(Zoomed::In { asked: Instant::now() });
                }
            }
            Zoomed::In { asked } => {
                let full = s.placed().is_some_and(|p| p.at_full_resolution);
                if full {
                    full_after.set(asked.elapsed());
                    let from = s.views().count();
                    drop(s);
                    c.invoke_grabbed(200.0, 200.0);
                    phase.set(Zoomed::Panning { step: 0, from });
                } else if asked.elapsed() > FULL {
                    drop(s);
                    finish(format!("bench, zoom: no full-resolution render in {} s", FULL.as_secs()));
                }
            }
            Zoomed::Panning { step, from } => {
                drop(s);
                let step = step + 1;
                let x = 200.0 - step as f32 * PAN_STEP;
                c.invoke_dragged(x, 200.0 - step as f32 * PAN_STEP / 2.0);
                if step < PANS {
                    phase.set(Zoomed::Panning { step, from });
                } else {
                    c.invoke_double_clicked();
                    phase.set(Zoomed::Out { from, full_after: full_after.get() });
                }
            }
            Zoomed::Out { from, full_after } => {
                let fitted = s.viewport().is_fitted() && s.placed().is_some_and(|p| p.whole);
                if !fitted {
                    return;
                }
                let report = format!(
                    "bench, zoom: full resolution {:.0} ms after the double click; {}\n{}",
                    full_after.as_secs_f64() * 1000.0,
                    s.views().summary(from, &format!("{PANS} pan steps at 100 %")),
                    s.views().cadence(from)
                );
                drop(s);
                finish(report);
            }
        }
    });
}
