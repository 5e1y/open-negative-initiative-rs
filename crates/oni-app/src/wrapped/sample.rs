//! `ONI_WRAPPED_SAMPLE`: a full year made of the library's own photos, so the summary can be looked at
//! before one is lived. The photos, their films and their colours are real; the gestures are invented.

use super::gather;
use oni_library::wrapped::{Darkroom, Frame, Frames, Moment, Palette, Year};
use oni_settings::Medal;
use std::path::Path;

/// Every photo dated across `year`, one in seven given gold, under a year of invented gestures.
pub fn year(year: i32, mut frames: Vec<Frame>, support: &Path) -> Year {
    let start = oni_settings::Date::parse_iso8601(&format!("{year}-01-02T12:00:00Z")).map_or(0.0, oni_settings::Date::unix_seconds);
    let count = frames.len().max(1) as f64;
    for (i, frame) in frames.iter_mut().enumerate() {
        // The year fills faster toward its end, so the months differ.
        frame.unix = start + ((i as f64 / count).powf(0.7) * 340.0).floor() * 86_400.0;
        if i % 7 == 3 {
            frame.medal = Medal::Gold;
        }
    }
    let frames = Frames::build(&frames, year, Moment::local);
    let palette = Palette::of(&gather::tones(support, &frames.paths), super::SWATCHES);
    let mut by_hour = [0; 24];
    for (hour, count) in [(9, 40), (10, 60), (14, 80), (15, 70), (20, 160), (21, 320), (22, 410), (23, 380), (0, 150), (1, 40)] {
        by_hour[hour] = count;
    }
    let darkroom = Darkroom {
        photos: frames.count.max(412),
        gestures: 9_840,
        most_edited: frames.paths.first().map(|(path, _)| (path.clone(), 412)),
        favourite_rows: vec![(0, 1_180), (1, 920), (2, 460)],
        favourite_preset: Some(("Portra warm".into(), 212)),
        balanced: 318,
        pasted: 640,
        exported: 1_284,
        rolls: 43,
        measured: 1_512,
        hours: 162.0,
        longest_sitting: 3.5,
        by_hour,
        days: 96,
        streak: 9,
        busiest_day: Some((3, 14, 420)),
    };
    Year::new(year, darkroom, frames, palette)
}
