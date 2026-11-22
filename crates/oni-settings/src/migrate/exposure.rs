//! A photo a sidecar before format 42 kept read Exposure once on the print's curve: its distance from its rest
//! halved, so its render stays where it was.

use super::chain::EXPOSURE_GEAR_VERSION;
use crate::PipelineSettings;
use crate::tone::Print;

/// The settings of an earlier format with Exposure in this format's units.
pub fn caught_up(version: i64, settings: PipelineSettings) -> PipelineSettings {
    if version >= EXPOSURE_GEAR_VERSION {
        return settings;
    }
    PipelineSettings { print: geared(settings.print, Print::resting_exposure(settings.mode)), ..settings }
}

/// An earlier format's print, resting at `rest`, in this format's units: what its curve took, undone of the gear.
pub fn geared(print: Print, rest: f32) -> Print {
    Print { exposure: Print::exposure_at(print.exposure, rest), ..print }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConversionMode;

    #[test]
    fn an_earlier_exposure_keeps_its_curve_and_a_rest_stays() {
        let older = EXPOSURE_GEAR_VERSION - 1;
        for mode in ConversionMode::ALL {
            let rest = PipelineSettings::rest(mode, true);
            assert_eq!(caught_up(older, rest.clone()), rest, "{mode:?}: a rest stays");
            let mut lit = rest.clone();
            lit.print.exposure += 0.8;
            let moved = caught_up(older, lit.clone());
            let r = Print::resting_exposure(mode);
            assert!((Print::curve_stops(moved.print.exposure, r) - lit.print.exposure).abs() < 1e-6, "{mode:?}");
            assert_eq!(PipelineSettings { print: Print { exposure: lit.print.exposure, ..moved.print }, ..moved.clone() }, lit, "nothing else moves");
            // Twin: a document of this format keeps its value, which a second pass would halve again.
            assert_eq!(caught_up(EXPOSURE_GEAR_VERSION, moved.clone()), moved);
            assert!((caught_up(older, moved.clone()).print.exposure - moved.print.exposure).abs() > 0.1);
        }
    }
}
