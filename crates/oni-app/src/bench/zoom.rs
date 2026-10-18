//! `ONI_BENCH=zoom`: a double click to 100 %, full resolution waited for, a pan, and back to fit.

use super::*;

/// Sixty pan steps of 6 points, a trackpad's glide, once at 100 %.
pub(super) const PANS: usize = 60;
pub(super) const PAN_STEP: f32 = 6.0;
/// What full resolution may take to arrive, decode included.
pub(super) const FULL: Duration = Duration::from_secs(20);

#[derive(Clone, Copy)]
pub(super) enum Zoomed {
    Loading,
    In { asked: Instant },
    Panning { step: usize, from: usize },
    Out { from: usize, full_after: Duration },
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
