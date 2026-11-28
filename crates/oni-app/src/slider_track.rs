//! A field's value and its place on its slider's track, on the fixed range or on a frame's own.

use crate::fields::Field;
use crate::track;
use std::ops::{Deref, RangeInclusive};

/// The range a field's track spans, in the shown sense, and the levels it notches on, stored values lowest first.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub range: RangeInclusive<f32>,
    pub levels: Option<Vec<f32>>,
}

impl From<RangeInclusive<f32>> for Track {
    fn from(range: RangeInclusive<f32>) -> Track {
        Track { range, levels: None }
    }
}

impl Deref for Track {
    type Target = RangeInclusive<f32>;
    fn deref(&self) -> &RangeInclusive<f32> {
        &self.range
    }
}

impl Track {
    /// The notch nearest a stored value, and the notches' count.
    fn notch(&self, stored: f32) -> Option<(usize, usize)> {
        let levels = self.levels.as_deref().filter(|l| !l.is_empty())?;
        Some((oni_settings::effect_levels::nearest(levels, stored), levels.len()))
    }
}

/// An arrow's step, or a number typed over the readout: `by` units of its last digit from the figure shown,
/// laid on that digit's grid, held on the track.
pub fn stepped(f: Field, stored: f32, by: i32, track: &Track) -> f32 {
    stepped_on(f, stored, by, track, None)
}

/// `stepped` on a photo whose scan's long side is `long` full-resolution pixels, the digit its readout shows; on a
/// notched track, any step is one notch its way.
pub fn stepped_on(f: Field, stored: f32, by: i32, track: &Track, long: Option<f64>) -> f32 {
    if let (Some((at, n)), Some(levels)) = (track.notch(stored), track.levels.as_ref()) {
        return levels[(at as i32 + by.signum()).clamp(0, n as i32 - 1) as usize];
    }
    let (readout, figure) = f.figure(stored, long);
    // Counted in units of the digit the readout writes, rounded as it writes it: a tie to the even one.
    let per_unit = readout.scaled(1.0) * 10f64.powi(readout.places() as i32);
    let digit = (f64::from(figure) * per_unit).round_ties_even();
    let shown = f.unfigured(((digit + f64::from(by)) / per_unit) as f32, long);
    f.shown(shown.clamp(*track.start(), *track.end()))
}

/// A number typed over the readout, `by` units of its last digit: on a notched track, the notch nearest it.
pub fn typed_on(f: Field, stored: f32, by: i32, track: &Track, long: Option<f64>) -> f32 {
    let typed = stepped_on(f, stored, by, &Track::from(track.range.clone()), long);
    track.levels.as_deref().map_or(typed, |l| oni_settings::effect_levels::snapped(l, typed))
}

/// A number as a readout or a hand writes it, and its places after the point: `+`, `-` or U+2212
/// before it, a comma read as the point, the side a readout names before it and its unit left aside.
pub fn read(text: &str) -> Option<(f64, u32)> {
    let text = text.trim_start_matches(|c: char| c.is_alphabetic() || c.is_whitespace());
    let (negative, rest) = match text.chars().next() {
        Some('-' | '\u{2212}') => (true, &text[text.chars().next().map_or(0, char::len_utf8)..]),
        Some('+') => (false, &text[1..]),
        _ => (false, text),
    };
    let (mut digits, mut places, mut point) = (String::new(), 0, false);
    for c in rest.trim_start().chars() {
        match c {
            '0'..='9' => {
                digits.push(c);
                places += u32::from(point);
            }
            '.' | ',' if !point => {
                digits.push('.');
                point = true;
            }
            _ => break,
        }
    }
    let value: f64 = digits.parse().ok()?;
    Some((if negative { -value } else { value }, places))
}

/// The figure a readout shows, its sign and places as shown, the side it names and its unit left aside:
/// what a field typed over it starts from.
pub fn number(readout: &str) -> &str {
    let text = readout.trim_start_matches(|c: char| c.is_alphabetic() || c.is_whitespace());
    let end = text
        .char_indices()
        .find(|&(i, c)| !(c.is_ascii_digit() || matches!(c, '.' | ',') || (i == 0 && matches!(c, '+' | '-' | '\u{2212}'))))
        .map_or(text.len(), |(i, _)| i);
    &text[..end]
}

/// The steps from a readout to a number typed over it, one unit of the readout's last digit a step, an
/// arrow's; none when either holds no number.
pub fn typed_steps(readout: &str, typed: &str) -> i32 {
    let (Some((shown, places)), Some((wanted, _))) = (read(readout), read(typed)) else { return 0 };
    ((wanted - shown) / 10f64.powi(-(places as i32))).round() as i32
}

