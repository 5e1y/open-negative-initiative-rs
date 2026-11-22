//! `ONI_BENCH=stocks`: the open photo's stocks' menu laid, each stock lit in turn as the pointer resting on its
//! line lights it, once cold and once warm, then every line swept at a trackpad's pace; each light timed to the
//! GPU's end of the first frame showing its render, which the photon follows by one refresh at most.

use super::chain::{self, centiles, photon};
use super::*;

/// Long enough for a light's render to land and show, so the next light finds the slot empty.
const REST: Duration = Duration::from_millis(600);

#[derive(Clone, Copy)]
enum St {
    Loading,
    Ready { at: Instant },
    Lighting { pass: usize, k: usize, at: Instant },
    Sweeping { k: usize },
    Settling { at: Instant },
}

pub(super) fn stocks(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let phase = Cell::new(St::Loading);
    let done = RefCell::new(Some(done));
    let started = Instant::now();
    // Each pass's lights, by the stock's place in the catalogue, and the sweep's.
    let lit: RefCell<[Vec<Instant>; 3]> = RefCell::default();
    let order: RefCell<Vec<i32>> = RefCell::default();
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, PACE, move || {
        let timer = &held;
        let Some(ctl) = me.upgrade() else { return timer.stop() };
        let Some(w) = ctl.window() else { return };
        let stocks = w.global::<crate::Stocks>();
        let finish = |text: String| {
            timer.stop();
            if let Some(done) = done.borrow_mut().take() {
                done(text);
            }
        };
        match phase.get() {
            St::Loading => {
                if ctl.state.borrow().renders() < 1 || !healed(&ctl) {
                    if started.elapsed() > LOADING {
                        finish("stocks: no frame rendered".into());
                    }
                    return;
                }
                let panel = w.global::<crate::PanelState>();
                panel.set_tab(crate::PanelTab::Color);
                panel.invoke_tab_picked(crate::PanelTab::Color);
                stocks.invoke_dropping();
                let lines = stocks.get_lines();
                let own = stocks.get_in_use();
                *order.borrow_mut() = (0..lines.row_count()).filter(|&i| i as i32 != own).filter_map(|i| lines.row_data(i)).filter(|l| !l.brand).map(|l| l.index).collect();
                chain::record();
                phase.set(St::Ready { at: Instant::now() });
            }
            St::Ready { at } => {
                if at.elapsed() >= REST {
                    phase.set(St::Lighting { pass: 0, k: 0, at: Instant::now() - REST });
                }
            }
            St::Lighting { pass, k, at } => {
                if at.elapsed() < REST {
                    return;
                }
                let Some(&i) = order.borrow().get(k) else {
                    stocks.invoke_lit(-1);
                    phase.set(if pass == 0 { St::Lighting { pass: 1, k: 0, at: Instant::now() } } else { St::Sweeping { k: 0 } });
                    return;
                };
                // Timed only where the light moved what the canvas shows: one that changes nothing renders nothing.
                let (shown, at) = (ctl.state.borrow().shown_settings().cloned(), Instant::now());
                stocks.invoke_lit(i);
                if ctl.state.borrow().shown_settings() != shown.as_ref() {
                    lit.borrow_mut()[pass].push(at);
                }
                phase.set(St::Lighting { pass, k: k + 1, at: Instant::now() });
            }
            St::Sweeping { k } => match order.borrow().get(k) {
                Some(&i) => {
                    lit.borrow_mut()[2].push(Instant::now());
                    stocks.invoke_lit(i);
                    phase.set(St::Sweeping { k: k + 1 });
                }
                None => phase.set(St::Settling { at: Instant::now() }),
            },
            St::Settling { at } => {
                if at.elapsed() < REST * 2 {
                    return;
                }
                let own = ctl.state.borrow().store().settings().clone();
                stocks.invoke_lit(-1);
                let restored = ctl.state.borrow().stock_lit().is_none();
                let recorded = chain::taken();
                let lights = lit.borrow().clone();
                // Each light's render: the first asked after it, closed at its photon.
                let to_photon = |at: &Instant| {
                    let view = recorded.views.iter().filter(|v| v.stages.asked >= *at).min_by_key(|v| v.stages.asked)?;
                    Some(photon(view.stages.encoded, &recorded.frames)? - *at)
                };
                let pass = |p: &[Instant]| p.iter().filter_map(to_photon).collect::<Vec<_>>();
                // The warm pass's legs: to the render's encode, to the next frame's start, that frame's own time.
                let legs = |at: &Instant| {
                    let view = recorded.views.iter().filter(|v| v.stages.asked >= *at).min_by_key(|v| v.stages.asked)?;
                    let frame = recorded.frames.iter().find(|f| f.began >= view.stages.encoded)?;
                    Some([view.stages.encoded - *at, frame.began - view.stages.encoded, *frame.gpu.get()? - frame.began])
                };
                let warm_legs: Vec<[Duration; 3]> = lights[1].iter().filter_map(legs).collect();
                let leg = |k: usize| centiles(warm_legs.iter().map(|l| l[k]).collect());
                let (cold, warm) = (pass(&lights[0]), pass(&lights[1]));
                let first = cold.first().copied();
                let sweep = &lights[2];
                let swept = sweep.first().map_or(0, |from| recorded.views.iter().filter(|v| v.stages.asked >= *from).count());
                let last = sweep.last().and_then(to_photon);
                let written = ctl.state.borrow().store().settings() == &own;
                finish(format!(
                    "stocks: {} stocks lit, {} moving the photo, hover to photon cold {} (first {:.1} ms), warm {} (to the encode {}, to the next frame {}, the frame {}); a sweep of {} lights rendered {} times, its last shown in {:.1} ms; light gone {restored}, settings untouched {written}",
                    order.borrow().len(),
                    lights[0].len(),
                    centiles(cold.iter().skip(1).copied().collect()),
                    first.map_or(f64::NAN, |d| d.as_secs_f64() * 1000.0),
                    centiles(warm),
                    leg(0),
                    leg(1),
                    leg(2),
                    sweep.len(),
                    swept,
                    last.map_or(f64::NAN, |d| d.as_secs_f64() * 1000.0),
                ));
            }
        }
    });
}
