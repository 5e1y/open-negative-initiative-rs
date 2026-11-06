//! A field's value and its place on its slider's track, on the fixed range or on a frame's own.

use crate::fields::{Field, Readout};
use crate::track;
use std::ops::RangeInclusive;

/// One unit of a readout's last shown digit, in the value's own terms.
fn unit(readout: Readout) -> f32 {
    match readout {
        Readout::Ceiling => 0.001,
        Readout::Pixels => 0.1,
        Readout::Stops | Readout::Degrees | Readout::Times | Readout::SignedTimes => 0.01,
        Readout::SignedPercent | Readout::Percent => 0.01,
    }
}

/// An arrow's step: `by` units of the readout's last digit, laid on that digit's grid so each press
/// shows the next number, held on the track.
pub fn stepped(f: Field, stored: f32, by: i32, track: &RangeInclusive<f32>) -> f32 {
    let unit = unit(f.spec().readout);
    let shown = ((f.shown(stored) / unit).round() + by as f32) * unit;
    f.shown(shown.clamp(*track.start(), *track.end()))
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

/// The steps from a readout to a number typed over it: `per` readout units a step, or one unit of
/// the readout's last digit where `per` is 0, an arrow's step; none when either holds no number.
pub fn typed_steps(readout: &str, typed: &str, per: f32) -> i32 {
    let (Some((shown, places)), Some((wanted, _))) = (read(readout), read(typed)) else { return 0 };
    let unit = if per > 0.0 { f64::from(per) } else { 10f64.powi(-(places as i32)) };
    ((wanted - shown) / unit).round() as i32
}

/// Answers `Typing.steps` on `window`, whose sliders otherwise take no typed number.
pub fn serve<C>(window: &C)
where
    C: slint::ComponentHandle,
    for<'a> crate::Typing<'a>: slint::Global<'a, C>,
{
    window.global::<crate::Typing>().on_steps(|readout, typed, per| typed_steps(&readout, &typed, per));
}

pub(crate) fn track_range(f: Field) -> RangeInclusive<f32> {
    let (low, high) = f.track();
    low..=high
}

/// The track a field spans on these settings: `Knob::range`, a monochrome frame's or a plain file's
/// own, in the shown sense.
pub fn track_on(f: Field, s: &oni_settings::PipelineSettings) -> RangeInclusive<f32> {
    let (low, high) = f.knob().range(s).unwrap_or(f.spec().range);
    if f.spec().inverted { -high..=-low } else { low..=high }
}

/// `position` and `stored_at` on a settings' own track.
pub fn position_on(f: Field, stored: f32, track: &RangeInclusive<f32>) -> f32 {
    track::position(f.shown(stored), track, f.spec().centre)
}

pub fn stored_on(f: Field, at: f32, track: &RangeInclusive<f32>) -> f32 {
    f.shown(track::value(at, track, f.spec().centre))
}

/// Where a stored value sits on its track.
pub(crate) fn position(f: Field, stored: f32) -> f32 {
    track::position(f.shown(stored), &track_range(f), f.spec().centre)
}

/// The stored value at a track position: the exact inverse of `position`.
pub(crate) fn stored_at(f: Field, at: f32) -> f32 {
    f.shown(track::value(at, &track_range(f), f.spec().centre))
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::{ConversionMode, PipelineSettings};

    #[test]
    fn an_arrow_moves_the_readout_by_one_and_lands_on_its_numbers() {
        let track = track_range(Field::Temperature);
        let top = *track.end();
        // +0.186 reads +0.19 EV: the next press reads +0.20 EV, never +0.21 EV.
        let at = stepped(Field::Temperature, 0.186, 1, &track);
        assert_eq!(Field::Temperature.readout(at), "Warm +0.20 EV");
        assert_eq!(Field::Temperature.readout(stepped(Field::Temperature, at, -10, &track)), "Warm +0.10 EV");
        // Twin: a step of 1 % of the range, as accessibility's was, skips three numbers in four.
        let span = top - *track.start();
        assert_eq!(Field::Temperature.readout(0.20 + span / 100.0), "Warm +0.24 EV");
        // Held at the track's end, and the black point keeps its shown sense: right lifts the black.
        assert_eq!(stepped(Field::Temperature, top, 5, &track), top);
        let black = track_range(Field::BlackPoint);
        let lifted = stepped(Field::BlackPoint, 0.0, 1, &black);
        assert!(lifted < 0.0 && Field::BlackPoint.readout(lifted) == "+1%", "{}", Field::BlackPoint.readout(lifted));
        assert_eq!(Field::Exposure.readout(stepped(Field::Exposure, 0.5, 1, &track_range(Field::Exposure))), "+0.51 EV");
    }

    #[test]
    fn a_number_typed_over_a_readout_is_the_steps_an_arrow_would_take() {
        // The readout's last digit is a step: +1.50 EV to 2 is fifty hundredths, to −0.3 one hundred and eighty.
        assert_eq!(typed_steps("+1.50 EV", "2", 0.0), 50);
        assert_eq!(typed_steps("+1.50 EV", "\u{2212}0.3", 0.0), -180);
        assert_eq!(typed_steps("+23", "-4", 0.0), -27);
        assert_eq!(typed_steps("0%", "15 %", 0.0), 15);
        assert_eq!(typed_steps("12.5 px", "12,8", 0.0), 3);
        assert_eq!(typed_steps("3 / 24", "7", 0.0), 4);
        // A stepped slider's steps are its own: +15% to 25% at 5% a step is two, half units on a letter.
        assert_eq!(typed_steps("+15%", "25", 5.0), 2);
        assert_eq!(typed_steps("+2", "\u{2212}1.5", 0.5), -7);
        // The same number, or none, moves nothing.
        assert_eq!(typed_steps("+1.50 EV", "1.5", 0.0), 0);
        assert_eq!(typed_steps("+1.50 EV", "warm", 0.0), 0);
        // The side a colour readout names is left aside, its sign read: Green −0.06 EV to 0 is six.
        assert_eq!((typed_steps("Warm +0.46 EV", "0.5", 0.0), typed_steps("Green \u{2212}0.06 EV", "0", 0.0)), (4, 6));
        // Twin: the side named without its sign would step the wrong way.
        assert_eq!(typed_steps("Green 0.06 EV", "0", 0.0), -6);
        assert_eq!(typed_steps("", "2", 0.0), 0);
        // Twin: the steps counted in whole units would leave the hundredths' readout fifty times short.
        assert_ne!(typed_steps("+1.50 EV", "2", 1.0), 50);
        // Typed then stepped, the value lands on the number typed.
        let track = track_range(Field::Exposure);
        let at = stepped(Field::Exposure, 1.5, typed_steps(&Field::Exposure.readout(1.5), "-0.37", 0.0), &track);
        assert_eq!(Field::Exposure.readout(at), "\u{2212}0.37 EV");
    }

    #[test]
    fn a_monochrome_frame_spans_its_own_contrast_and_a_value_round_trips() {
        let mono = PipelineSettings::rest(ConversionMode::Monochrome, true);
        let colour = PipelineSettings::rest(ConversionMode::Negative, true);
        // Twin: the colour track is another, wider one.
        assert_ne!(track_on(Field::Contrast, &mono), track_on(Field::Contrast, &colour));
        assert_eq!(track_on(Field::Contrast, &colour), track_range(Field::Contrast));
        let range = track_on(Field::Contrast, &mono);
        let at = position_on(Field::Contrast, 0.4, &range);
        assert!((stored_on(Field::Contrast, at, &range) - 0.4).abs() < 1e-5);
        // The black point keeps its shown sense on its own track.
        let black = track_on(Field::BlackPoint, &colour);
        assert!((stored_on(Field::BlackPoint, position_on(Field::BlackPoint, -0.1, &black), &black) + 0.1).abs() < 1e-5);
    }
}
