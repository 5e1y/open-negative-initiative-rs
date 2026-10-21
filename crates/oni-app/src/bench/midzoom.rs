//! `ONI_BENCH=midzoom`: at 60 %, between fit and full size, a pan then a pinch about that zoom, the
//! full resolution built first, then Exposure dragged there: the histogram holds still under the view's
//! moves and follows the drag.

use super::*;

/// The scale held, drawable pixels per full-resolution pixel.
const AT: f32 = 0.6;

#[derive(Clone, Copy)]
enum Mid {
    Loading,
    Zooming { at: Instant },
    Panning { step: usize, from: usize },
    Pinching { event: usize, pan_from: usize, from: usize },
    Dragging { notch: usize, held: usize, report: usize },
    Landing { at: Instant, held: usize, dragged: usize },
}

pub(super) fn midzoom(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let phase = Cell::new(Mid::Loading);
    let done = RefCell::new(Some(done));
    let started = Instant::now();
    let drawn: RefCell<Rc<RefCell<Vec<(Instant, Duration)>>>> = RefCell::default();
    let marks = Cell::new((started, started));
    let text = RefCell::new(String::new());
    let before = Cell::new(0usize);
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
            Mid::Loading => {
                if w.window().size().width < (WINDOW.0 * w.window().scale_factor()) as u32 {
                    w.window().set_size(slint::LogicalSize::new(WINDOW.0, WINDOW.1));
                }
                let fitted = s.placed().filter(|p| p.whole && s.full_built());
                if let Some(p) = fitted {
                    drop(s);
                    c.invoke_pinch_started();
                    c.invoke_pinched(AT / (p.width / p.full.0) as f32, c.get_width() / 2.0, c.get_height() / 2.0);
                    phase.set(Mid::Zooming { at: Instant::now() });
                } else if started.elapsed() > FULL {
                    drop(s);
                    finish(format!("bench, midzoom: no full resolution in {} s", FULL.as_secs()));
                }
            }
            Mid::Zooming { at } => {
                if at.elapsed() < RUNG_HOLD {
                    return;
                }
                let from = s.views().count();
                before.set(s.notches().histogram_count());
                drop(s);
                *drawn.borrow_mut() = frame_clock(&ctl);
                marks.set((Instant::now(), Instant::now()));
                c.invoke_grabbed(400.0, 300.0);
                phase.set(Mid::Panning { step: 0, from });
            }
            Mid::Panning { step, from } => {
                drop(s);
                let step = step + 1;
                c.invoke_dragged(400.0 - step as f32 * PAN_STEP, 300.0 - step as f32 * PAN_STEP / 2.0);
                if step < PANS {
                    phase.set(Mid::Panning { step, from });
                } else {
                    c.invoke_pinch_started();
                    marks.set((marks.get().0, Instant::now()));
                    phase.set(Mid::Pinching { event: 0, pan_from: from, from: ctl.state.borrow().views().count() });
                }
            }
            Mid::Pinching { event, pan_from, from } => {
                let event = event + 1;
                if event <= PINCH_EVENTS {
                    drop(s);
                    // About the held zoom, a third up and back down, one event a 120 Hz frame.
                    let k = (std::f32::consts::TAU * event as f32 / PINCH_EVENTS as f32).sin();
                    c.invoke_pinched(1.33f32.powf(k), c.get_width() / 2.0, c.get_height() / 2.0);
                    phase.set(Mid::Pinching { event, pan_from, from });
                    return;
                }
                let frames = drawn.borrow().borrow().clone();
                let (pan_at, pinch_at) = marks.get();
                let pan_frames: Vec<_> = frames.iter().copied().filter(|(t, _)| *t < pinch_at).collect();
                let report = format!(
                    "bench, midzoom at {:.0} %:\n{}\n  {}\n{}\n  {}",
                    AT * 100.0,
                    s.views().summary_between(pan_from, from, &format!("{PANS} pan steps")),
                    frames_since(&pan_frames, pan_at),
                    s.views().summary(from, &format!("a pinch of {PINCH_EVENTS} events about it")),
                    frames_since(&frames, pinch_at),
                );
                let held = s.notches().histogram_count();
                drop(s);
                *text.borrow_mut() = report;
                w.global::<StoreGlobal>().invoke_press(Field::Exposure.index(), false);
                phase.set(Mid::Dragging { notch: 0, held, report: 0 });
            }
            Mid::Dragging { notch, held, .. } => {
                drop(s);
                let g = w.global::<StoreGlobal>();
                if notch < NOTCHES / 2 {
                    g.invoke_drag(Field::Exposure.index(), (notch + 1) as f32 * STEP, SPAN, false);
                    phase.set(Mid::Dragging { notch: notch + 1, held, report: 0 });
                } else {
                    g.invoke_release(Field::Exposure.index());
                    phase.set(Mid::Landing { at: Instant::now(), held, dragged: 0 });
                }
            }
            Mid::Landing { at, held, .. } => {
                if at.elapsed() < Duration::from_millis(500) {
                    return;
                }
                let dragged = s.notches().histogram_count() - held;
                drop(s);
                ctl.act("undo");
                ctl.flush();
                let moved = text.borrow().clone();
                finish(format!("{moved}
histograms zoomed: {} drawn through the pan and the pinch, {dragged} through the drag", held - before.get()));
            }
        }
    });
}
