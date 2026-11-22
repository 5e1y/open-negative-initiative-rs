//! `ONI_BENCH=stocks` and `ONI_BENCH=papers`: the open photo's stocks' or papers' menu laid, each line lit in turn as
//! the pointer resting on it lights it, once cold and once warm, then every line swept at a trackpad's pace; each
//! light timed to the GPU's end of the first frame showing its render, which the photon follows by one refresh at most.

use super::chain::{self, centiles, photon};
use super::*;

/// The menu a run lights: the film's, or the Colour tab's papers.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Lights {
    Stocks,
    Papers,
}

impl Lights {
    fn name(self) -> &'static str {
        match self {
            Lights::Stocks => "stocks",
            Lights::Papers => "papers",
        }
    }

    /// The menu laid as its key drops it: its lines' places in the catalogue, the one in use left out.
    fn drop(self, w: &crate::MainWindow) -> Vec<i32> {
        let (lines, own) = match self {
            Lights::Stocks => {
                let g = w.global::<crate::Stocks>();
                g.invoke_dropping();
                (g.get_lines(), g.get_in_use())
            }
            Lights::Papers => {
                let g = w.global::<crate::PanelState>();
                g.invoke_paper_dropping();
                (g.get_paper_lines(), g.get_paper_in_use())
            }
        };
        (0..lines.row_count()).filter(|&i| i as i32 != own).filter_map(|i| lines.row_data(i)).filter(|l| !l.brand).map(|l| l.index).collect()
    }

    fn light(self, w: &crate::MainWindow, i: i32) {
        match self {
            Lights::Stocks => w.global::<crate::Stocks>().invoke_lit(i),
            Lights::Papers => w.global::<crate::PanelState>().invoke_paper_lit(i),
        }
    }
}

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

pub(super) fn lights(ctl: &Rc<Ctl>, menu: Lights, done: impl FnOnce(String) + 'static) {
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
                        finish(format!("{}: no frame rendered", menu.name()));
                    }
                    return;
                }
                let panel = w.global::<crate::PanelState>();
                panel.set_tab(crate::PanelTab::Color);
                panel.invoke_tab_picked(crate::PanelTab::Color);
                if menu == Lights::Papers && ctl.state.borrow().store().settings().paper().is_none() {
                    return finish("papers: the photo prints on no paper".into());
                }
                *order.borrow_mut() = menu.drop(&w);
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
                    menu.light(&w, -1);
                    phase.set(if pass == 0 { St::Lighting { pass: 1, k: 0, at: Instant::now() } } else { St::Sweeping { k: 0 } });
                    return;
                };
                // Timed only where the light moved what the canvas shows: one that changes nothing renders nothing.
                let (shown, at) = (ctl.state.borrow().shown_settings().cloned(), Instant::now());
                menu.light(&w, i);
                if ctl.state.borrow().shown_settings() != shown.as_ref() {
                    lit.borrow_mut()[pass].push(at);
                }
                phase.set(St::Lighting { pass, k: k + 1, at: Instant::now() });
            }
            St::Sweeping { k } => match order.borrow().get(k) {
                Some(&i) => {
                    lit.borrow_mut()[2].push(Instant::now());
                    menu.light(&w, i);
                    phase.set(St::Sweeping { k: k + 1 });
                }
                None => phase.set(St::Settling { at: Instant::now() }),
            },
            St::Settling { at } => {
                if at.elapsed() < REST * 2 {
                    return;
                }
                let own = ctl.state.borrow().store().settings().clone();
                menu.light(&w, -1);
                let restored = ctl.state.borrow().stock_lit().is_none() && ctl.state.borrow().paper_lit().is_none();
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
                    "{}: {} lit, {} moving the photo, hover to photon cold {} (first {:.1} ms), warm {} (to the encode {}, to the next frame {}, the frame {}); \
                     a sweep of {} lights rendered {} times, its last shown in {:.1} ms; light gone {restored}, settings untouched {written}",
                    menu.name(),
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
