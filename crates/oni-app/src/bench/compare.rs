//! `ONI_BENCH=compare`: the full resolution built, Compare on and the view at 100 % (`ONI_AT`, a scale);
//! a drag pan and a trackpad pan out and back, a pinch, an Exposure drag and a split drag at 120 Hz. Each
//! move is timed to the GPU's end of the first frame showing the after moved, and the before on it.

use super::chain::{self, Recorded, Side, centiles, photon};
use super::*;
use std::fmt::Write as _;

/// Events a leg: a pan goes out for two legs and comes back for two, a pinch and a drag take one.
const EVENTS: usize = 60;
/// The view held at its scale before the moves, so its sharp render and the before are done.
const HOLD: Duration = Duration::from_millis(1500);
/// Between two phases, so each starts on a canvas at rest.
const REST: Duration = Duration::from_millis(1000);
/// A gap between two frames' ends past three 120 Hz refreshes, which the eye sees as a hitch.
const HITCH: Duration = Duration::from_millis(25);
const PHASES: [&str; 5] = ["drag pan", "trackpad pan", "pinch", "Exposure drag", "split drag"];

#[derive(Clone, Copy)]
enum Cp {
    Loading,
    Comparing,
    Holding { at: Instant },
    Moving { phase: usize, event: usize },
    Resting { at: Instant, next: usize },
}

