//! The year on film, read off what this machine already keeps and never sent: the journal's gestures
//! (`darkroom`), the library's frames and their sidecars (`frames`), the thumbnails' colours (`palette`),
//! and the character they add up to (`persona`).

pub mod darkroom;
pub mod frames;
pub mod palette;
pub mod persona;

pub use darkroom::{Darkroom, GOLD, SESSION_BREAK};
pub use frames::{Frame, Frames};
pub use palette::Palette;
pub use persona::{Persona, Trait};

use crate::journal::epoch;
use chrono::{Datelike, Timelike};

/// A moment on this machine's calendar: the year, the month (1 to 12), the day of the month, the day of
/// the year (1 to 366) and the hour (0 to 23).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Moment {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub ordinal: u32,
    pub hour: u32,
}

impl Moment {
    fn of<T: chrono::TimeZone>(unix: f64, zone: &T) -> Moment {
        let Some(at) = chrono::DateTime::from_timestamp(unix.floor() as i64, 0) else { return Moment::default() };
        let at = at.with_timezone(zone);
        Moment { year: at.year(), month: at.month(), day: at.day(), ordinal: at.ordinal(), hour: at.hour() }
    }

    /// A date's moment on this machine's clock.
    pub fn local(unix: f64) -> Moment {
        Moment::of(unix, &chrono::Local)
    }

    /// The same moment read in UTC, as the checks read it.
    pub fn utc(unix: f64) -> Moment {
        Moment::of(unix, &chrono::Utc)
    }

    /// A journal record's time, seconds since the journal's epoch, as a Unix time.
    pub fn journal_unix(time: u32) -> f64 {
        epoch().unix_seconds() + f64::from(time)
    }
}

/// This machine's year and month now, which the December offer reads.
pub fn today() -> (i32, u32) {
    let now = chrono::Local::now();
    (now.year(), now.month())
}

/// The whole year: what the hands did, what the frames hold, their colours and the character.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Year {
    pub year: i32,
    pub darkroom: Darkroom,
    pub frames: Frames,
    pub palette: Palette,
    pub persona: Persona,
}

impl Year {
    pub fn new(year: i32, darkroom: Darkroom, frames: Frames, palette: Palette) -> Year {
        let persona = Persona::of(&darkroom, &frames);
        Year { year, darkroom, frames, palette, persona }
    }

    /// Nothing worth telling: no frame dated in the year and no gesture made in it.
    pub fn is_empty(&self) -> bool {
        self.frames.count == 0 && self.darkroom.photos == 0 && self.darkroom.gestures == 0
    }
}
