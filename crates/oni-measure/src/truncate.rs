//! `PipelineSettings.truncated(before:)`: the frame as it enters a stage on the face that renders,
//! everything downstream at rest, the one recipe every reading is taken through.

use oni_settings::axis::graduation;
use oni_settings::blocks::{ChromaDenoise, Texture, Vignette};
use oni_settings::curves::{CurveSet, SaturationCurve};
use oni_settings::levels::LevelsSet;
use oni_settings::modes::Face;
use oni_settings::scanner::ScannerFinish;
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
    out.saturation = SaturationCurve::default();
    out.secret_sauce = 0.0;
    out.texture = Texture::default();
    // The amount alone: the sharpening's rest carries a real dose.
    out.sharpen.amount = 0.0;
    // The Scanner face renders its own finishing in Pro's place: at rest alike, but the print's contrast.
    out.scanner_finish = ScannerFinish { contrast: s.scanner_finish.contrast, manual_curves: s.scanner_finish.manual_curves.clone(), ..ScannerFinish::default() };
    match before {
        Stage::Levels | Stage::GlobalLevels => {
            // In front of the print a reading is of the film through Pro's windows, whichever face renders.
            out.face = Face::Pro;
            out.print.bypassed = true;
            out.density = 0.0;
            out.manual_curves = CurveSet::default();
        }
        Stage::ManualCurves => {
            out.manual_curves = CurveSet::default();
            out.scanner_finish.manual_curves = CurveSet::default();
        }
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
        assert_eq!(truncated(&s, Stage::GlobalLevels).face, Face::Pro, "Pro's windows in front of the print");
        // Twin: before the curves, the windows, the separation and the face are the frame's own.
        let late = truncated(&s, Stage::Curves);
        assert_eq!((late.levels, late.separation, late.print.bypassed), (s.levels, 0.6, false));
        assert_eq!(late.face, Face::Scanner);
    }

    #[test]
    fn before_the_curves_the_scanner_finishing_rests_but_its_contrast() {
        let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
        s.face = Face::Scanner;
        s.scanner_finish.contrast = 0.2;
        s.scanner_finish.secret_sauce = 0.4;
        s.scanner_finish.vignette.stops = -0.5;
        s.scanner_finish.manual_curves.linked.points = vec![Point::new(0.0, 0.0), Point::new(0.3, 0.8), Point::new(1.0, 1.0)];
        let t = truncated(&s, Stage::ManualCurves);
        assert_eq!(t.face, Face::Scanner, "the curves' input is the face's own");
        assert_eq!(t.scanner_finish, ScannerFinish { contrast: 0.2, ..ScannerFinish::default() });
        // Twin: in front of a later stage the face's own curves stay, as Pro's do.
        assert_eq!(truncated(&s, Stage::Curves).scanner_finish.manual_curves, s.scanner_finish.manual_curves);
        assert_ne!(t.scanner_finish.manual_curves, s.scanner_finish.manual_curves);
    }
}
