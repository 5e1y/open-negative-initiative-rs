//! The welcome in the window: one screen a gift, each opened before the next, the message last. Its clock moves
//! the screens and the gift the GPU draws into the texture the sheet shows; a check holds the clock and steps it.

use super::reels::{self, Reel};
use super::{EDITION, KIT_GIFT, WELCOME_KEY};
use crate::app::{Ctl, SHELL};
use crate::stage::direction::{self, Cue};
use crate::stage::{Stage, glide, unwrap};
use crate::{Help, StageSpot};
use oni_i18n::tr;
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, Image, SharedPixelBuffer, Timer, TimerMode};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
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
/// How long one screen takes to give way to the next, in seconds.
const TRAVEL: f32 = 0.7;
/// Where the gifts opened on this computer are kept, so a welcome closed midway resumes where it was.
pub const PROGRESS_KEY: &str = "welcomeProgress";
/// The message's screen, after the four gifts'.
const MESSAGE: i32 = 5;

/// The screen's frame, in seconds: a 60 Hz one where the window's screen tells no rate.
fn screen_frame(w: &crate::MainWindow) -> f32 {
    let rate = w.window().with_winit_window(|window| window.current_monitor().and_then(|m| m.refresh_rate_millihertz())).flatten().unwrap_or(60_000);
    1000.0 / (rate as f32).clamp(30_000.0, 240_000.0)
}

/// The gift a screen shows, none on the message's.
fn gift_of(page: i32) -> Option<usize> {
    (1..=4).contains(&page).then(|| page as usize - 1)
}

/// The screen `by` from `page`, the Collection's passed over without its gift.
fn step(page: i32, by: i32) -> i32 {
    let next = (page + by).clamp(1, MESSAGE);
    if next == 2 && !KIT_GIFT { (next + by).clamp(1, MESSAGE) } else { next }
}

/// A screen's place in the scroll, the Collection's taking none without its gift; the message comes over the last
/// gift's, out of the scroll.
fn slot(page: i32) -> f32 {
    let page = page.min(MESSAGE - 1);
    (if !KIT_GIFT && page > 2 { page - 2 } else { page - 1 }) as f32
}

/// The gifts opened, as kept for this edition: one bit a gift.
pub fn progress(kept: Option<&str>) -> u8 {
    kept.and_then(|k| k.strip_prefix(EDITION)?.strip_prefix(':')?.parse::<u8>().ok()).unwrap_or(0) & 0b1111
}

/// The stage while the welcome shows.
pub(in crate::app) struct Playing {
    stage: Option<Stage>,
    /// The pipeline being built off this thread, so the sheet opens at once; gone, the stage failed.
    building: Option<Receiver<(Stage, f32)>>,
    /// The most pixels this GPU draws in a screen's frame.
    most: f32,
    target: Option<wgpu::Texture>,
    revision: i32,
    began: Instant,
    /// A check's clock, in seconds, in place of the time since the welcome opened.
    held: Option<f32>,
    /// The screen shown, the one it came from and when it left it.
    page: i32,
    from: i32,
    moved: f32,
    /// When each gift was opened, on the welcome's clock; one opened on an earlier launch long before.
    opened: [Option<f32>; 4],
    /// The sheet's size and its footer's band, in the window's points.
    size: (f32, f32),
    foot: f32,
    /// Whether the GPU is still drawing the last picture handed to it.
    flying: Arc<AtomicBool>,
    /// The gifts' films, each decoded as it plays off this thread.
    reels: Vec<Reel>,
    /// Kept so its frames keep coming; dropped with the stage.
    _timer: Timer,
}

impl Playing {
    fn now(&self) -> f32 {
        self.held.unwrap_or_else(|| self.began.elapsed().as_secs_f32())
    }

    fn bits(&self) -> u8 {
        self.opened.iter().enumerate().fold(0, |bits, (i, o)| bits | (u8::from(o.is_some()) << i))
    }

    /// Opens the screen's gift if it is still wrapped; whether it did.
    fn open(&mut self, now: f32, still: bool) -> bool {
        let Some(i) = gift_of(self.page).filter(|i| self.opened[*i].is_none()) else { return false };
        // With nothing moving it lies open at once.
        self.opened[i] = Some(if still { now - 1000.0 } else { now });
        true
    }

    /// Whether the person may go on past this screen: its gift opened and its words come.
    fn may_go_on(&self, now: f32, still: bool) -> bool {
        gift_of(self.page).is_some_and(|i| self.opened[i].is_some_and(|o| still || now - o >= unwrap::title(i)))
    }

