//! The welcome's stage in the window: its acts on one clock, each picture drawn by the GPU into the texture the
//! sheet shows, the gifts' places told back so the hand finds them. A check holds the clock and steps it.

use super::{EDITION, WELCOME_KEY};
use crate::app::{Ctl, SHELL};
use crate::stage::direction::{self, Act, Cue, OVERTURE};
use crate::stage::{Stage, film, unwrap};
use crate::{Help, StageSpot};
use oni_i18n::tr;
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, Image, ModelRc, SharedPixelBuffer, Timer, TimerMode, VecModel};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// A picture every 8 ms at the most: the pace of a 120 Hz screen.
const PACE: Duration = Duration::from_millis(8);
/// The most pixels a picture holds: past it the stage is drawn smaller and the window stretches it.
const BUDGET: f32 = 2.6e6;
/// The share of that budget a slow GPU is let down to, and the share of a screen's frame a picture may take.
const LEAST: f32 = 0.3;
const SHARE: f32 = 0.7;
/// How much finer than the window's own pixels the stage may be drawn, each way.
const FINER: f32 = 1.4;
/// How fast a gift answers the hand, a share of the way each second.
const ANSWER: f32 = 9.0;

/// The screen's frame, in seconds: a 60 Hz one where the window's screen tells no rate.
fn screen_frame(w: &crate::MainWindow) -> f32 {
    let rate = w.window().with_winit_window(|window| window.current_monitor().and_then(|m| m.refresh_rate_millihertz())).flatten().unwrap_or(60_000);
    1000.0 / (rate as f32).clamp(30_000.0, 240_000.0)
}

/// The stage while the welcome shows.
pub(in crate::app) struct Playing {
    stage: Option<Stage>,
    /// The pipeline being built off this thread, so the sheet opens at once.
    building: Option<Receiver<(Stage, f32)>>,
    /// The seconds this GPU takes over a million pixels of the stage, and the most pixels it draws in a frame.
    pace: f32,
    most: f32,
    target: Option<wgpu::Texture>,
    revision: i32,
    began: Instant,
    /// A check's clock, in seconds, in place of the time since the welcome opened.
    held: Option<f32>,
    act: Act,
    act_began: f32,
    before: Option<(Act, f32)>,
    /// The page the act answers, which a turn of the sheet changes under it.
    page: i32,
    opened: [bool; 4],
    hover: [f32; 4],
    hovered: Option<usize>,
    pointer: (f32, f32),
    leaned: (f32, f32),
    step: usize,
    step_began: f32,
    /// A step the person picked holds until they pick another.
    step_held: bool,
    compare: Option<f32>,
    /// The sheet's size, the words' column and the footer's band, in the window's points.
    size: (f32, f32),
    words: f32,
    foot: f32,
    last: f32,
    /// Whether the GPU is still drawing the last picture handed to it.
    flying: Arc<AtomicBool>,
    /// Kept so its frames keep coming; dropped with the stage.
    _timer: Timer,
}

impl Playing {
    fn now(&self) -> f32 {
        self.held.unwrap_or_else(|| self.began.elapsed().as_secs_f32())
    }

    fn enter(&mut self, act: Act, now: f32) {
        self.before = Some((self.act, now - self.act_began));
        // The overture ends on the hall as it stands: its words are there already.
        self.act_began = if self.act == Act::Overture && act == Act::Hall { now - 60.0 } else { now };
        self.act = act;
        if let Act::Page(2) = act {
            (self.step, self.step_began, self.step_held, self.compare) = (0, now, false, None);
        }
    }

    /// The act the sheet's page asks for: a gift still wrapped is untied first, unless nothing may move.
    fn follow(&mut self, page: i32, now: f32, still: bool) {
        if page == self.page {
            return;
        }
        self.page = page;
        let act = match page {
            1..=4 if self.opened[page as usize - 1] || still => Act::Page(page as usize - 1),
            1..=4 => Act::Unwrap(page as usize - 1),
            5 => Act::Message,
            _ => Act::Hall,
        };
        if let Act::Page(i) = act {
            self.opened[i] = true;
        }
        self.enter(act, now);
    }

    /// The acts that end by themselves, and the film's steps that follow one another.
    fn advance(&mut self, now: f32) {
        let t = now - self.act_began;
        match self.act {
            Act::Overture if t >= OVERTURE => self.enter(Act::Hall, now),
            Act::Unwrap(i) if t >= unwrap::length(i) => {
                self.opened[i] = true;
                self.enter(Act::Page(i), now);
            }
            Act::Page(2) if !self.step_held && now - self.step_began >= film::length(self.step) && self.step + 1 < film::STEPS => {
                self.step += 1;
                self.step_began = now;
            }
            _ => {}
        }
    }