pub(super) fn compare(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let state = Cell::new(Cp::Loading);
    // Each phase's first event, and its last: the frames' gaps are judged while the hand moves.
    let starts: RefCell<Vec<Instant>> = RefCell::default();
    let stops: RefCell<Vec<Instant>> = RefCell::default();
    let done = RefCell::new(Some(done));
    let started = Instant::now();
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, PACE, move || {
        let timer = &held;
        let Some(ctl) = me.upgrade() else { return timer.stop() };
        let Some(w) = ctl.window() else { return };
        let (c, g) = (w.global::<crate::Canvas>(), w.global::<Store>());
        let (cx, cy) = (c.get_width() / 2.0, c.get_height() / 2.0);
        let finish = |text: String| {
            timer.stop();
            if let Some(done) = done.borrow_mut().take() {
                done(text);
            }
        };
        let field = Field::Exposure.index();
        match state.get() {
            Cp::Loading => {
                let ready = ctl.state.borrow().placed().is_some_and(|p| p.whole) && ctl.state.borrow().full_built();
                if ready && healed(&ctl) {
                    ctl.act(crate::Act::Compare);
                    state.set(Cp::Comparing);
                } else if started.elapsed() > LOADING {
                    finish(format!("bench, compare: no full resolution in {} s", LOADING.as_secs()));
                }
            }
            Cp::Comparing => {
                let Some(p) = ctl.state.borrow().placed().filter(|_| ctl.state.borrow().befores() > 0) else { return };
                let at: f32 = std::env::var("ONI_AT").ok().and_then(|a| a.parse().ok()).unwrap_or(1.0);
                c.invoke_pinch_started();
                c.invoke_pinched(at / (p.width / p.full.0) as f32, cx, cy);
                state.set(Cp::Holding { at: Instant::now() });
            }
            Cp::Holding { at } => {
                if at.elapsed() >= HOLD {
                    chain::record();
                    state.set(Cp::Resting { at: Instant::now() - REST, next: 0 });
                }
            }
            Cp::Resting { at, next } => {
                if at.elapsed() < REST {
                    return;
                }
                starts.borrow_mut().push(Instant::now());
                match next {
                    0 => c.invoke_grabbed(cx, cy),
                    2 => c.invoke_pinch_started(),
                    3 => _ = g.invoke_press(field, false),
                    5 => {
                        let r = chain::taken();
                        let befores = ctl.state.borrow().befores();
                        ctl.act(crate::Act::Undo);
                        ctl.flush();
                        return finish(report(&r, &starts.borrow(), &stops.borrow(), befores));
                    }
                    _ => {}
                }
                state.set(Cp::Moving { phase: next, event: 0 });
            }
            Cp::Moving { phase, event } => {
                let event = event + 1;
                // Out for two legs and back for two: the pans cross the rendered margin both ways.
                let out = if event <= 2 * EVENTS { event as f32 } else { (4 * EVENTS - event) as f32 };
                let last = match phase {
                    0 => {
                        c.invoke_dragged(cx - out * 4.0, cy - out * 2.0);
                        4 * EVENTS
                    }
                    1 => {
                        let d = if event <= 2 * EVENTS { -4.0 } else { 4.0 };
                        c.invoke_scrolled(d, d / 2.0, cx * 1.5, cy, false);
                        4 * EVENTS
                    }
                    2 => {
                        c.invoke_pinched(1.33f32.powf((std::f32::consts::TAU * event as f32 / EVENTS as f32).sin()), cx, cy);
                        EVENTS
                    }
                    3 => {
                        g.invoke_drag(field, event as f32 * STEP, SPAN, false);
                        EVENTS
                    }
                    _ => {
                        c.invoke_split_dragged(c.get_width() * (0.2 + 0.6 * event as f32 / EVENTS as f32));
                        EVENTS
                    }
                };
                if event < last {
                    return state.set(Cp::Moving { phase, event });
                }
                stops.borrow_mut().push(Instant::now());
                if phase == 3 {
                    g.invoke_release(field);
                }
                state.set(Cp::Resting { at: Instant::now(), next: phase + 1 });
            }
        }
    });
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Each phase's moves to the photon of each side, the renders and textures they cost, and its frames.
fn report(r: &Recorded, starts: &[Instant], stops: &[Instant], befores: usize) -> String {
    let since = |from: Instant, to: Option<Instant>| to.map(|t| t.saturating_duration_since(from));
    let mut out = format!("bench, compare: {} moves, {} view renders, {} notches, {} frames; {befores} befores made in all", r.moves.len(), r.views.len(), r.notches.len(), r.frames.len());
    for (k, name) in PHASES.iter().enumerate() {
        let (from, to) = (starts[k], starts.get(k + 1).copied().unwrap_or_else(Instant::now));
        let within = |t: Instant| t >= from && t < to;
        let canvas: Vec<&chain::Canvas> = r.canvas.iter().filter(|c| c.at.is_some_and(within)).collect();
        let frames: Vec<_> = r.frames.iter().filter(|f| within(f.began)).collect();
        // Each frame shows the canvas as the last state before it began left it.
        let state = |t: Instant| r.canvas.iter().rev().find(|c| c.at.is_some_and(|a| a <= t));
        let off = frames.iter().filter(|f| state(f.began).is_some_and(|c| c.before == Some(false))).count();
        let shown = frames.iter().filter(|f| state(f.began).is_some_and(|c| c.before.is_some())).count();
        let made = |side| canvas.iter().filter(|c| c.made == Some(side)).count();
        let _ = write!(out, "\n{name}: {} frames, the before on {shown}, off the after's region on {off}", frames.len());
        let _ = write!(out, "\n  display textures made: after {}, before {}", made(Side::After), made(Side::Before));
        let views: Vec<&chain::Viewed> = r.views.iter().filter(|v| within(v.stages.encoded)).collect();
        let _ = write!(out, "\n  view renders landed {}, lighter {}", views.len(), views.iter().filter(|v| v.coarse).count());
        let moves: Vec<&chain::Move> = r.moves.iter().filter(|m| within(m.at)).collect();
        if !moves.is_empty() {
            let after: Vec<Duration> = moves.iter().filter_map(|m| since(m.at, photon(m.at + m.took, &r.frames))).collect();
            // The before shown on the move's place: the first state at or after it holding the after's region.
            let caught = |m: &chain::Move| r.canvas.iter().find(|c| c.at.is_some_and(|a| a >= m.at) && c.before == Some(true))?.at;
            let before: Vec<Duration> = moves.iter().filter_map(|m| since(m.at, photon(caught(m)?, &r.frames))).collect();
            let _ = write!(out, "\n  {} moves; move to the after shown, GPU end {}", moves.len(), centiles(after));
            let _ = write!(out, "\n  move to the before shown, GPU end {}", centiles(before));
        }
        let notches: Vec<&chain::Stages> = r.notches.iter().filter(|n| within(n.asked)).collect();
        if !notches.is_empty() {
            let photons: Vec<Duration> = notches.iter().filter_map(|n| since(n.asked, photon(n.encoded, &r.frames))).collect();
            let _ = write!(out, "\n  {} notches; asked to the GPU end {}", notches.len(), centiles(photons));
        }
        let moving = |t: Instant| t >= from && t <= stops[k] + PACE;
        let ends: Vec<Instant> = frames.iter().filter(|f| moving(f.began)).filter_map(|f| f.gpu.get().copied()).collect();
        let gaps: Vec<Duration> = ends.windows(2).map(|w| w[1].saturating_duration_since(w[0])).collect();
        let hitches = gaps.iter().filter(|g| **g > HITCH).count();
        let _ = write!(out, "\n  frames' GPU-end gaps while moving {}; {hitches} over {:.0} ms", centiles(gaps), ms(HITCH));
    }
    out
}
