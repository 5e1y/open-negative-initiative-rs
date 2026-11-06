//! The starter: the app's own window opens at once on the version's picture, a band beneath it saying
//! each step of the launch, then the window grows into the app the moment the last step lands.

use crate::render::Kernels;
use crate::{MainWindow, Starter};
use oni_i18n::tr;
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, Timer, TimerMode};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The pictures shipped, oldest first, each named after the version it opens.
const PICTURES: [(&str, &str); 3] = [
    ("1.0.0", include_str!("../../oni-ui/ui/splash/1.0.0.txt")),
    ("1.3.0", include_str!("../../oni-ui/ui/splash/1.3.0.txt")),
    ("1.5.0", include_str!("../../oni-ui/ui/splash/1.5.0.txt")),
];

/// One frame of a 60 Hz screen: a step starts that long after the band changed, so the band is drawn
/// before the step holds the interface thread.
const DRAWN: Duration = Duration::from_millis(17);
/// How often the band reads the clock and the kernels' count.
const TICK: Duration = Duration::from_millis(100);
/// The last step, which waits on the kernels' thread rather than running on the interface's.
const KERNELS: &str = oni_i18n::marked("Compiling the image kernels");

/// The version's own picture, else the newest.
pub fn picture(version: &str) -> usize {
    PICTURES.iter().position(|(v, _)| *v == version).unwrap_or(PICTURES.len() - 1)
}

/// The credit printed along the version's picture.
pub fn credit(version: &str) -> String {
    PICTURES[picture(version)].1.trim().to_string()
}

/// A step of the launch: its words, English in the log and translated on the band, and its work on
/// the interface thread.
pub struct Step {
    words: &'static str,
    run: Box<dyn FnOnce()>,
}

pub fn step(words: &'static str, run: impl FnOnce() + 'static) -> Step {
    Step { words, run: Box::new(run) }
}

impl Step {
    /// The step's work at once, where no starter shows it.
    pub(crate) fn run(self) {
        (self.run)()
    }
}

/// Steps a running step found, run next and counted on the bar; at once where no launch runs.
pub fn more(steps: Vec<Step>) {
    let Some(launch) = LAUNCH.with(|l| l.borrow().clone()) else {
        return steps.into_iter().for_each(Step::run);
    };
    launch.total.set(launch.total.get() + steps.len());
    let mut queue = launch.steps.borrow_mut();
    for step in steps.into_iter().rev() {
        queue.push_front(step);
    }
}

/// The parts of one step, each written to the log as it ends: the band cannot redraw inside a step.
pub struct Laps {
    step: &'static str,
    at: Instant,
}

impl Laps {
    pub fn new(step: &'static str) -> Laps {
        Laps { step, at: Instant::now() }
    }

    pub fn lap(&mut self, part: &str) {
        crate::timing::line(&format!("boot: {} · {part} in {:.0} ms", self.step, ms(self.at.elapsed())));
        self.at = Instant::now();
    }
}

/// The share of the launch done: each step one part, the kernels one more, filled as they compile.
pub fn fraction(steps_done: usize, steps: usize, kernels_done: usize, kernels: usize) -> f32 {
    let kernels = if kernels == 0 { 1.0 } else { kernels_done.min(kernels) as f32 / kernels as f32 };
    (steps_done.min(steps) as f32 + kernels) / (steps + 1) as f32
}

