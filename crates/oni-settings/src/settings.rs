//! Every setting the graph consumes, the rest a photograph is imported in, and the three families
//! a paste, a roll gesture and the frame itself each own.

use crate::axis::density_axis;
use crate::blocks::{
    ChromaDenoise, Gains, Grain, Halation, PaperCorrection, SeparationFamily, Sharpen, Texture, Vignette, chroma_decompression,
    filter_balance, separation,
};
use crate::curves::{CurveSet, SaturationCurve};
use crate::film::Film;
use crate::frame::{ContactSheetGrid, Correction, FlatField, Geometry, RgbPasses, RollReference};
use crate::geometry::placing::Placing;
use crate::lens::{LensCorrection, Radial};
use crate::levels::{Levels, LevelsSet, PictureLevels};
use crate::modes::{ConversionMode, Face, ScannerMachine, SourceEncoding};
use crate::scanner::{ScannerClicks, ScannerFinish};
use crate::spectrogram::Spectrogram;
use crate::tone::{Print, Tone};
use crate::white::White;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Every setting, its serialised names the Swift app's. Optional fields are omitted when absent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PipelineSettings {
    pub mode: ConversionMode,
    /// A positive read as its own negative, every density flipped before the negative's conversion
    /// (owner's experiment); false, and unwritten, on every other frame.
    #[serde(rename = "invertedSource", skip_serializing_if = "std::ops::Not::not", default)]
    pub inverted_source: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub film: Option<Film>,
    #[serde(rename = "manualOrder", skip_serializing_if = "Option::is_none", default)]
    pub manual_order: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sheet: Option<ContactSheetGrid>,
    #[serde(rename = "decodedFromMosaic")]
    pub decoded_from_mosaic: bool,
    #[serde(rename = "captureStops")]
    pub capture_stops: f32,
    #[serde(rename = "lensCorrected")]
    pub lens_corrected: bool,
    #[serde(rename = "primariesConverted")]
    pub primaries_converted: bool,
    #[serde(rename = "sourceEncoding", skip_serializing_if = "Option::is_none", default)]
    pub source_encoding: Option<SourceEncoding>,
    #[serde(rename = "rgbPasses", skip_serializing_if = "Option::is_none", default)]
    pub rgb_passes: Option<RgbPasses>,
    #[serde(rename = "passOf", skip_serializing_if = "Option::is_none", default)]
    pub pass_of: Option<String>,
    pub geometry: Geometry,
    pub chroma: ChromaDenoise,
    pub gains: Gains,
    #[serde(rename = "densityCeiling")]
    pub density_ceiling: f32,
    pub separation: f32,
    #[serde(rename = "separationFamily", skip_serializing_if = "Option::is_none", default)]
    pub separation_family: Option<SeparationFamily>,
    #[serde(rename = "separationByHand")]
    pub separation_by_hand: bool,
    #[serde(rename = "flatField")]
    pub flat_field: FlatField,
    pub corrections: Vec<Correction>,
    pub levels: LevelsSet,
    #[serde(rename = "chromaDecompression")]
    pub chroma_decompression: f32,
    #[serde(rename = "chromaDecompressionByHand")]
    pub chroma_decompression_by_hand: bool,
    #[serde(rename = "rollReference", skip_serializing_if = "Option::is_none", default)]
    pub roll_reference: Option<RollReference>,
    #[serde(rename = "rollReferenceDeclined")]
    pub roll_reference_declined: bool,
    /// A slide's gains as its gains-only measure placed them, on no roll reference: what its Source
    /// Balance rests on and a reset lands back on (owner's call).
    #[serde(rename = "measuredGains", skip_serializing_if = "Option::is_none", default)]
    pub measured_gains: Option<[f32; 3]>,
    #[serde(rename = "balanceMethod", skip_serializing_if = "Option::is_none", default)]
    pub balance_method: Option<String>,
    #[serde(rename = "balanceFilters", skip_serializing_if = "Option::is_none", default)]
    pub balance_filters: Option<[f32; 3]>,
    #[serde(rename = "balanceStrength")]
    pub balance_strength: f32,
    /// The light a digital photo is balanced for; `None` is the white its decode stands on.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub white: Option<White>,
    /// A RAW's As Shot white, read from its metadata as it became digital: where its white rests.
    #[serde(rename = "shotWhite", skip_serializing_if = "Option::is_none", default)]
    pub shot: Option<White>,
    pub print: Print,
    pub density: f32,
    #[serde(rename = "manualCurves")]
    pub manual_curves: CurveSet,
    pub spectrogram: Spectrogram,
    pub saturation: SaturationCurve,
    pub vignette: Vignette,
    #[serde(rename = "secretSauce")]
    pub secret_sauce: f32,
    #[serde(rename = "paperCorrection")]
    pub paper_correction: PaperCorrection,
    pub face: Face,
    pub scanner: ScannerClicks,
    #[serde(rename = "scannerMachine")]
    pub scanner_machine: ScannerMachine,
    #[serde(rename = "scannerFinish")]
    pub scanner_finish: ScannerFinish,
    pub texture: Texture,
    pub sharpen: Sharpen,
    pub grain: Grain,
    pub halation: Halation,
    /// A digital photo's own levels, laid in the curves' tables; a film frame keeps them unrendered.
    #[serde(rename = "pictureLevels", skip_serializing_if = "PictureLevels::is_neutral", default)]
    pub picture_levels: PictureLevels,
    /// A digital photo's lens, corrected by hand; a film mode keeps it unrendered.
    #[serde(rename = "lensCorrection")]
    pub lens_correction: LensCorrection,
}

