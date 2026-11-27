//! The welcome in the window: one screen a gift, each opened before the next, the message last. Its clock moves
//! the screens and the gift the GPU draws into the texture the sheet shows; a check holds the clock and steps it.

mod picture;
mod screens;

use super::reels;
use super::{EDITION, KIT_GIFT, WELCOME_KEY};
use crate::Help;
use crate::app::{Ctl, SHELL};
use crate::stage::Stage;
use oni_i18n::tr;
use picture::BUDGET;
use screens::{MESSAGE, gift_of, slot};
use slint::{ComponentHandle, Image, Timer, TimerMode};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

pub(in crate::app) use screens::Playing;
pub use screens::progress;

/// A picture every 8 ms at the most: the pace of a 120 Hz screen.
const PACE: Duration = Duration::from_millis(8);
/// Where the gifts opened on this computer are kept, so a welcome closed midway resumes where it was.
pub const PROGRESS_KEY: &str = "welcomeProgress";

impl Ctl {
    pub(super) fn wire_welcome(me: &std::rc::Rc<Ctl>, help: &Help) {
        let p = me.clone();
        help.on_welcome_sized(move |width, height, foot, middle| p.welcome_set(|play| (play.size, play.foot, play.middle) = ((width, height), foot, middle)));
        let p = me.clone();
        help.on_overture_skip(move || {
            p.welcome_set(|play| play.overture = false);
            p.welcome_frame();
        });
        let p = me.clone();
        help.on_message_on(move || {
            p.welcome_set(|play| play.told = play.told.or(Some(play.now())));
            p.welcome_frame();
        });
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

    /// This release's sheet not yet shown on this computer.
    pub fn welcome_due(&self) -> bool {
        super::welcome_due(self.state.borrow().preferences.string(WELCOME_KEY))
    }

    /// The welcome asked for from the menu: again from its first gift, every gift wrapped, however often it is asked.
    pub(in crate::app) fn replay_welcome(&self) -> bool {
        if let Err(e) = self.state.borrow_mut().preferences.set(PROGRESS_KEY, serde_json::Value::Null) {
            crate::timing::line(&format!("welcome: the opened gifts cannot be wrapped again: {e}"));
        }
        self.open_welcome()
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
            let door = crate::render::app_gpu().map(|(_, _, door)| door);
            let _ = std::thread::Builder::new().name("stage".into()).spawn(move || {
                let mut stage = Stage::new(&device);
                let pace = stage.pace(&device, &queue, door.as_deref());
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
                origin: slot(page),
                opened,
                size: (0.0, 0.0),
                foot: 0.0,
                middle: 0.5,
                told: None,
                overture: !w.global::<crate::Motion>().get_reduced(),
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
        // Seen to its end: the gifts opened are kept only while a first pass is left halfway, so the next showing
        // begins on the first gift, wrapped.
        let written = {
            let mut s = self.state.borrow_mut();
            s.help.welcome = None;
            s.preferences.set(WELCOME_KEY, EDITION.into()).and_then(|()| s.preferences.set(PROGRESS_KEY, serde_json::Value::Null))
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

}