struct Launch {
    window: slint::Weak<MainWindow>,
    began: Instant,
    steps: RefCell<VecDeque<Step>>,
    total: std::cell::Cell<usize>,
    done: RefCell<Vec<(&'static str, Duration)>>,
    current: RefCell<(&'static str, Instant)>,
    kernels: Arc<Kernels>,
    ticker: Timer,
    ready: RefCell<Option<Box<dyn FnOnce()>>>,
}

/// The window shown as the starter, the steps `before` it already done, then `steps` one by one, a
/// frame of the band drawn between two, and `ready` once the kernels are compiled too.
pub fn launch(window: &MainWindow, began: Instant, before: Vec<(&'static str, Duration)>, steps: Vec<Step>, kernels: Arc<Kernels>, ready: impl FnOnce() + 'static) -> Result<(), slint::PlatformError> {
    let version = env!("CARGO_PKG_VERSION");
    let band = window.global::<Starter>();
    band.set_picture(picture(version) as i32);
    band.set_line(tr!("V{version} Loading", version = version).into());
    band.set_credit(credit(version).into());
    band.set_shown(true);
    for (words, took) in &before {
        crate::timing::line(&format!("boot: {words} in {:.0} ms", ms(*took)));
    }
    let launch = Rc::new(Launch {
        window: window.as_weak(),
        began,
        total: std::cell::Cell::new(before.len() + steps.len()),
        steps: RefCell::new(steps.into()),
        done: RefCell::new(before),
        current: RefCell::new((KERNELS, Instant::now())),
        kernels,
        ticker: Timer::default(),
        ready: RefCell::new(Some(Box::new(ready))),
    });
    sized(window, band.get_width(), band.get_height());
    window.show()?;
    crate::timing::line(&format!("boot: the starter shown {:.0} ms after the launch", ms(began.elapsed())));
    let ticking = Rc::downgrade(&launch);
    launch.ticker.start(TimerMode::Repeated, TICK, move || {
        if let Some(l) = ticking.upgrade() {
            l.tick();
        }
    });
    LAUNCH.with(|l| *l.borrow_mut() = Some(launch.clone()));
    launch.next();
    Ok(())
}

thread_local! {
    /// The launch under way, held until the app opens: past its last step it only waits on the kernels.
    static LAUNCH: RefCell<Option<Rc<Launch>>> = const { RefCell::new(None) };
}

impl Launch {
    /// The next step on the band, run once a frame of it is drawn; past the last, the kernels' wait.
    fn next(self: &Rc<Self>) {
        let Some(step) = self.steps.borrow_mut().pop_front() else {
            *self.current.borrow_mut() = (KERNELS, Instant::now());
            self.tick();
            return;
        };
        *self.current.borrow_mut() = (step.words, Instant::now());
        self.publish();
        let me = self.clone();
        Timer::single_shot(DRAWN, move || {
            let started = Instant::now();
            (step.run)();
            me.finished(step.words, started.elapsed());
            me.next();
        });
    }

    fn finished(&self, words: &'static str, took: Duration) {
        crate::timing::line(&format!("boot: {words} in {:.0} ms", ms(took)));
        self.done.borrow_mut().push((words, took));
    }

    /// The clock and the kernels' count on the band; the app once every step and the kernels are done.
    fn tick(self: &Rc<Self>) {
        let waiting = self.steps.borrow().is_empty() && self.current.borrow().0 == KERNELS;
        if waiting && self.kernels.finished() && self.ready.borrow().is_some() {
            let since = self.current.borrow().1.elapsed();
            self.finished(KERNELS, since);
            return self.open();
        }
        self.publish();
    }

    fn publish(&self) {
        let Some(w) = self.window.upgrade() else { return };
        let band = w.global::<Starter>();
        let (words, _) = *self.current.borrow();
        let stage = if words == KERNELS {
            tr!("{step} · {count}", step = tr(words), count = tr!("{done} of {total}", done = self.kernels.done(), total = Kernels::total()))
        } else {
            tr(words).to_string()
        };
        band.set_stage(stage.into());
        band.set_elapsed(seconds(self.began.elapsed()).into());
        let done = self.done.borrow();
        let line = done.iter().map(|(w, took)| tr!("{step} {seconds}", step = tr(w), seconds = seconds(*took))).collect::<Vec<_>>().join(" · ");
        band.set_done(line.into());
        band.set_fraction(fraction(done.len(), self.total.get(), self.kernels.done(), Kernels::total()));
    }

    /// The starter gone and the window at the app's size, centred where it stood, in one turn.
    fn open(self: &Rc<Self>) {
        self.ticker.stop();
        if let Some(w) = self.window.upgrade() {
            let band = w.global::<Starter>();
            band.set_shown(false);
            let (width, height) = asked_size().unwrap_or((band.get_app_width(), band.get_app_height()));
            sized(&w, width, height);
        }
        crate::timing::line(&format!("boot: the app ready {:.0} ms after the launch", ms(self.began.elapsed())));
        let ready = self.ready.borrow_mut().take();
        if let Some(ready) = ready {
            ready();
        }
        LAUNCH.with(|l| l.borrow_mut().take());
    }
}

/// `ONI_WINDOW=<width>x<height>`: the window's size in points, the one the interface evaluations look at.
fn asked_size() -> Option<(f32, f32)> {
    let value = std::env::var("ONI_WINDOW").ok()?;
    let (width, height) = value.split_once('x')?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

/// The window at `width` × `height` points, centred on its screen.
fn sized(window: &MainWindow, width: f32, height: f32) {
    window.window().set_size(slint::LogicalSize::new(width, height));
    window.window().with_winit_window(|w| {
        if let Some(screen) = w.current_monitor() {
            let scale = w.scale_factor();
            let (s, o) = (screen.size(), screen.position());
            let (pw, ph) = ((f64::from(width) * scale) as i32, (f64::from(height) * scale) as i32);
            w.set_outer_position(slint::winit_030::winit::dpi::PhysicalPosition::new(o.x + (s.width as i32 - pw) / 2, o.y + (s.height as i32 - ph) / 2));
        }
    });
}

fn seconds(d: Duration) -> String {
    tr!("{seconds} s", seconds = format!("{:.1}", d.as_secs_f64()))
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_shows_its_own_picture_and_any_other_the_newest() {
        assert_eq!(picture("1.0.0"), 0);
        assert_eq!(picture("1.3.0"), 1);
        assert_eq!(picture("1.5.0"), 2);
        // Twin: a version with no picture of its own falls on the newest, never the first.
        assert_eq!(picture("0.1.0"), PICTURES.len() - 1);
        assert!(PICTURES.iter().all(|(_, credit)| !credit.trim().is_empty()));
    }

    #[test]
    fn the_bar_fills_with_the_steps_and_the_kernels_and_ends_full() {
        assert_eq!(fraction(0, 3, 0, 27), 0.0);
        assert_eq!(fraction(3, 3, 27, 27), 1.0);
        assert!((fraction(3, 3, 0, 27) - 0.75).abs() < 1e-6, "the kernels weigh one step");
        assert!(fraction(2, 3, 27, 27) < 1.0, "a step left keeps the bar short of the end");
        // Twin: counts past their totals never push the bar beyond full.
        assert_eq!(fraction(9, 3, 99, 27), 1.0);
    }
}
