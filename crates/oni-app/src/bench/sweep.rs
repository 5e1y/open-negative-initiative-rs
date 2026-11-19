//! `ONI_BENCH=sweep`: the full resolution built and the view held at 40 %, `ONI_SWEEP` (pinch, wheel or
//! keys) zooms to 120 % and back three times; each move's time to the GPU's end of the first frame that
//! shows it moved, and of the first that shows its render, which the photon follows by one refresh at most.

use super::chain::{self, Recorded, centiles, photon};
use super::*;
use std::fmt::Write as _;

const LOW: f64 = 0.4;
const HIGH: f64 = 1.2;
const CYCLES: usize = 3;
/// The view held at `LOW` before the sweep, so its sharp render and the full frame's halvings are done.
const HOLD: Duration = Duration::from_millis(1500);
/// After the last move: the sharp render lands, whatever the screen.
const REST: Duration = Duration::from_secs(2);
/// A gap between two frames' ends past three 120 Hz refreshes, which the eye sees as a hitch.
const HITCH: Duration = Duration::from_millis(25);
/// The full resolution waited for before the bench gives up.
const FULL: Duration = Duration::from_secs(40);

#[derive(Clone, Copy)]
enum Sw {
    Loading,
    Holding { at: Instant },
    Moving { tick: usize },
    Resting { at: Instant },
}

/// A sweep's way of moving: the timer's ticks between two moves (120 Hz ticks) and the moves a leg.
fn pace(how: &str) -> (usize, usize) {
    match how {
        "wheel" => (15, 12),
        "keys" => (30, 8),
        _ => (1, 60),
    }
}

/// The canvas in drawable pixels.
fn canvas(ctl: &Ctl) -> Option<(f64, f64)> {
    let w = ctl.window()?;
    let (c, k) = (w.global::<crate::Canvas>(), f64::from(w.window().scale_factor()));
    Some((f64::from(c.get_width()) * k, f64::from(c.get_height()) * k))
}

fn scale_now(ctl: &Ctl) -> Option<f64> {
    let size = canvas(ctl)?;
    let s = ctl.state.borrow();
    Some(s.viewport().effective_scale(s.full_size()?, size))
}

