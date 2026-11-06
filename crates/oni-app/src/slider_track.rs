//! A field's value and its place on its slider's track, on the fixed range or on a frame's own.

use crate::fields::{Field, Readout};
use crate::track;
use std::ops::RangeInclusive;

/// One unit of a readout's last shown digit, in the value's own terms.
fn unit(readout: Readout) -> f32 {
    match readout {
        Readout::OfRange(top) => top / 100.0,
        Readout::Ceiling => 0.001,
        Readout::Pixels => 0.1,
        Readout::Stops | Readout::Signed | Readout::Degrees | Readout::Times | Readout::SignedTimes => 0.01,
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
        // +18.6 reads +19: the next press reads +20, never +21.
        let at = stepped(Field::Temperature, 0.186 * top, 1, &track);
        assert_eq!(Field::Temperature.readout(at), "+20");
        assert_eq!(Field::Temperature.readout(stepped(Field::Temperature, at, -10, &track)), "+10");
        // Twin: a step of 1 % of the range, as accessibility's was, skips every odd number.
        let span = top - *track.start();
        assert_eq!(Field::Temperature.readout(0.20 * top + span / 100.0), "+22");
        // Held at the track's end, and the black point keeps its shown sense: right lifts the black.
        assert_eq!(stepped(Field::Temperature, top, 5, &track), top);
        let black = track_range(Field::BlackPoint);
        let lifted = stepped(Field::BlackPoint, 0.0, 1, &black);
        assert!(lifted < 0.0 && Field::BlackPoint.readout(lifted) == "+1%", "{}", Field::BlackPoint.readout(lifted));
        assert_eq!(Field::Exposure.readout(stepped(Field::Exposure, 0.5, 1, &track_range(Field::Exposure))), "+0.51 EV");
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