    /// Goes `by` screens: back at any time, on only past an opened gift; never while a screen still travels.
    fn go(&mut self, by: i32, now: f32, still: bool) {
        if !still && now - self.moved < TRAVEL || by > 0 && !self.may_go_on(now, still) {
            return;
        }
        let next = step(self.page, by.signum());
        if next != self.page {
            (self.from, self.page, self.moved) = (self.page, next, now);
        }
    }

    /// How far the screens have travelled from the last to this one, 0 to 1.
    fn travelled(&self, now: f32, still: bool) -> f32 {
        if still { 1.0 } else { glide(((now - self.moved) / TRAVEL).clamp(0.0, 1.0)) }
    }

    fn cue(&self, now: f32, still: bool) -> Cue {
        let p = self.travelled(now, still);
        // The stage goes dark as one screen leaves and lights the next as it settles.
        let (page, shown) = if p < 0.5 { (self.from, 1.0 - 2.0 * p) } else { (self.page, 2.0 * p - 1.0) };
        let gift = gift_of(page);
        let since = gift.and_then(|i| self.opened[i]).map(|o| now - o);
        let aspect = self.size.0 / self.size.1.max(1.0);
        Cue { gift, since, clock: now, aspect, foot: self.foot / self.size.1.max(1.0), shown }
    }
}

impl Ctl {
    pub(super) fn wire_welcome(me: &std::rc::Rc<Ctl>, help: &Help) {
        let p = me.clone();
        help.on_welcome_sized(move |width, height, foot| p.welcome_set(|play| (play.size, play.foot) = ((width, height), foot)));
        let p = me.clone();
        help.on_welcome_open(move || p.welcome_open());
        let p = me.clone();
        help.on_welcome_go(move |by| {
            let still = p.window().is_some_and(|w| w.global::<crate::Motion>().get_reduced());
            p.welcome_set(|play| {
                let now = play.now();
                play.go(by, now, still);
            });
            p.welcome_frame();
        });
    }

    fn welcome_set(&self, set: impl FnOnce(&mut Playing)) {
        if let Some(play) = self.state.borrow_mut().help.welcome.as_mut() {
            set(play);
        }
    }

    /// The screen's gift opened, and kept opened for a launch after this one.
    fn welcome_open(&self) {
        let still = self.window().is_some_and(|w| w.global::<crate::Motion>().get_reduced());
        let kept = {
            let mut s = self.state.borrow_mut();
            let opened = s.help.welcome.as_mut().and_then(|play| {
                let now = play.now();
                play.open(now, still).then(|| play.bits())
            });
            opened.map(|bits| s.preferences.set(PROGRESS_KEY, format!("{EDITION}:{bits}").into()))
        };
        if let Some(Err(e)) = kept {
            crate::timing::line(&format!("welcome: the opened gifts cannot be kept: {e}"));
        }
        self.welcome_frame();
    }

    /// The Wrapped opened from the mystery gift closed: the welcome goes on to its message, the last thing before the
    /// app.
    pub(in crate::app) fn welcome_after_wrapped(&self) {
        let still = self.window().is_some_and(|w| w.global::<crate::Motion>().get_reduced());
        self.welcome_set(|play| {
            let now = play.now();
            if play.page == MESSAGE - 1 {
                play.go(1, now, still);
            }
        });
        self.welcome_frame();
    }

    /// `WelcomeSheet.isDue`: this release's sheet not yet shown on this computer.
    pub fn welcome_due(&self) -> bool {
        super::welcome_due(self.state.borrow().preferences.string(WELCOME_KEY))
    }


