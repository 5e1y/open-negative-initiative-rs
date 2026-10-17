//! A Slint proof of concept for the ONI interface: one keycap slider, Lighting's Exposure, driving
//! `Stage47::print_stops` on a roll #8 frame rendered by `oni_render::cpu::stage47`.

pub mod frame;
pub mod keycap;
pub mod motion;
pub mod track;

use std::cell::RefCell;
use std::ops::RangeInclusive;
use std::rc::Rc;
use std::time::{Duration, Instant};

use oni_render::Stage47;
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer, Timer, TimerMode};

use frame::{Frame, display_rgba8};
use motion::Spring;
use track::{Hand, Offer};

slint::include_modules!();

/// `Print.exposureRange` and `Tone.neutral.exposure`, the slider's travel and its rest.
pub const EXPOSURE_RANGE: RangeInclusive<f32> = -4.0..=4.0;
pub const EXPOSURE_BASE: f32 = 0.0;
/// Slint's and AppKit's default double-click interval.
pub const DOUBLE_CLICK: Duration = Duration::from_millis(500);
/// `DSScroll.trackpadTravel`: points of travel per point of trackpad glide.
pub const TRACKPAD_TRAVEL: f32 = 0.25;
const FRAME_TICK: Duration = Duration::from_millis(16);

/// `PrintPanel.row("Exposure", …, "%+.2f EV")`.
pub fn exposure_readout(v: f32) -> String {
    format!("{v:+.2} EV")
}

/// Everything the slider and the canvas hold between two events.
pub struct State {
    frame: Frame,
    stage: Stage47,
    /// The binding: what the settings hold.
    value: f32,
    /// The dragged value, drawn at once and handed to `value` thirty times a second.
    live: Option<f32>,
    start: Option<f32>,
    resetting: bool,
    last_press: Option<Instant>,
    hand: Hand,
    shown: f32,
    arrival: Option<(f32, f32, Instant)>,
    renders: usize,
}

impl State {
    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn renders(&self) -> usize {
        self.renders
    }

    fn position(&self, v: f32) -> f32 {
        track::position(v, &EXPOSURE_RANGE, None)
    }

    /// `DSSlider.fraction`: where the value the slider shows sits, never where a spring has the knob.
    fn fraction(&self) -> f32 {
        self.position(self.live.unwrap_or(self.value))
    }
}

/// The window and what its callbacks reach; the callbacks hold the window weakly, never in a cycle.
pub struct Poc {
    pub window: MainWindow,
    pub state: Rc<RefCell<State>>,
}

struct Ctl {
    window: slint::Weak<MainWindow>,
    state: Rc<RefCell<State>>,
    trailing: Timer,
    arrival: Timer,
}

/// The canvas copy through stages 4 to 7, then to the display, 8 bits only at that last step.
pub fn rendered(frame: &Frame, stage: &Stage47) -> Image {
    let linear = frame.converted(stage);
    let bytes = display_rgba8(&linear);
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&bytes, linear.width as u32, linear.height as u32))
}

/// The knob raster at `scale` device pixels per point.
pub fn knob_image(scale: f64) -> Image {
    let (w, h, bytes) = keycap::knob_pixels(&keycap::Style::knob(), scale);
    Image::from_rgba8_premultiplied(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&bytes, w as u32, h as u32))
}

impl Poc {
    pub fn new(frame: Frame, scale: f64) -> Result<Poc, slint::PlatformError> {
        let window = MainWindow::new()?;
        let stage = frame.set.stage47;
        let value = stage.print_stops;
        let state = Rc::new(RefCell::new(State {
            frame,
            stage,
            value,
            live: None,
            start: None,
            resetting: false,
            last_press: None,
            hand: Hand::default(),
            shown: track::position(value, &EXPOSURE_RANGE, None),
            arrival: None,
            renders: 0,
        }));
        window.set_knob(knob_image(scale));
        let ctl = Rc::new(Ctl { window: window.as_weak(), state: state.clone(), trailing: Timer::default(), arrival: Timer::default() });
        ctl.commit(value);
        let shown = state.borrow().shown;
        ctl.show_fraction(shown);
        Ctl::wire(&ctl, &window);
        Ok(Poc { window, state })
    }
}

impl Ctl {
    fn show_fraction(&self, f: f32) {
        self.state.borrow_mut().shown = f;
        if let Some(w) = self.window.upgrade() {
            w.set_exposure_fraction(f);
        }
    }

    fn set_readout(&self, v: f32) {
        if let Some(w) = self.window.upgrade() {
            w.set_exposure_readout(exposure_readout(v).into());
        }
    }

    /// A gesture places the knob itself: a spring still running from an arrival stops where it is.
    fn stop_arrival(&self) {
        self.state.borrow_mut().arrival = None;
        self.arrival.stop();
    }

