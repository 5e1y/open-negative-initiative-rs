//! The notches a digital photo's hand may ask next, whose preview scans the render thread makes while it waits.

use crate::fields::Field;
use oni_settings::PipelineSettings;
use oni_settings::effect_levels::nearest;

/// The fields whose levels a digital photo's scan holds.
const SCANNED: [Field; 6] = [Field::FilmExposure, Field::GrainAmount, Field::GrainSize, Field::HalationAmount, Field::HalationSize, Field::Bloom];

/// The scanned field that moved from `before` to `now`, and whether it rose.
pub fn touched(before: &PipelineSettings, now: &PipelineSettings) -> Option<(Field, bool)> {
    SCANNED.into_iter().find(|f| f.knob().get(before) != f.knob().get(now)).map(|f| (f, f.knob().get(now) > f.knob().get(before)))
}

/// `now` one notch either side on the field the hand last moved, the way it moved first, then on every other scanned
/// field, so the next notch of any lands at once: none on a photo on no virtual film.
pub fn neighbours(now: &PipelineSettings, touched: Option<(Field, bool)>) -> Vec<PipelineSettings> {
    let mut fields = vec![];
    if let Some((f, rising)) = touched {
        fields.push((f, rising));
    }
    for f in SCANNED {
        if !fields.iter().any(|(g, _)| *g == f) {
            fields.push((f, false));
        }
    }
    let mut out = vec![];
    for (f, rising) in fields {
        let Some(levels) = f.levels(now) else { continue };
        let at = nearest(&levels, f.knob().get(now)) as i64;
        let sides = if rising { [1, -1] } else { [-1, 1] };
        for side in sides {
            if let Some(v) = usize::try_from(at + side).ok().and_then(|i| levels.get(i)) {
                out.push(f.knob().set(now, *v));
            }
        }
    }
    out.truncate(super::engine::AHEAD);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::{ConversionMode, VirtualStock};

    #[test]
    fn the_notches_ahead_are_the_hands_way_first_then_every_scanned_fields() {
        let s = PipelineSettings::rest(ConversionMode::Digital, true).picking(VirtualStock::of("kodak-portra-800"));
        let halation = Field::HalationAmount;
        let lowered = halation.knob().set(&s, 0.10);
        let moved = touched(&s, &lowered);
        assert_eq!(moved, Some((halation, false)));
        let ahead = neighbours(&lowered, moved);
        let read = |f: Field| ahead.iter().map(|n| f.knob().get(n)).collect::<Vec<_>>();
        // Down first, then up, then shooting exposure a third of a stop either side, then every other field's.
        assert_eq!(ahead.len(), super::super::engine::AHEAD);
        assert_eq!(read(halation)[..2], [0.0, 0.31]);
        assert!((read(Field::FilmExposure)[2] + 1.0 / 3.0).abs() < 1e-6 && (read(Field::FilmExposure)[3] - 1.0 / 3.0).abs() < 1e-6);
        // Each scanned field has its notches either side made: none is left to the hand's next press.
        for f in SCANNED {
            let levels = f.levels(&lowered).expect("a film's levels");
            let at = nearest(&levels, f.knob().get(&lowered));
            let beside: Vec<f32> = [at.wrapping_sub(1), at + 1].into_iter().filter_map(|i| levels.get(i).copied()).collect();
            for v in beside {
                assert!(ahead.iter().any(|n| f.knob().get(n) == v), "{f:?}'s notch {v} not made ahead");
            }
        }
        // Rising, the way up comes first.
        let raised = halation.knob().set(&s, 0.92);
        assert_eq!(halation.knob().get(&neighbours(&raised, touched(&s, &raised))[0]), 0.31);
        // Twin: a scanned negative has no notch ahead.
        let scan = PipelineSettings::rest(ConversionMode::Negative, true);
        assert!(neighbours(&scan, None).is_empty());
        assert_eq!(touched(&s, &s), None);
    }
}