    /// The welcome on its first gift still wrapped, the gifts opened on an earlier launch left open; whether a
    /// window shows it.
    pub(in crate::app) fn open_welcome(&self) -> bool {
        let Some(w) = self.window() else { return false };
        let h = w.global::<Help>();
        h.set_film_count(oni_settings::VirtualStock::sheets().len() as i32);
        let bits = progress(self.state.borrow().preferences.string(PROGRESS_KEY));
        let opened: [Option<f32>; 4] = std::array::from_fn(|i| (bits & (1 << i) != 0 || i == 1 && !KIT_GIFT).then_some(-1000.0));
        let page = (1..MESSAGE).find(|p| gift_of(*p).is_some_and(|i| opened[i].is_none())).unwrap_or(MESSAGE);
        let (sender, building) = channel();
        {
            let mut s = self.state.borrow_mut();
            let (device, queue) = (s.gpu.device.clone(), s.gpu.queue.clone());
            // Its shader compiles off this thread: the sheet is there at once, each gift a still until it lands;
            // a stage that fails on this GPU drops its end, and the stills stay.
            let _ = std::thread::Builder::new().name("stage".into()).spawn(move || {
                let mut stage = Stage::new(&device);
                let pace = stage.pace(&device, &queue);
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
                most: BUDGET,
                target: None,
                revision: 0,
                began: Instant::now(),
                held: None,
                page,
                from: page,
                moved: -10.0,
                opened,
                size: (0.0, 0.0),
                foot: 0.0,
                flying: Arc::new(AtomicBool::new(false)),
                reels: reels::reels(),
                _timer: timer,
            });
        }
        h.set_welcome_unwrapped(i32::from(bits));
        h.set_welcome_page(page);
        h.set_scroll(slot(page));
        h.set_stage(Image::default());
        h.set_stage_live(false);
        h.set_welcome_shown(true);
        true
    }

    /// From its last key alone: not shown again on the next launch; its stage and its clock go with it.
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

    /// A check's clock: the welcome at `seconds` since it opened, drawn; none gives the clock back to time.
    pub fn welcome_hold(&self, seconds: Option<f32>) {
        self.welcome_set(|play| play.held = seconds);
        self.welcome_frame();
    }

    /// One picture: the screens where the clock has them, the gift drawn and told to the sheet.
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
        match play.building.as_ref().map(Receiver::try_recv) {
            Some(Ok((built, pace))) => {
                play.most = (SHARE * screen_frame(&w) / pace.max(1e-6) * 1e6).clamp(LEAST * BUDGET, BUDGET);
                (play.stage, play.building) = (Some(built), None);
                crate::timing::line(&format!("welcome: the stage draws a million pixels in {:.1} ms, {:.1} of them a picture", pace * 1000.0, play.most / 1e6));
            }
            Some(Err(TryRecvError::Disconnected)) => {
                play.building = None;
                crate::timing::line("welcome: the stage cannot be drawn on this GPU; its gifts are shown as stills");
            }
            _ => {}
        }
        let now = play.now();
        let cue = play.cue(now, still);
        let frame = direction::frame(&cue);
        // One picture in flight: the next waits until the GPU has finished the last, so none queues behind it.
        let _ = state.gpu.device.poll(wgpu::PollType::Poll);
        let free = !play.flying.load(Ordering::Acquire);
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
        help.set_stage_live(play.stage.is_some());
        help.set_welcome_page(play.page);
        help.set_welcome_unwrapped(i32::from(play.bits()));
        help.set_scroll(slot(play.from) + (slot(play.page) - slot(play.from)) * play.travelled(now, still));
        help.set_stage_shown(cue.shown * cue.gift.map_or(0.0, |i| direction::kept(i, cue.since)));
        let travelled = play.travelled(now, still);
        help.set_message_shown(if play.page == MESSAGE { travelled } else if play.from == MESSAGE { 1.0 - travelled } else { 0.0 });
        help.set_gift_shown(cue.gift.map_or(-1, |i| i as i32));
        let since = gift_of(play.page).and_then(|i| play.opened[i]).map_or(-1.0, |o| (now - o).max(0.0));
        help.set_opened_time(since);
        help.set_words_at(gift_of(play.page).map_or(0.0, |i| if still { 0.0 } else { unwrap::title(i) }));
        help.set_may_go_on(play.may_go_on(now, still));
        // The screen's film, from the moment its words come.
        let reel = gift_of(play.page).and_then(|i| play.reels.iter_mut().find(|r| r.gift == i).map(|r| (r, i)));
        let showing = help.get_reel_gift();
        help.set_reel_gift(reel.as_ref().map_or(-1, |(_, i)| *i as i32));
        if let Some((reel, i)) = reel {
            // Another film's frame is in the picture: this one's is given again.
            if showing != i as i32 {
                reel.again();
            }
            let words = if still { 0.0 } else { unwrap::title(i) };
            if let Some(frame) = reel.at(since - words - 0.4, still) {
                help.set_reel(frame);
            }
        }
        let spot = direction::gift_spot(&frame, cue.aspect).unwrap_or_default();
        help.set_gift_spot(StageSpot { x: spot[0] * play.size.0, y: spot[1] * play.size.1, width: spot[2] * play.size.0, height: spot[3] * play.size.1 });
    }
}
