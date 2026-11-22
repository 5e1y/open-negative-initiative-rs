//! `PipelineSettings.truncated(before:)`: the frame as it enters a stage, everything downstream at rest, the one
//! recipe every reading is taken through.

use oni_settings::axis::graduation;
use oni_settings::blocks::{ChromaDenoise, Grain, Halation, Texture, Vignette};
use oni_settings::curves::{CurveSet, SaturationCurve};
use oni_settings::levels::LevelsSet;
use oni_settings::spectrogram::Spectrogram;
use oni_settings::PipelineSettings;

/// The stages a reading can be taken in front of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Levels,
    GlobalLevels,
    ManualCurves,
    Curves,
}

pub fn truncated(s: &PipelineSettings, before: Stage) -> PipelineSettings {
    let mut out = s.clone();
    // The flat-field stays: a balance reads the frame the rig's light was evened on.
    out.vignette = Vignette::default();
    // Chroma denoise runs downstream of the levels, so a measurement of what enters them carries none.
    out.chroma = ChromaDenoise::OFF;
    out.spectrogram = Spectrogram::default();
    // A measure renders neither film effect, so a notch on one reads nothing again.
    out.grain = Grain::default();
    out.halation = Halation::default();
    out.saturation = SaturationCurve::default();
    out.secret_sauce = 0.0;
    out.texture = Texture::default();
    // The amount alone: the sharpening's rest carries a real dose.
    out.sharpen.amount = 0.0;
    match before {
        Stage::Levels | Stage::GlobalLevels => {
            // In front of the print a reading is of the film through ONI's windows, whatever legacy print lays its look.
            out.legacy_print = None;
            out.print.bypassed = true;
            out.density = 0.0;
            out.manual_curves = CurveSet::default();
        }
        Stage::ManualCurves => out.manual_curves = CurveSet::default(),
        Stage::Curves => {}
    }
    if before == Stage::Levels {
        // Downstream of the windows: a gain written before the matrix lands mixed.
        out.separation = 0.0;
        // What enters the windows is measured through the graduation, never the dose.
        out.chroma_decompression = 1.0;
        out.roll_reference = None;
        let graduated = graduation::levels(s.mode);
        out.levels = LevelsSet { red: graduated, green: graduated, blue: graduated };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::cg::Point;
    use oni_settings::{ConversionMode, LegacyPrint};

    #[test]
    fn before_the_levels_the_windows_carry_the_graduation_and_the_print_is_bypassed() {
        let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
        s.separation = 0.6;
        s.density = 0.3;
        s.legacy_print = Some(LegacyPrint { d: 2, ..LegacyPrint::default() });
        let t = truncated(&s, Stage::Levels);
        assert_eq!(t.levels.red, graduation::levels(ConversionMode::Negative));
        assert!(t.print.bypassed && t.separation == 0.0 && t.chroma_decompression == 1.0 && t.legacy_print.is_none());
        assert_eq!(truncated(&s, Stage::GlobalLevels).legacy_print, None, "ONI's windows in front of the print");
        // Twin: before the curves, the windows, the separation and the legacy print are the frame's own.
        let late = truncated(&s, Stage::Curves);
        assert_eq!((late.levels, late.separation, late.print.bypassed), (s.levels, 0.6, false));
        assert_eq!(late.legacy_print, s.legacy_print);
    }

    #[test]
    fn in_front_of_the_curves_a_digital_photos_lighting_stays_as_a_negatives_does() {
        let mut s = PipelineSettings::rest(ConversionMode::Digital, true);
        s.print.exposure = 0.6;
        (s.print.channel_contrast, s.density) = ([0.3, 0.0, -0.2], 0.3);
        s.manual_curves.linked.points = vec![Point::new(0.0, 0.0), Point::new(0.3, 0.8), Point::new(1.0, 1.0)];
        let t = truncated(&s, Stage::ManualCurves);
        assert_eq!((t.print, t.density), (s.print, 0.3));
        assert!(t.manual_curves.is_neutral());
        // Twin: in front of the levels the print is bypassed, the windows on the graduation.
        let levels = truncated(&s, Stage::Levels);
        assert!(levels.print.bypassed && levels.levels.red == graduation::levels(ConversionMode::Digital));
    }

    #[test]
    fn every_reading_is_taken_without_the_film_effects() {
        let rest = PipelineSettings::rest(ConversionMode::Digital, true);
        let mut s = rest.clone();
        (s.grain.amount, s.grain.size, s.halation.amount, s.halation.size) = (0.6, 0.2, 0.4, 0.9);
        for stage in [Stage::Levels, Stage::GlobalLevels, Stage::ManualCurves, Stage::Curves] {
            assert_eq!(truncated(&s, stage), truncated(&rest, stage), "{stage:?}");
        }
        // Twin: a print notch reaches the reading past the curves.
        let mut lit = rest.clone();
        lit.print.exposure += 0.1;
        assert_ne!(truncated(&lit, Stage::Curves), truncated(&rest, Stage::Curves));
    }
}