impl Default for PipelineSettings {
    fn default() -> Self {
        PipelineSettings {
            mode: ConversionMode::Negative,
            inverted_source: false,
            film: None,
            manual_order: None,
            sheet: None,
            decoded_from_mosaic: false,
            capture_stops: 0.0,
            lens_corrected: false,
            primaries_converted: false,
            source_encoding: None,
            rgb_passes: None,
            pass_of: None,
            geometry: Geometry::default(),
            chroma: ChromaDenoise::default(),
            gains: Gains::default(),
            density_ceiling: density_axis::base(),
            separation: separation::BASE,
            separation_family: None,
            separation_by_hand: false,
            flat_field: FlatField::default(),
            corrections: Vec::new(),
            levels: LevelsSet::default(),
            chroma_decompression: chroma_decompression::BASE,
            chroma_decompression_by_hand: false,
            roll_reference: None,
            roll_reference_declined: false,
            measured_gains: None,
            balance_method: None,
            balance_filters: None,
            balance_strength: filter_balance::RESTING_STRENGTH,
            white: None,
            shot: None,
            print: Print::default(),
            density: 0.0,
            manual_curves: CurveSet::default(),
            spectrogram: Spectrogram::default(),
            saturation: SaturationCurve::default(),
            vignette: Vignette::default(),
            secret_sauce: 0.0,
            paper_correction: PaperCorrection::default(),
            face: Face::Pro,
            scanner: ScannerClicks::default(),
            scanner_machine: ScannerMachine::Oni,
            scanner_finish: ScannerFinish::default(),
            texture: Texture::default(),
            sharpen: Sharpen::default(),
            grain: Grain::default(),
            halation: Halation::default(),
            picture_levels: PictureLevels::default(),
            lens_correction: LensCorrection::default(),
        }
    }
}

impl PipelineSettings {
    /// `Parameters.rest`: the state a photograph is imported in.
    pub fn rest(mode: ConversionMode, mosaic: bool) -> PipelineSettings {
        let mut block = PipelineSettings { mode, decoded_from_mosaic: mosaic, ..PipelineSettings::default() };
        block.density_ceiling = density_axis::rest(mosaic, mode);
        block.levels = LevelsSet::neutral(mode, block.density_ceiling);
        block.print.exposure = Print::resting_exposure(mode);
        block
    }

    /// These settings on mode `next`: each value still on its mode's rest takes the next one's, so a
    /// mode change moves no value the hand placed and leaves none on another mode's rest.
    pub fn carried(&self, next: ConversionMode) -> PipelineSettings {
        let mut moved = self.clone();
        moved.mode = next;
        // A positive read as a negative is one as long as it stays a negative.
        moved.inverted_source &= next == ConversionMode::Negative;
        moved.density_ceiling = density_axis::carried(self.density_ceiling, self.decoded_from_mosaic, self.mode, next);
        if self.mode != next {
            let (rest, to) = (Levels::resting_black(self.mode, self.density_ceiling), Levels::resting_black(next, moved.density_ceiling));
            for window in [&mut moved.levels.red, &mut moved.levels.green, &mut moved.levels.blue] {
                if window.black == rest {
                    window.set_black(to);
                }
            }
        }
        if self.print.exposure == Print::resting_exposure(self.mode) {
            moved.print.exposure = Print::resting_exposure(next);
        }
        moved
    }

    /// The lens corrections' radial map on a scan of `size` pixels: a digital photo's alone, so a film
    /// mode renders none it hides; `None` at rest.
    pub fn lens(&self, size: (f64, f64)) -> Option<Radial> {
        self.lens_correction.radial(size).filter(|_| !self.mode.is_film())
    }

    /// The optical vignetting's stops at the corner, a digital photo's alone; `None` at rest.
    pub fn optical_vignetting(&self) -> Option<f32> {
        self.lens_correction.vignetting_stops().filter(|_| !self.mode.is_film())
    }