    fn cue(&mut self, now: f32) -> Cue {
        let passed = (now - self.last).clamp(0.0, 0.1);
        self.last = now;
        let near = 1.0 - (-ANSWER * passed).exp();
        for (i, h) in self.hover.iter_mut().enumerate() {
            *h += ((if self.hovered == Some(i) { 1.0 } else { 0.0 }) - *h) * near;
        }
        self.leaned = (self.leaned.0 + (self.pointer.0 - self.leaned.0) * near * 0.4, self.leaned.1 + (self.pointer.1 - self.leaned.1) * near * 0.4);
        let step_t = now - self.step_began;
        // Over the second stock's print the photo is laid back across half of it for a while, its edge swaying
        // once; a hand that takes the edge keeps it, until the three prints stand side by side.
        let (from, until) = film::COMPARE;
        let laid = crate::stage::glide(((step_t - from) / 1.1).clamp(0.0, 1.0)) * (1.0 - crate::stage::glide(((step_t - until) / 0.6).clamp(0.0, 1.0)));
        let sway = (step_t - from - 1.1).max(0.0);
        let printed = self.step == 4 && step_t > film::PRINTED && step_t < film::FINALE - 0.2;
        let shown = if printed { self.compare.unwrap_or(laid * (0.5 + 0.14 * (sway * 1.6).sin() * (-sway * 0.5).exp())) } else { 0.0 };
        Cue {
            act: self.act,
            t: now - self.act_began,
            clock: now,
            aspect: self.size.0 / self.size.1.max(1.0),
            // The huge gift's page is all stage: its words lie under the film, not beside it.
            words: if self.act == Act::Page(2) { 0.0 } else { self.words / self.size.1.max(1.0) },
            foot: self.foot / self.size.1.max(1.0),
            opened: self.opened,
            hover: self.hover,
            pointer: self.leaned,
            step: self.step,
            step_t,
            compare: shown,
            before: self.before,
        }
    }
}

impl Ctl {
    pub(super) fn wire_welcome(me: &std::rc::Rc<Ctl>, help: &Help) {
        let p = me.clone();
        help.on_welcome_sized(move |width, height, words, foot| p.welcome_set(|play| (play.size, play.words, play.foot) = ((width, height), words, foot)));
        let p = me.clone();
        help.on_welcome_pointer(move |x, y| p.welcome_set(|play| play.pointer = (x, y)));
        let p = me.clone();
        help.on_welcome_hover(move |gift| p.welcome_set(|play| play.hovered = usize::try_from(gift).ok()));
        let p = me.clone();
        help.on_welcome_hurry(move || p.welcome_hurry());
        let p = me.clone();
        help.on_welcome_step(move |step| {
            p.welcome_set(|play| {
                let now = play.now();
                let last = film::STEPS - 1;
                if step as usize > last && play.step == last {
                    // Past the last step, on to its next moment: its print, each other stock, the three together.
                    let t = now - play.step_began;
                    let moments = [film::PRINTED + 0.1, film::SECOND + 0.7, film::THIRD + 0.7, film::FINALE + 1.7];
                    let next = moments.into_iter().find(|m| *m > t + 0.3).unwrap_or(film::FINALE + 1.7);
                    (play.step_began, play.compare) = (now - next, None);
                } else {
                    (play.step, play.step_began, play.step_held, play.compare) = (step.clamp(0, last as i32) as usize, now, true, None);
                }
            });
            // Told to the sheet at once: its keys read the step they act on.
            p.welcome_frame();
        });
        let p = me.clone();
        help.on_welcome_compare(move |share| p.welcome_set(|play| play.compare = Some(share)));
    }

    fn welcome_set(&self, set: impl FnOnce(&mut Playing)) {
        if let Some(play) = self.state.borrow_mut().help.welcome.as_mut() {
            set(play);
        }
    }

    /// `WelcomeSheet.isDue`: this release's sheet not yet shown on this computer.
    pub fn welcome_due(&self) -> bool {
        super::welcome_due(self.state.borrow().preferences.string(WELCOME_KEY))
    }

