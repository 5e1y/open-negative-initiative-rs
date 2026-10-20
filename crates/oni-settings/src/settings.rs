//! Every setting the graph consumes, the rest a photograph is imported in, and the three families
//! a paste, a roll gesture and the frame itself each own.

use crate::axis::density_axis;
use crate::blocks::{
    ChromaDenoise, Gains, PaperCorrection, SeparationFamily, Sharpen, Texture, Vignette, chroma_decompression,
    filter_balance, separation,
};
use crate::curves::{CurveSet, SaturationCurve};
use crate::film::Film;
use crate::frontier_lab::FrontierLab;
use crate::frame::{ContactSheetGrid, Correction, FlatField, Geometry, RgbPasses, RollReference};
use crate::levels::LevelsSet;
use crate::modes::{ConversionMode, Face, ScannerMachine, SourceEncoding};
use crate::scanner::{ScannerClicks, ScannerFinish};
use crate::spectrogram::Spectrogram;
use crate::tone::{Print, Tone};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Every setting, its serialised names the Swift app's. Optional fields are omitted when absent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineSettings {
    pub mode: ConversionMode,
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
    #[serde(rename = "balanceMethod", skip_serializing_if = "Option::is_none", default)]
    pub balance_method: Option<String>,
    #[serde(rename = "balanceFilters", skip_serializing_if = "Option::is_none", default)]
    pub balance_filters: Option<[f32; 3]>,
    #[serde(rename = "balanceStrength")]
    pub balance_strength: f32,
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
    /// The Look Lab's edits of the Frontier, absent when the frame renders the measured table.
    #[serde(rename = "frontierLab", skip_serializing_if = "FrontierLab::is_neutral", default)]
    pub frontier_lab: FrontierLab,
    pub texture: Texture,
    pub sharpen: Sharpen,
}

impl Default for PipelineSettings {
    fn default() -> Self {
        PipelineSettings {
            mode: ConversionMode::Negative,
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
            balance_method: None,
            balance_filters: None,
            balance_strength: filter_balance::RESTING_STRENGTH,
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
            frontier_lab: FrontierLab::default(),
            texture: Texture::default(),
            sharpen: Sharpen::default(),
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

    pub const FIELDS: [&str; 16] = [
        "balanceFilters",
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
        "frontierLab",
    ];

    /// The frame's own facts, which neither a paste nor a roll gesture carries.
    pub const FRAME_FIELDS: [&str; 15] = [
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
        "balanceMethod",
        "captureStops",
        "rgbPasses",
        "passOf",
        "face",
    ];

    pub fn copied(donor: &PipelineSettings, target: &PipelineSettings) -> PipelineSettings {
        let mut out = target.clone();
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
        out.frontier_lab = donor.frontier_lab;
        out
    }
}

/// What a roll shares: the Roll Settings pane's fields, one value on every frame of the roll.
pub mod roll_settings {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub enum Field {
        Mode,
        SourceEncoding,
        FlatField,
        Gains,
        DensityCeiling,
        Levels,
        Decompression,
        Separation,
        BalanceStrength,
    }

    impl Field {
        pub const ALL: [Field; 9] = [
            Field::Mode,
            Field::SourceEncoding,
            Field::FlatField,
            Field::Gains,
            Field::DensityCeiling,
            Field::Levels,
            Field::Decompression,
            Field::Separation,
            Field::BalanceStrength,
        ];
    }

    pub const FIELD_NAMES: [&str; 12] = [
        "mode",
        "sourceEncoding",
        "flatField",
        "gains",
        "densityCeiling",
        "levels",
        "chromaDecompression",
        "chromaDecompressionByHand",
        "separation",
        "separationFamily",
        "separationByHand",
        "balanceStrength",
    ];

    pub fn changed(before: &PipelineSettings, after: &PipelineSettings) -> BTreeSet<Field> {
        Field::ALL
            .into_iter()
            .filter(|field| match field {
                Field::Mode => before.mode != after.mode,
                Field::SourceEncoding => before.source_encoding != after.source_encoding,
                Field::FlatField => before.flat_field != after.flat_field,
                Field::Gains => before.gains != after.gains,
                Field::DensityCeiling => before.density_ceiling != after.density_ceiling,
                Field::Levels => before.levels != after.levels,
                Field::Decompression => {
                    (before.chroma_decompression, before.chroma_decompression_by_hand)
                        != (after.chroma_decompression, after.chroma_decompression_by_hand)
                }
                Field::Separation => {
                    before.separation != after.separation
                        || before.separation_by_hand != after.separation_by_hand
                        || before.separation_family != after.separation_family
                }
                Field::BalanceStrength => before.balance_strength != after.balance_strength,
            })
            .collect()
    }

    /// Balance strength re-doses the filters a frame no key balanced leaves alone.
    pub fn dosed(settings: &PipelineSettings, strength: f32) -> PipelineSettings {
        let mut out = settings.clone();
        out.balance_strength = strength.clamp(filter_balance::STRENGTH_RANGE.0, filter_balance::STRENGTH_RANGE.1);
        if let Some(full) = settings.balance_filters {
            let s = out.balance_strength;
            out.print.red = Tone::exposure(full[0] * s);
            out.print.green = Tone::exposure(full[1] * s);
            out.print.blue = Tone::exposure(full[2] * s);
        }
        out
    }

    /// `target` carrying `donor`'s value of each field; a contact sheet never takes a second flat-field.
    pub fn copied(fields: &BTreeSet<Field>, donor: &PipelineSettings, target: &PipelineSettings) -> PipelineSettings {
        let mut out = target.clone();
        for field in fields {
            if *field == Field::FlatField && target.sheet.is_some() {
                continue;
            }
            match field {
                Field::Mode => out.mode = donor.mode,
                Field::SourceEncoding => out.source_encoding = donor.source_encoding,
                Field::FlatField => out.flat_field = donor.flat_field.clone(),
                Field::Gains => out.gains = donor.gains,
                Field::DensityCeiling => out.density_ceiling = donor.density_ceiling,
                Field::Levels => out.levels = donor.levels,
                Field::Decompression => {
                    out.chroma_decompression = donor.chroma_decompression;
                    out.chroma_decompression_by_hand = donor.chroma_decompression_by_hand;
                }
                Field::Separation => {
                    out.separation = donor.separation;
                    out.separation_by_hand = donor.separation_by_hand;
                    out.separation_family = donor.separation_family;
                }
                Field::BalanceStrength => out = dosed(&out, donor.balance_strength),
            }
        }
        out
    }
}