    /// The map between the scan of `size` pixels and the frame stage 0 hands on, the lens included.
    pub fn placing(&self, size: (f64, f64)) -> Placing {
        self.geometry.placing(size).through(self.lens(size))
    }

    /// Lighting, each channel's own tone and Saturation at rest, the print on unless bypassed: a digital
    /// photo's picture as its levels and curves receive it.
    pub fn unlit(&self) -> PipelineSettings {
        PipelineSettings { print: self.unlit_print(), density: crate::tone::color_density::BASE, ..self.clone() }
    }

    /// The print with Lighting and each channel's own tone at rest, bypassed where this one is.
    pub fn unlit_print(&self) -> Print {
        Print { exposure: Print::resting_exposure(self.mode), bypassed: self.print.bypassed, ..Print::default() }
    }

    /// Every neighbourhood stage off, for thumbnails and per-pixel checks.
    pub fn per_pixel_only(&self) -> PipelineSettings {
        let mut out = self.clone();
        out.sharpen.amount = 0.0;
        out.chroma = ChromaDenoise::OFF;
        out.texture = Texture::default();
        out
    }

    /// These settings as the Scanner face edits and renders them: its own finishing in Pro's fields.
    pub fn as_scanner(&self) -> PipelineSettings {
        let mut out = self.clone();
        out.print.contrast = self.scanner_finish.contrast;
        out.manual_curves = self.scanner_finish.manual_curves.clone();
        out.spectrogram = self.scanner_finish.spectrogram;
        out.saturation = self.scanner_finish.saturation.clone();
        out.vignette = self.scanner_finish.vignette;
        out.secret_sauce = self.scanner_finish.secret_sauce;
        out
    }

    /// An edit made on the Scanner face, its finishing stored apart and Pro's own kept.
    pub fn storing_scanner_edit(&self, edited: &PipelineSettings) -> PipelineSettings {
        let mut out = edited.clone();
        out.scanner_finish = ScannerFinish {
            contrast: edited.print.contrast,
            manual_curves: edited.manual_curves.clone(),
            spectrogram: edited.spectrogram,
            saturation: edited.saturation.clone(),
            vignette: edited.vignette,
            secret_sauce: edited.secret_sauce,
        };
        out.print.contrast = self.print.contrast;
        out.manual_curves = self.manual_curves.clone();
        out.spectrogram = self.spectrogram;
        out.saturation = self.saturation.clone();
        out.vignette = self.vignette;
        out.secret_sauce = self.secret_sauce;
        out
    }
}

/// What a settings paste and Apply Settings to Roll carry: the Color and Details tabs, whole.
pub mod look_settings {
    use super::*;

    pub const FIELDS: [&str; 20] = [
        "balanceFilters",
        "white",
        "print",
        "density",
        "manualCurves",
        "spectrogram",
        "saturation",
        "vignette",
        "secretSauce",
        "paperCorrection",
        "chroma",
        "texture",
        "sharpen",
        "scanner",
        "scannerFinish",
        "scannerMachine",
        "grain",
        "halation",
        "pictureLevels",
        "lensCorrection",
    ];

    /// The frame's own facts, which neither a paste nor a roll gesture carries.
    pub const FRAME_FIELDS: [&str; 17] = [
        "film",
        "manualOrder",
        "sheet",
        "decodedFromMosaic",
        "lensCorrected",
        "primariesConverted",
        "geometry",
        "corrections",
        "rollReference",
        "rollReferenceDeclined",
        "measuredGains",
        "balanceMethod",
        "captureStops",
        "rgbPasses",
        "passOf",
        "face",
        "shotWhite",
    ];

    pub fn copied(donor: &PipelineSettings, target: &PipelineSettings) -> PipelineSettings {
        let mut out = target.clone();
        out.white = crate::white::carried(donor.white, target);
        out.balance_filters = donor.balance_filters;
        out.print = donor.print;
        out.density = donor.density;
        out.manual_curves = donor.manual_curves.clone();
        out.spectrogram = donor.spectrogram;
        out.saturation = donor.saturation.clone();
        out.vignette = donor.vignette;
        out.secret_sauce = donor.secret_sauce;
        out.paper_correction = donor.paper_correction;
        out.chroma = donor.chroma;
        out.texture = donor.texture;
        out.sharpen = donor.sharpen;
        out.scanner = donor.scanner;
        out.scanner_finish = donor.scanner_finish.clone();
        out.scanner_machine = donor.scanner_machine;
        out.grain = donor.grain;
        out.halation = donor.halation;
        out.picture_levels = donor.picture_levels;
        out.lens_correction = donor.lens_correction;
        out
    }
}

/// What a roll shares: the Roll Settings pane's fields, one value on every frame of the roll.
pub mod roll_settings;