    /// The welcome from its overture, every gift wrapped again; whether a window shows it.
    pub(in crate::app) fn open_welcome(&self) -> bool {
        let Some(w) = self.window() else { return false };
        let h = w.global::<Help>();
        h.set_year(oni_library::wrapped::today().0.to_string().into());
        let stocks: Vec<slint::SharedString> = film::STOCKS.iter().map(|key| oni_settings::VirtualStock::of(key).name().into()).collect();
        h.set_film_stocks(ModelRc::new(VecModel::from(stocks)));
        h.set_film_count(oni_settings::VirtualStock::sheets().len() as i32);
        h.set_welcome_unwrapped(0);
        h.set_welcome_page(0);
        let still = w.global::<crate::Motion>().get_reduced();
        let (sender, building) = channel();
        {
            let mut s = self.state.borrow_mut();
            let (device, queue) = (s.gpu.device.clone(), s.gpu.queue.clone());
            // Its shader compiles off this thread: the sheet and its keys are there at once.
            let _ = std::thread::Builder::new().name("stage".into()).spawn(move || {
                let mut stage = Stage::new(&device, &queue);
                let pace = stage.pace(&device, &queue);
                // A welcome closed meanwhile has dropped its end: the stage goes with this thread.
                let _ = sender.send((stage, pace));
            });
            let timer = Timer::default();
            timer.start(TimerMode::Repeated, PACE, || {
                if let Some(me) = SHELL.with(|s| s.borrow().upgrade()) {
                    me.welcome_frame();
                }
            });
            s.help.welcome = Some(Playing {
                stage: None,
                building: Some(building),
                pace: 0.0,
                most: BUDGET,
                target: None,
                revision: 0,
                began: Instant::now(),
                held: None,
                act: if still { Act::Hall } else { Act::Overture },
                act_began: 0.0,
                before: None,
                page: 0,
                opened: [false; 4],
                hover: [0.0; 4],
                hovered: None,
                pointer: (0.0, 0.0),
                leaned: (0.0, 0.0),
                step: 0,
                step_began: 0.0,
                step_held: false,
                compare: None,
                size: (0.0, 0.0),
                words: 0.0,
                foot: 0.0,
                last: 0.0,
                flying: Arc::new(AtomicBool::new(false)),
                _timer: timer,
            });
        }
        h.set_act(if still { 1 } else { 0 });
        h.set_act_time(0.0);
        h.set_stage(Image::default());
        h.set_welcome_shown(true);
        true
    }

    /// However it closes, it is not shown again on the next launch; its stage and its clock go with it.
    pub(super) fn welcome_closed(&self) {
        let Some(w) = self.window() else { return };
        if !w.global::<Help>().get_welcome_shown() {
            return;
        }
        w.global::<Help>().set_welcome_shown(false);
        let written = {
            let mut s = self.state.borrow_mut();
            s.help.welcome = None;
            s.preferences.set(WELCOME_KEY, EDITION.into())
        };
        if let Err(e) = written {
            self.say(&tr!("The welcome cannot be marked seen: {error}", error = e));
        }
        w.invoke_refocus();
        self.launch_sheet_after(Some(crate::app::shell::LaunchSheet::Welcome));
    }

    /// A scene that plays by itself brought to its end: the overture to the hall, an untying to its page.
    fn welcome_hurry(&self) {
        self.welcome_set(|play| {
            let now = play.now();
            match play.act {
                Act::Overture => play.act_began = now - OVERTURE,
                Act::Unwrap(i) => play.act_began = now - unwrap::length(i),
                _ => {}
            }
        });
    }

    /// A check's clock: the welcome at `seconds` since it opened, drawn; none gives the clock back to time.
    pub fn welcome_hold(&self, seconds: Option<f32>) {
        self.welcome_set(|play| play.held = seconds);
        self.welcome_frame();
    }

