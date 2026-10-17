//! `PipelineSettings.truncated(before:)`: the frame as it enters a stage, everything downstream at
//! rest, the one recipe every reading is taken through.

use oni_settings::axis::graduation;
use oni_settings::blocks::{ChromaDenoise, Texture, Vignette};
use oni_settings::curves::{CurveSet, SaturationCurve};
use oni_settings::levels::LevelsSet;
use oni_settings::modes::Face;
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
    // A reading is of the film through Pro's windows, whichever edit renders.
    out.face = Face::Pro;
    // Chroma denoise runs downstream of the levels, so a measurement of what enters them carries none.
    out.chroma = ChromaDenoise::OFF;
    out.spectrogram = Spectrogram::default();
    out.saturation = SaturationCurve::default();
    out.secret_sauce = 0.0;
    out.texture = Texture::default();
    // The amount alone: the sharpening's rest carries a real dose.
    out.sharpen.amount = 0.0;
    match before {
        Stage::Levels => {
            // Downstream of the windows: a gain written before the matrix lands mixed.
            out.separation = 0.0;
            // What enters the windows is measured through the graduation, never the dose.
            out.chroma_decompression = 1.0;
            out.roll_reference = None;
            let graduated = graduation::levels(s.mode);
            out.levels = LevelsSet { red: graduated, green: graduated, blue: graduated };
            out.print.bypassed = true;
            out.density = 0.0;
            out.manual_curves = CurveSet::default();
        }
        Stage::GlobalLevels => {
            out.print.bypassed = true;
            out.density = 0.0;
            out.manual_curves = CurveSet::default();
        }
        Stage::ManualCurves => out.manual_curves = CurveSet::default(),
        Stage::Curves => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::ConversionMode;

    #[test]
    fn before_the_levels_the_windows_carry_the_graduation_and_the_print_is_bypassed() {
        let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
        s.separation = 0.6;
        s.density = 0.3;
        s.face = Face::Scanner;
        let t = truncated(&s, Stage::Levels);
        assert_eq!(t.levels.red, graduation::levels(ConversionMode::Negative));
        assert!(t.print.bypassed && t.separation == 0.0 && t.chroma_decompression == 1.0 && t.face == Face::Pro);
        // Twin: before the curves, the windows and the separation are the frame's own.
        let late = truncated(&s, Stage::Curves);
        assert_eq!((late.levels, late.separation, late.print.bypassed), (s.levels, 0.6, false));
        assert_eq!(late.face, Face::Pro);
    }
}