/// `steps` units of a readout's last digit, unsigned, as a slider key's tooltip names its press; empty
/// where it holds no number.
pub fn pressed(readout: &str, steps: i32) -> String {
    // The last figure and the unit after it: a span's higher end, a side's name left before it.
    let Some(end) = readout.rfind(|c: char| c.is_ascii_digit()) else { return String::new() };
    let start = readout[..end].rfind(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | ','))).map_or(0, |i| i + readout[i..].chars().next().map_or(1, char::len_utf8));
    let figure = &readout[start..=end];
    let places = figure.find(['.', ',']).map_or(0, |i| figure.len() - i - 1);
    let unit = &readout[end + 1..];
    let amount = f64::from(steps.unsigned_abs()) * 10f64.powi(-(places as i32));
    format!("{amount:.places$}{unit}")
}

/// Answers `Readouts` on `window`, whose sliders otherwise take no typed number.
pub fn serve<C>(window: &C)
where
    C: slint::ComponentHandle,
    for<'a> crate::Readouts<'a>: slint::Global<'a, C>,
{
    window.global::<crate::Readouts>().on_steps(|readout, typed| typed_steps(&readout, &typed));
    window.global::<crate::Readouts>().on_number(|readout| number(&readout).into());
    window.global::<crate::Readouts>().on_pressed(|readout, steps| pressed(&readout, steps).into());
}

pub(crate) fn track_range(f: Field) -> Track {
    let (low, high) = f.track();
    Track::from(low..=high)
}

/// The track a field spans on these settings: `Knob::range`, a monochrome frame's or a plain file's
/// own, in the shown sense, and the levels it notches on, evenly spaced along it.
pub fn track_on(f: Field, s: &oni_settings::PipelineSettings) -> Track {
    let (low, high) = f.travel(s);
    Track { range: if f.spec().inverted { -high..=-low } else { low..=high }, levels: f.levels(s) }
}

/// `position` and `stored_at` on a settings' own track.
pub fn position_on(f: Field, stored: f32, track: &Track) -> f32 {
    match track.notch(stored) {
        Some((at, n)) => if n > 1 { at as f32 / (n - 1) as f32 } else { 0.0 },
        None => track::position(f.shown(stored), track, f.spec().bend),
    }
}

pub fn stored_on(f: Field, at: f32, track: &Track) -> f32 {
    match track.levels.as_deref().filter(|l| !l.is_empty()) {
        Some(levels) => levels[(track::clamped(at) * (levels.len() - 1) as f32).round() as usize],
        None => f.shown(track::value(at, track, f.spec().bend)),
    }
}

/// How many notches each field's track has on these settings, by its index; none on an even track.
pub fn notches(s: &oni_settings::PipelineSettings) -> Vec<i32> {
    Field::ALL.iter().map(|f| f.levels(s).map_or(0, |l| l.len() as i32)).collect()
}

/// The notch a stored value lies on, on these settings; none on an even track.
pub fn notch_of(f: Field, s: &oni_settings::PipelineSettings, stored: f32) -> Option<usize> {
    track_on(f, s).notch(stored).map(|(at, _)| at)
}

/// A field's readout on these settings: on a notched track, its number then its notch's name.
pub fn readout_named(f: Field, s: &oni_settings::PipelineSettings, stored: f32, readout: String) -> String {
    let track = track_on(f, s);
    match track.notch(stored).and_then(|(at, _)| f.level_name(s, at)) {
        Some(name) => format!("{readout} · {name}"),
        None => readout,
    }
}

/// Where each field rests on its track on these settings, by its index: its tick, which a double click
/// and its block's reset land on alike.
pub fn rests(s: &oni_settings::PipelineSettings) -> Vec<f32> {
    Field::ALL.iter().map(|f| position_on(*f, f.base(s), &track_on(*f, s))).collect()
}

/// The share of each field's track past what its knob reaches on these settings, by its index: 0 where it reaches its end.
pub fn beyond(s: &oni_settings::PipelineSettings) -> Vec<f32> {
    let past = |f: Field| f.knob().range(s).filter(|_| !f.spec().inverted).map_or(0.0, |(_, high)| 1.0 - position_on(f, high, &track_on(f, s)));
    Field::ALL.iter().map(|f| past(*f).max(0.0)).collect()
}

/// `stored_on`, but rest itself where a drag holds on rest's place, as the double click lands it.
pub fn landed(f: Field, at: f32, track: &Track, rest: f32) -> f32 {
    if (at - position_on(f, rest, track)).abs() < 1e-5 { rest } else { stored_on(f, at, track) }
}

/// Where a stored value sits on its track.
pub(crate) fn position(f: Field, stored: f32) -> f32 {
    track::position(f.shown(stored), &track_range(f), f.spec().bend)
}

/// The stored value at a track position: the exact inverse of `position`.
pub(crate) fn stored_at(f: Field, at: f32) -> f32 {
    f.shown(track::value(at, &track_range(f), f.spec().bend))
}

#[cfg(test)]
mod tests;