pub(super) fn sweep(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let how = std::env::var("ONI_SWEEP").unwrap_or_else(|_| "pinch".into());
    let timer = Rc::new(Timer::default());
    let phase = Cell::new(Sw::Loading);
    let done = RefCell::new(Some(done));
    let started = Instant::now();
    let pinch = Cell::new(1.0f32);
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
        let (cx, cy) = (c.get_width() / 2.0, c.get_height() / 2.0);
        match phase.get() {
            Sw::Loading => {
                let ready = {
                    let s = ctl.state.borrow();
                    s.placed().is_some_and(|p| p.whole) && s.full_built()
                };
                if ready && healed(&ctl) {
                    let Some(now) = scale_now(&ctl) else { return };
                    c.invoke_pinch_started();
                    c.invoke_pinched((LOW / now) as f32, cx, cy);
                    phase.set(Sw::Holding { at: Instant::now() });
                } else if started.elapsed() > FULL {
                    finish(format!("bench, sweep: no full resolution in {} s", FULL.as_secs()));
                }
            }
            Sw::Holding { at } => {
                if at.elapsed() >= HOLD {
                    chain::record();
                    c.invoke_pinch_started();
                    pinch.set(1.0);
                    phase.set(Sw::Moving { tick: 0 });
                }
            }
            Sw::Moving { tick } => {
                let Some(now) = scale_now(&ctl) else { return };
                let (period, leg) = pace(&how);
                let tick = tick + 1;
                phase.set(Sw::Moving { tick });
                if tick % period != 0 {
                    return;
                }
                let event = tick / period - 1;
                if event / leg >= 2 * CYCLES {
                    return phase.set(Sw::Resting { at: Instant::now() });
                }
                let up = (event / leg) % 2 == 0;
                match how.as_str() {
                    "wheel" => {
                        crate::viewport::note_wheel(crate::viewport::Wheel { dx: 0.0, dy: if up { 1.0 } else { -1.0 }, precise: false });
                        c.invoke_scrolled(0.0, if up { 60.0 } else { -60.0 }, cx, cy, false);
                    }
                    "keys" => ctl.act(if up { crate::Act::Larger } else { crate::Act::Smaller }),
                    _ => {
                        let k = (event % leg + 1) as f64 / leg as f64;
                        let target = if up { LOW * (HIGH / LOW).powf(k) } else { HIGH * (LOW / HIGH).powf(k) };
                        pinch.set(pinch.get() * (target / now) as f32);
                        c.invoke_pinched(pinch.get(), cx, cy);
                    }
                }
            }
            Sw::Resting { at } => {
                if at.elapsed() >= REST {
                    let size = canvas(&ctl).unwrap_or_default();
                    finish(report(&how, size, &chain::taken()));
                }
            }
        }
    });
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// The sweep's centiles: each move to its photon, each render's stages, and the frames' consistency.
fn report(how: &str, size: (f64, f64), r: &Recorded) -> String {
    let (moves, views, frames) = (&r.moves, &r.views, &r.frames);
    let since = |from: Instant, to: Option<Instant>| to.map(|t| t.saturating_duration_since(from));
    let moved: Vec<Duration> = moves.iter().filter_map(|m| since(m.at, photon(m.at + m.took, frames))).collect();
    let rendered: Vec<Duration> = moves.iter().filter_map(|m| since(m.at, photon(views.iter().find(|v| v.stages.asked >= m.at)?.stages.encoded, frames))).collect();
    let mut out = format!("bench, sweep ({how}) on a {:.0}×{:.0} canvas: {} moves, {} view renders landed, {} frames", size.0, size.1, moves.len(), views.len(), frames.len());
    let _ = write!(out, "\n  move handled {}", centiles(moves.iter().map(|m| m.took).collect()));
    let _ = write!(out, "\n  move to the picture moved, GPU end {}", centiles(moved));
    let _ = write!(out, "\n  move to its render shown, GPU end {}", centiles(rendered));
    for coarse in [true, false] {
        let kept: Vec<&chain::Viewed> = views.iter().filter(|v| v.coarse == coarse).collect();
        let stage = |f: &dyn Fn(&chain::Viewed) -> Option<Duration>| centiles(kept.iter().filter_map(|v| f(v)).collect());
        let name = if coarse { "lighter" } else { "sharp" };
        let _ = write!(out, "\n  {name} renders: {}", kept.len());
        let _ = write!(out, "\n    asked to started {}", stage(&|v| Some(v.stages.started - v.stages.asked)));
        let _ = write!(out, "\n    CPU side {}", stage(&|v| Some(v.stages.drawn - v.stages.started)));
        let _ = write!(out, "\n    encoded to GPU end {}", stage(&|v| since(v.stages.encoded, photon(v.stages.encoded, frames))));
        let _ = write!(out, "\n    asked to GPU end {}", stage(&|v| since(v.stages.asked, photon(v.stages.encoded, frames))));
        let left = kept.iter().filter(|v| (v.landed_scale / v.asked_scale - 1.0).abs() > 1e-6).count();
        let _ = write!(out, "\n    landed at a scale the view had left: {left}");
    }
    let ends: Vec<Instant> = frames.iter().filter_map(|f| f.gpu.get().copied()).collect();
    let gaps: Vec<Duration> = ends.windows(2).map(|w| w[1].saturating_duration_since(w[0])).collect();
    let hitches = gaps.iter().filter(|g| **g > HITCH).count();
    let _ = write!(out, "\n  frames' GPU-end gaps {}; {hitches} of {} over {:.0} ms", centiles(gaps.clone()), gaps.len(), ms(HITCH));
    let presents: Vec<Duration> = frames.windows(2).map(|w| w[1].presented - w[0].presented).collect();
    let _ = write!(out, "\n  frames' present gaps {}", centiles(presents));
    let _ = write!(out, "\n  frame presented to its GPU end {}", centiles(frames.iter().filter_map(|f| since(f.presented, f.gpu.get().copied())).collect()));
    let _ = write!(out, "\n  drawable acquire {}", centiles(frames.iter().map(|f| f.acquired).collect()));
    let _ = write!(out, "\n  items and submit {}", centiles(frames.iter().map(|f| f.items + f.submitted).collect()));
    if let (Some(a), Some(b)) = (moves.first(), moves.last()) {
        let (period, _) = pace(how);
        let _ = write!(out, "\n  the sweep took {:.0} ms for {} moves asked every {:.1} ms", ms(b.at - a.at), moves.len(), ms(PACE) * period as f64);
    }
    if let (Some(last), Some(sharp)) = (moves.last(), views.iter().rev().find(|v| !v.coarse))
        && let Some(p) = since(last.at, photon(sharp.stages.encoded, frames))
    {
        let _ = write!(out, "\n  last move to the sharp render's GPU end {:.1} ms, at {:.0} %", ms(p), sharp.landed_scale * 100.0);
    }
    out
}
