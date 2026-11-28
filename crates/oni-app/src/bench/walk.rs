//! `ONI_BENCH=walk`: the window the size of its screen, the editor walked by its arrows as a hand does, slowly,
//! in a burst and held down as the key repeats, each walk's line in the log; the report counts the walks drawn.

use super::*;
use slint::platform::{Key, WindowEvent};

/// One way of walking: how many presses, the gap between them, the arrow, and whether the key is held.
struct Pace {
    name: &'static str,
    presses: usize,
    gap: Duration,
    key: Key,
    held: bool,
}

/// A hand's paces: one photo looked at, then the next; a few quick presses; the key held at macOS's fastest repeat.
const PACES: [Pace; 5] = [
    Pace { name: "slow forward", presses: 5, gap: Duration::from_millis(1200), key: Key::RightArrow, held: false },
    Pace { name: "slow back", presses: 5, gap: Duration::from_millis(1200), key: Key::LeftArrow, held: false },
    Pace { name: "burst forward", presses: 8, gap: Duration::from_millis(150), key: Key::RightArrow, held: false },
    Pace { name: "held forward", presses: 15, gap: Duration::from_millis(33), key: Key::RightArrow, held: true },
    Pace { name: "held back", presses: 15, gap: Duration::from_millis(33), key: Key::LeftArrow, held: true },
];

/// The breath between two paces, past a walk's whole travel.
const REST: Duration = Duration::from_millis(1500);

#[derive(Clone, Copy, PartialEq)]
enum Step {
    Loading,
    Sizing { since: Instant },
    Walking { pace: usize, press: usize, at: Instant },
    Resting { pace: usize, since: Instant },
}

pub(super) fn walk(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let step = Cell::new(Step::Loading);
    let started = Instant::now();
    let done = RefCell::new(Some(done));
    let lines = RefCell::new(Vec::<String>::new());
    // The frames the walks' lines count are the ones this clock sees drawn.
    let _drawn = frame_clock(ctl);
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(5), move || {
        let timer = &held;
        let Some(ctl) = me.upgrade() else { return timer.stop() };
        let Some(w) = ctl.window() else { return };
        let finish = |text: String| {
            timer.stop();
            if let Some(done) = done.borrow_mut().take() {
                done(text);
            }
        };
        let walks = || w.global::<crate::Walk>().get_serial();
        match step.get() {
            Step::Loading => {
                if ctl.state.borrow().renders() >= 1 && ctl.state.borrow().placed().is_some() {
                    maximise(&w);
                    step.set(Step::Sizing { since: Instant::now() });
                } else if started.elapsed() > LOADING {
                    finish("walk: no frame rendered".into());
                }
            }
            Step::Sizing { since } => {
                if since.elapsed() < Duration::from_secs(2) {
                    return;
                }
                if w.global::<crate::App>().get_gallery() || w.global::<crate::Preparing>().get_shown() {
                    return finish("walk: the photo opened is not beside the editor".into());
                }
                step.set(Step::Walking { pace: 0, press: 0, at: Instant::now() });
            }
            Step::Walking { pace, press, at } => {
                let p = &PACES[pace];
                if at.elapsed() < p.gap && press > 0 {
                    return;
                }
                if press == 0 {
                    lines.borrow_mut().push(format!("{}: from walk {}", p.name, walks()));
                }
                let text: slint::SharedString = p.key.into();
                w.invoke_refocus();
                let event = if p.held && press > 0 { WindowEvent::KeyPressRepeated { text: text.clone() } } else { WindowEvent::KeyPressed { text: text.clone() } };
                w.window().dispatch_event(event);
                if !p.held || press + 1 == p.presses {
                    w.window().dispatch_event(WindowEvent::KeyReleased { text });
                }
                if press + 1 < p.presses {
                    step.set(Step::Walking { pace, press: press + 1, at: Instant::now() });
                } else {
                    step.set(Step::Resting { pace, since: Instant::now() });
                }
            }
            Step::Resting { pace, since } => {
                if since.elapsed() < REST {
                    return;
                }
                let p = &PACES[pace];
                lines.borrow_mut().push(format!("{}: to walk {}", p.name, walks()));
                if pace + 1 < PACES.len() {
                    step.set(Step::Walking { pace: pace + 1, press: 0, at: Instant::now() });
                } else {
                    finish(format!("walk:\n{}", lines.borrow().join("\n")));
                }
            }
        }
    });
}
