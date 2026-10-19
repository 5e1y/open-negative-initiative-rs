//! A field's value and its place on its slider's track, on the fixed range or on a frame's own.

use crate::fields::Field;
use crate::track;
use std::ops::RangeInclusive;

pub(crate) fn track_range(f: Field) -> RangeInclusive<f32> {
    let (low, high) = f.track();
    low..=high
}

/// The track a field spans on these settings: `Knob::range`, a monochrome frame's or a plain file's
/// own, in the shown sense.
pub(crate) fn track_on(f: Field, s: &oni_settings::PipelineSettings) -> RangeInclusive<f32> {
    let (low, high) = f.knob().range(s).unwrap_or(f.spec().range);
    if f.spec().inverted { -high..=-low } else { low..=high }
}

/// `position` and `stored_at` on a settings' own track.
pub(crate) fn position_on(f: Field, stored: f32, track: &RangeInclusive<f32>) -> f32 {
    track::position(f.shown(stored), track, f.spec().centre)
}

pub(crate) fn stored_on(f: Field, at: f32, track: &RangeInclusive<f32>) -> f32 {
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
