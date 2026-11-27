//! The welcome's screens: which gift each shows, the scroll's places, the gifts kept opened, and the state the clock
//! moves through them, one screen a gift, the message last.

use super::super::reels::Reel;
use super::super::{EDITION, KIT_GIFT};
use crate::stage::direction::{self, Cue};
use crate::stage::{Stage, glide, unwrap};
use slint::Timer;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Receiver;
use std::time::Instant;

/// How long one screen takes to give way to the next, in seconds.
const TRAVEL: f32 = 0.7;
/// The message's announcement holds alone this many seconds, then turns to what it says in `TURN`.
const ANNOUNCED: f32 = 2.2;
const TURN: f32 = 0.8;
/// The message's screen, after the four gifts'.
pub(super) const MESSAGE: i32 = 5;

/// The gift a screen shows, none on the message's.
pub(super) fn gift_of(page: i32) -> Option<usize> {
    (1..=4).contains(&page).then(|| page as usize - 1)
}

/// The screen `by` from `page`, the Collection's passed over without its gift.
fn step(page: i32, by: i32) -> i32 {
    let next = (page + by).clamp(1, MESSAGE);
    if next == 2 && !KIT_GIFT { (next + by).clamp(1, MESSAGE) } else { next }
}

/// A screen's place in the list of screens, one under the other, the Collection's taking none without its gift.
pub(super) fn slot(page: i32) -> f32 {
    (if !KIT_GIFT && page > 2 { page - 2 } else { page - 1 }) as f32
}

/// The gifts opened, as kept for this edition: one bit a gift.
pub fn progress(kept: Option<&str>) -> u8 {
    kept.and_then(|k| k.strip_prefix(EDITION)?.strip_prefix(':')?.parse::<u8>().ok()).unwrap_or(0) & 0b1111
}

/// The stage while the welcome shows.
pub(in crate::app) struct Playing {
    pub(super) stage: Option<Stage>,
    /// The pipeline being built off this thread, so the sheet opens at once; gone, the stage failed.
    pub(super) building: Option<Receiver<(Stage, f32)>>,
    /// The most pixels this GPU draws in a screen's frame.
    pub(super) most: f32,
    pub(super) target: Option<wgpu::Texture>,
    pub(super) revision: i32,
    pub(super) began: Instant,
    /// A check's clock, in seconds, in place of the time since the welcome opened.
    pub(super) held: Option<f32>,
    /// The screen shown, the one it came from, when it left it and where the list of screens stood then.
    pub(super) page: i32,
    pub(super) from: i32,
    pub(super) moved: f32,
    pub(super) origin: f32,
    /// When each gift was opened, on the welcome's clock; one opened on an earlier launch long before.
    pub(super) opened: [Option<f32>; 4],
    /// The sheet's size and its footer's band, in the window's points.
    pub(super) size: (f32, f32),
    pub(super) foot: f32,
    /// Where the film stands across the sheet, as a share of its width: the wrapped gift stands there.
    pub(super) middle: f32,
    /// When the message was asked past its announcement, on the welcome's clock.
    pub(super) told: Option<f32>,
    /// Whether the GPU is still drawing the last picture handed to it.
    pub(super) flying: Arc<AtomicBool>,
    /// The gifts' films, each decoded as it plays off this thread.
    pub(super) reels: Vec<Reel>,
    /// Kept so its frames keep coming; dropped with the stage.
    pub(super) _timer: Timer,
}

impl Playing {
    pub(super) fn now(&self) -> f32 {
        self.held.unwrap_or_else(|| self.began.elapsed().as_secs_f32())
    }

    pub(super) fn bits(&self) -> u8 {
        self.opened.iter().enumerate().fold(0, |bits, (i, o)| bits | (u8::from(o.is_some()) << i))
    }

    /// Opens the screen's gift if it is still wrapped; whether it did.
    pub(super) fn open(&mut self, now: f32, still: bool) -> bool {
        let Some(i) = gift_of(self.page).filter(|i| self.opened[*i].is_none()) else { return false };
        // With nothing moving it lies open at once.
        self.opened[i] = Some(if still { now - 1000.0 } else { now });
        true
    }

    /// Whether the person may go on past this screen: its gift opened and its words come.
    pub(super) fn may_go_on(&self, now: f32, still: bool) -> bool {
        gift_of(self.page).is_some_and(|i| self.opened[i].is_some_and(|o| still || now - o >= unwrap::title(i)))
    }

    /// Goes `by` screens: back at any time, on only past an opened gift. A press while the screens still glide sets
    /// off at once from where they stand.
    pub(super) fn go(&mut self, by: i32, now: f32, still: bool) {
        if by > 0 && !self.may_go_on(now, still) {
            return;
        }
        let next = step(self.page, by.signum());
        if next != self.page {
            let origin = self.scroll(now, still);
            (self.from, self.page, self.moved, self.origin, self.told) = (self.page, next, now, origin, None);
        }
    }

    /// Where the list of screens stands, in screens: gliding from where it stood to the screen shown.
    pub(super) fn scroll(&self, now: f32, still: bool) -> f32 {
        self.origin + (slot(self.page) - self.origin) * self.travelled(now, still)
    }

    /// The screen whose gift the stage draws: the one shown, or while the screens glide the one they leave if its gift
    /// is still wrapped and the one shown has none to show.
    pub(super) fn staged(&self, now: f32) -> i32 {
        let showing = |page: i32| gift_of(page).is_some_and(|i| direction::kept(i, self.opened[i].map(|o| now - o)) > 0.0);
        if !showing(self.page) && showing(self.from) { self.from } else { self.page }
    }

    /// How far the message has turned from its announcement to what it says: by itself once the announcement has
    /// held, at once when asked; 0 on any other screen.
    pub(super) fn turned(&self, now: f32, still: bool) -> f32 {
        if self.page != MESSAGE {
            return 0.0;
        }
        if still {
            return 1.0;
        }
        let by_itself = glide(((now - self.moved - TRAVEL - ANNOUNCED) / TURN).clamp(0.0, 1.0));
        by_itself.max(self.told.map_or(0.0, |t| glide(((now - t) / TURN).clamp(0.0, 1.0))))
    }

    /// How far the screens have travelled from the last to this one, 0 to 1.
    pub(super) fn travelled(&self, now: f32, still: bool) -> f32 {
        if still { 1.0 } else { glide(((now - self.moved) / TRAVEL).clamp(0.0, 1.0)) }
    }

    pub(super) fn cue(&self, now: f32) -> Cue {
        let (page, shown) = (self.staged(now), 1.0);
        let gift = gift_of(page);
        let since = gift.and_then(|i| self.opened[i]).map(|o| now - o);
        let aspect = self.size.0 / self.size.1.max(1.0);
        Cue { gift, since, clock: now, aspect, foot: self.foot / self.size.1.max(1.0), shown, middle: self.middle }
    }
}
