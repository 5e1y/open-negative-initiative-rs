//! `ONI_BENCH=zoom`: a double click to 100 %, full resolution waited for, a pan, then the zoom
//! ladder walked up and down to fit, each rung settled.

use super::*;

/// Sixty pan steps of 6 points, a trackpad's glide, once at 100 %.
pub(super) const PANS: usize = 60;
pub(super) const PAN_STEP: f32 = 6.0;
/// What full resolution may take to arrive, decode included.
pub(super) const FULL: Duration = Duration::from_secs(20);
/// Rungs walked up from 100 %, then down to fit, each held past the view's settle so it renders sharp.
pub(super) const RUNGS_UP: usize = 2;
pub(super) const RUNG_HOLD: Duration = Duration::from_millis(400);
/// A trackpad pinch from fit to three times and back, one event a 120 Hz frame.
pub(super) const PINCH_EVENTS: usize = 60;
pub(super) const PINCH_TO: f32 = 3.0;
/// A laptop's whole screen, so the regions rendered are the size a full-screen window asks.
pub(super) const WINDOW: (f32, f32) = (1728.0, 1080.0);

#[derive(Clone, Copy)]
pub(super) enum Zoomed {
    Loading,
    In { asked: Instant, step: usize, early: usize },
    Panning { step: usize, from: usize },
    Ladder { rung: usize, at: Instant, from: usize },
    Pinching { event: usize, from: usize, pinch_from: usize },
    Out { from: usize, full_after: Duration },
}

thread_local! {
    static PINCHED: Cell<usize> = const { Cell::new(0) };
    static EARLY: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
    /// The interface thread's longest wait between two ticks while full resolution is built.
    static STALL: Cell<(Option<Instant>, Duration)> = const { Cell::new((None, Duration::ZERO)) };
}

/// The canvas's own callbacks driven as a hand would: the double click, the drag, the double click back.
pub(super) fn zoom(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
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
                if w.window().size().width < (WINDOW.0 * w.window().scale_factor()) as u32 {
                    w.window().set_size(slint::LogicalSize::new(WINDOW.0, WINDOW.1));
                }
                if s.renders() >= 1 && s.loaded() >= s.photos() && s.placed().is_some() {
                    let early = s.views().count();
                    drop(s);
                    c.invoke_double_clicked();
                    c.invoke_grabbed(200.0, 200.0);
                    phase.set(Zoomed::In { asked: Instant::now(), step: 0, early });
                }
            }
            Zoomed::In { asked, step, early } => {
                let full = s.placed().is_some_and(|p| p.at_full_resolution);
                if full {
                    full_after.set(asked.elapsed());
                    let from = s.views().count();
                    EARLY.with(|e| e.set((early, from)));
                    drop(s);
                    c.invoke_grabbed(200.0, 200.0);
                    phase.set(Zoomed::Panning { step: 0, from });
                } else if asked.elapsed() > FULL {
                    drop(s);
                    finish(format!("bench, zoom: no full-resolution render in {} s", FULL.as_secs()));
                } else {
                    STALL.with(|t| {
                        let (last, worst) = t.get();
                        t.set((Some(Instant::now()), last.map_or(worst, |l| worst.max(l.elapsed()))));
                    });
                    // The hand moves on at once in bursts, back and forth, each pause long enough to settle
                    // the view, so full resolution is built while it moves.
                    drop(s);
                    let beat = step % 40;
                    if beat < 12 {
                        let x = 200.0 + PAN_STEP * if (step / 40) % 2 == 0 { beat as f32 } else { (12 - beat) as f32 };
                        c.invoke_dragged(x, 200.0);
                    }
                    phase.set(Zoomed::In { asked, step: step + 1, early });
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
                    phase.set(Zoomed::Ladder { rung: 0, at: Instant::now(), from });
                }
            }
            Zoomed::Ladder { rung, at, from } => {
                let fitted = s.viewport().is_fitted();
                drop(s);
                if at.elapsed() < RUNG_HOLD {
                    return;
                }
                if fitted {
                    c.invoke_pinch_started();
                    phase.set(Zoomed::Pinching { event: 0, from, pinch_from: ctl.state.borrow().views().count() });
                    return;
                }
                ctl.act(if rung < RUNGS_UP { "larger" } else { "smaller" });
                phase.set(Zoomed::Ladder { rung: rung + 1, at: Instant::now(), from });
            }
            Zoomed::Pinching { event, from, pinch_from } => {
                drop(s);
                let event = event + 1;
                let half = PINCH_EVENTS / 2;
                let k = if event <= half { event } else { PINCH_EVENTS - event } as f32 / half as f32;
                let (x, y) = (c.get_width() / 2.0, c.get_height() / 2.0);
                c.invoke_pinched(PINCH_TO.powf(k), x, y);
                if event < PINCH_EVENTS {
                    phase.set(Zoomed::Pinching { event, from, pinch_from });
                } else {
                    PINCHED.with(|p| p.set(pinch_from));
                    phase.set(Zoomed::Out { from, full_after: full_after.get() });
                }
            }
            Zoomed::Out { from, full_after } => {
                // A pinch back to the fitted scale is not the fit itself: the ladder's last rung is.
                if !s.viewport().is_fitted() {
                    drop(s);
                    ctl.act("smaller");
                    return;
                }
                if !s.placed().is_some_and(|p| p.whole) {
                    return;
                }
                let pinch_from = PINCHED.with(Cell::get);
                let (early, built) = EARLY.with(Cell::get);
                let report = format!(
                    "bench, zoom: full resolution {:.0} ms after the double click; {}\n{}\n{}\n{}",
                    full_after.as_secs_f64() * 1000.0,
                    format!(
                        "{}; the interface thread's longest tick {:.1} ms",
                        s.views().summary_between(early, built, "pans while full resolution is built"),
                        STALL.with(Cell::get).1.as_secs_f64() * 1000.0
                    ),
                    s.views().summary(from, &format!("{PANS} pan steps at 100 %, then the ladder")),
                    s.views().cadence(from),
                    s.views().summary(pinch_from, &format!("a pinch of {PINCH_EVENTS} events from fit")),
                );
                drop(s);
                finish(report);
            }
        }
    });
}