    /// One picture: the act the page asks, its moment on the clock, drawn and told to the sheet.
    pub(in crate::app) fn welcome_frame(&self) {
        let Some(w) = self.window() else { return };
        let help = w.global::<Help>();
        let (still, scale) = (w.global::<crate::Motion>().get_reduced(), w.window().scale_factor());
        let mut state = self.state.borrow_mut();
        let state = &mut *state;
        let Some(play) = state.help.welcome.as_mut() else { return };
        if play.size.0 < 1.0 || play.size.1 < 1.0 {
            return;
        }
        if let Some((built, pace)) = play.building.as_ref().and_then(|b| b.try_recv().ok()) {
            let most = (SHARE * screen_frame(&w) / pace.max(1e-6) * 1e6).clamp(LEAST * BUDGET, BUDGET);
            (play.stage, play.building, play.pace, play.most) = (Some(built), None, pace, most);
            crate::timing::line(&format!("welcome: the stage draws a million pixels in {:.1} ms, {:.1} of them a picture", pace * 1000.0, most / 1e6));
        }
        let now = play.now();
        play.follow(help.get_welcome_page(), now, still);
        play.advance(now);
        let cue = play.cue(now);
        let frame = direction::frame(&cue);
        // One picture in flight: the next waits until the GPU has finished the last, so none queues behind it.
        let _ = state.gpu.device.poll(wgpu::PollType::Poll);
        let free = !play.flying.load(Ordering::Acquire);
        // The picture at the window's own pixels, fewer past the budget or past what this GPU draws in a frame.
        // A screen of few pixels is drawn finer than it shows, as far as the budget goes: its edges come out smooth.
        let fit = (play.most / (play.size.0 * play.size.1 * scale * scale)).sqrt().min(FINER) * scale;
        let size = ((play.size.0 * fit).round().max(8.0) as u32, (play.size.1 * fit).round().max(8.0) as u32);
        if let Some(stage) = play.stage.as_mut().filter(|_| free) {
            let gpu = &state.gpu;
            let fresh = play.target.as_ref().is_none_or(|t| (t.width(), t.height()) != size);
            if fresh {
                play.target = Some(Stage::target(&gpu.device, size.0, size.1));
            }
            let Some(target) = &play.target else { return };
            stage.draw(&gpu.device, &gpu.queue, &frame, target);
            play.flying.store(true, Ordering::Release);
            let flying = play.flying.clone();
            gpu.queue.on_submitted_work_done(move || flying.store(false, Ordering::Release));
            if gpu.reads_back {
                if let Some(bytes) = crate::display::read_back(&gpu.device, &gpu.queue, target, size.0, size.1) {
                    help.set_stage(Image::from_rgba8(SharedPixelBuffer::clone_from_slice(&bytes, size.0, size.1)));
                }
            } else if fresh {
                match Image::try_from(target.clone()) {
                    Ok(image) => help.set_stage(image),
                    Err(e) => crate::timing::line(&format!("welcome: the stage's texture refused: {e}")),
                }
            }
            play.revision = play.revision.wrapping_add(1);
            help.set_stage_revision(play.revision);
        }
        let (kind, gift) = match cue.act {
            Act::Overture => (0, 0),
            Act::Hall => (1, 0),
            Act::Unwrap(i) => (2, i),
            Act::Page(i) => (3, i),
            Act::Message => (4, 0),
        };
        help.set_act(kind);
        help.set_act_gift(gift as i32);
        help.set_act_time(cue.t);
        help.set_film_step(cue.step as i32);
        help.set_film_time(cue.step_t);
        help.set_film_printed(cue.step == 4 && cue.step_t > film::PRINTED);
        help.set_film_ended(if cue.step == 4 { cue.step_t - film::FINALE } else { -1.0 });
        help.set_hovered(play.hovered.map_or(-1, |i| i as i32));
        help.set_film_show(frame.show[0]);
        help.set_film_compare(cue.compare);
        if let Act::Unwrap(i) = cue.act {
            help.set_act_cue(unwrap::title(i));
            help.set_act_length(unwrap::length(i));
        }
        if let Act::Page(i) = cue.act {
            let (x, y) = direction::mouth_spot(&frame, cue.aspect, i);
            help.set_mouth(StageSpot { x: x * play.size.0, y: y * play.size.1, width: 0.0, height: 0.0 });
        }
        if let Act::Unwrap(i) | Act::Page(i) = cue.act {
            let bit = 1 << i;
            if help.get_welcome_unwrapped() & bit == 0 {
                help.set_welcome_unwrapped(help.get_welcome_unwrapped() | bit);
            }
        }
        let spot = |s: [f32; 4]| StageSpot { x: s[0] * play.size.0, y: s[1] * play.size.1, width: s[2] * play.size.0, height: s[3] * play.size.1 };
        if kind <= 1 {
            let spots: Vec<StageSpot> = (0..4).map(|i| spot(direction::gift_spot(&frame, cue.aspect, i))).collect();
            help.set_spots(ModelRc::new(VecModel::from(spots)));
        }
        // The print's own place, once it has come forward: its captions and the hand's drag lie on it.
        help.set_plate_spot(direction::plate_spot(&frame, cue.aspect).filter(|_| cue.step == 4 && cue.step_t > film::PRINTED && cue.step_t < film::FINALE).map_or_else(StageSpot::default, spot));
    }
}