    /// The value reaches the settings: the readout says it and the canvas is rendered again.
    fn commit(&self, v: f32) {
        let image = {
            let mut s = self.state.borrow_mut();
            s.value = v;
            s.stage.print_stops = v;
            s.renders += 1;
            rendered(&s.frame, &s.stage)
        };
        if let Some(w) = self.window.upgrade() {
            w.set_frame(image);
            w.set_exposure_value(exposure_readout(v).into());
        }
        let shown = self.state.borrow().live.unwrap_or(v);
        self.set_readout(shown);
    }

    /// A value arriving from elsewhere springs the knob with `settle`; the canvas takes it at once.
    fn arrive(self: &Rc<Self>, v: f32) {
        self.commit(v);
        let (from, to) = {
            let mut s = self.state.borrow_mut();
            let to = s.position(v);
            s.arrival = Some((s.shown, to, Instant::now()));
            (s.shown, to)
        };
        if from == to {
            return;
        }
        let me = Rc::downgrade(self);
        self.arrival.start(TimerMode::Repeated, FRAME_TICK, move || {
            let Some(me) = me.upgrade() else { return };
            let arrival = me.state.borrow().arrival;
            let Some((from, to, at)) = arrival else { return me.arrival.stop() };
            let t = at.elapsed().as_secs_f64();
            let spring = Spring::SETTLE;
            if t >= spring.settled_after() {
                me.state.borrow_mut().arrival = None;
                me.show_fraction(to);
                me.arrival.stop();
            } else {
                me.show_fraction(from + (to - from) * spring.value(t) as f32);
            }
        });
    }

    fn wire(me: &Rc<Ctl>, window: &MainWindow) {
        let p = me.clone();
        window.on_press(move |_fine| {
            let now = Instant::now();
            let double = {
                let mut s = p.state.borrow_mut();
                // `DSClick` reads AppKit's click count, which keeps counting past two.
                let double = s.last_press.is_some_and(|t| now.duration_since(t) <= DOUBLE_CLICK);
                s.last_press = Some(now);
                s.resetting = double;
                s.start = if double { None } else { Some(s.fraction()) };
                double
            };
            if double {
                p.arrive(EXPOSURE_BASE);
            }
            double
        });

        let p = me.clone();
        window.on_drag(move |translation, span, fine| {
            let (asked, offer) = {
                let mut s = p.state.borrow_mut();
                let (false, Some(start)) = (s.resetting, s.start) else { return };
                let asked = track::clamped(track::asked(start, translation, span, fine));
                s.live = Some(track::value(asked, &EXPOSURE_RANGE, None));
                (asked, s.hand.offer(Instant::now()))
            };
            p.stop_arrival();
            p.show_fraction(asked);
            let live = p.state.borrow().live.unwrap_or(EXPOSURE_BASE);
            p.set_readout(live);
            match offer {
                Offer::Now => p.commit(live),
                Offer::After(delay) => {
                    let q = Rc::downgrade(&p);
                    p.trailing.start(TimerMode::SingleShot, delay, move || {
                        let Some(q) = q.upgrade() else { return };
                        q.state.borrow_mut().hand.trailed(Instant::now());
                        let live = q.state.borrow().live;
                        if let Some(live) = live {
                            q.commit(live);
                        }
                    });
                }
                Offer::Wait => {}
            }
        });

        let p = me.clone();
        window.on_release(move || {
            let live = {
                let mut s = p.state.borrow_mut();
                s.start = None;
                s.resetting = false;
                s.hand.released();
                s.live.take()
            };
            p.trailing.stop();
            if let Some(live) = live {
                p.commit(live);
            }
        });

        // A scroll along the track writes the binding at each event; one across it scrolls the panel.
        let p = me.clone();
        window.on_scroll(move |dx, dy, span, fine| {
            if dx == 0.0 || dx.abs() <= dy.abs() {
                return false;
            }
            p.stop_arrival();
            let (from, value) = {
                let s = p.state.borrow();
                (s.fraction(), s.value)
            };
            let asked = track::clamped(track::asked(from, dx * TRACKPAD_TRAVEL, span, fine));
            p.show_fraction(asked);
            let next = track::value(asked, &EXPOSURE_RANGE, None);
            if next != value {
                p.commit(next);
            }
            true
        });

        let p = me.clone();
        window.on_step(move |direction| {
            let step = (EXPOSURE_RANGE.end() - EXPOSURE_RANGE.start()) / 100.0;
            let v = (p.state.borrow().value + direction as f32 * step).clamp(*EXPOSURE_RANGE.start(), *EXPOSURE_RANGE.end());
            p.arrive(v);
        });
    }
}
