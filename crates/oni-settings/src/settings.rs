//! Every setting the graph consumes, the rest a photograph is imported in, and the three families
//! a paste, a roll gesture and the frame itself each own.

use crate::axis::density_axis;
use crate::blocks::{
    ChromaDenoise, Gains, Grain, Halation, PaperCorrection, Sharpen, Texture, Vignette, chroma_decompression,
    filter_balance,
};
use crate::curves::{CurveSet, SaturationCurve};
use crate::dyes::DyeSet;
use crate::film::Film;
use crate::frame::{ContactSheetGrid, Correction, FlatField, Geometry, RgbPasses, RollReference};
use crate::geometry::placing::Placing;
use crate::lens::{LensCorrection, Radial};
use crate::levels::{Levels, LevelsSet};
use crate::legacy_print::LegacyPrint;
use crate::modes::{ConversionMode, SourceEncoding};
use crate::paper::PaperStock;
use crate::spectrogram::Spectrogram;
use crate::tone::{Print, Tone};
use crate::virtual_film::{VirtualFilm, VirtualStock};
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
    /// The virtual film a digital photo is shot on, its negative read as a scanned one; unwritten on a
    /// frame that was never digital, and unrendered on any other mode.
    #[serde(rename = "virtualFilm", skip_serializing_if = "Option::is_none", default)]
    pub virtual_film: Option<VirtualFilm>,
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
    /// The stock the roll was shot on, which picking a stock writes on its roll's frames: the curve Lighting reads its
    /// negative back through.
    #[serde(rename = "dyeSet")]
    pub dye_set: DyeSet,
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
    pub print: Print,
    /// The virtual enlarger's paper, printing the negative in place of ONI's own print; none prints on ONI's.
    #[serde(rename = "printPaper", skip_serializing_if = "Option::is_none", default)]
    pub print_paper: Option<PaperStock>,
    /// The enlarger printing on a neutral paper, its paper's curves and dyes taken away; the paper stays chosen.
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub paperless: bool,
    /// The former Scanner face's print a photo edited there keeps; none on every other photo.
    #[serde(rename = "legacyPrint", skip_serializing_if = "Option::is_none", default)]
    pub legacy_print: Option<LegacyPrint>,
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
    pub texture: Texture,
    pub sharpen: Sharpen,
    pub grain: Grain,
    pub halation: Halation,
    /// A digital photo's lens, corrected by hand; a film mode keeps it unrendered.
    #[serde(rename = "lensCorrection")]
    pub lens_correction: LensCorrection,
}

impl Default for PipelineSettings {
    fn default() -> Self {
        PipelineSettings {
            mode: ConversionMode::Negative,
            inverted_source: false,
            virtual_film: None,
            film: None,
            manual_order: None,
            sheet: None,
            decoded_from_mosaic: false,
            capture_stops: 0.0,
            primaries_converted: false,
            source_encoding: None,
            rgb_passes: None,
            pass_of: None,
            geometry: Geometry::default(),
            chroma: ChromaDenoise::default(),
            gains: Gains::default(),
            density_ceiling: density_axis::base(),
            dye_set: DyeSet::default(),
            flat_field: FlatField::default(),
            corrections: Vec::new(),
            levels: LevelsSet::default(),
            chroma_decompression: chroma_decompression::BASE,
            chroma_decompression_by_hand: false,
            roll_reference: None,
            measured_gains: None,
            balance_method: None,
            balance_filters: None,
            balance_strength: filter_balance::RESTING_STRENGTH,
            print: Print::default(),
            print_paper: None,
            paperless: false,
            legacy_print: None,
            density: 0.0,
            manual_curves: CurveSet::default(),
            spectrogram: Spectrogram::default(),
            saturation: SaturationCurve::default(),
            vignette: Vignette::default(),
            secret_sauce: 0.0,
            paper_correction: PaperCorrection::default(),
            texture: Texture::default(),
            sharpen: Sharpen::default(),
            grain: Grain::default(),
            halation: Halation::default(),
            lens_correction: LensCorrection::default(),
        }
    }
}

impl PipelineSettings {
    /// `Parameters.rest`: the state a photograph is imported in; a digital photo on its default film, printed on
    /// the default paper, with its grain (owner's call: a scan takes its paper at Prepare the Roll).
    pub fn rest(mode: ConversionMode, mosaic: bool) -> PipelineSettings {
        let mut block = PipelineSettings { mode, decoded_from_mosaic: mosaic, ..PipelineSettings::default() };
        block.density_ceiling = density_axis::rest(mosaic, mode);
        block.levels = LevelsSet::neutral(mode, block.density_ceiling);
        if mode.is_digital() {
            block = block.on_stock(VirtualStock::default());
            block.print_paper = Some(PaperStock::default());
            block.grain.amount = Grain::DIGITAL_AMOUNT;
        }
        block
    }

    /// Whether a paper can print this frame: every one but a positive read as its own negative, whose light no
    /// enlarger passes, and a slide, scanned or shot on a reversal film, which is projected.
    pub fn takes_paper(&self) -> bool {
        !self.inverted_source && !self.mode.is_slide() && self.projector().is_none()
    }

    /// The paper the render prints on: the chosen one where the frame takes a paper.
    pub fn paper(&self) -> Option<PaperStock> {
        self.print_paper.filter(|_| self.takes_paper())
    }

    /// The paper a print on paper takes: the chosen one, else the default.
    pub fn fitting_paper(&self) -> PaperStock {
        self.print_paper.unwrap_or_default()
    }

    /// These settings printed on their fitting paper through the enlarger, as a prepared frame is (owner's call).
    pub fn on_paper(self) -> PipelineSettings {
        PipelineSettings { print_paper: Some(self.fitting_paper()), paperless: false, ..self }
    }

    /// The former Scanner face's print the render lays, while it is laid.
    pub fn legacy(&self) -> Option<LegacyPrint> {
        self.legacy_print.filter(|l| l.laid)
    }

    /// These settings shot on `stock`, its exposure kept: its negative's gains, windows and dyes placed from
    /// the stock's reference, as Prepare the Roll places a roll's.
    pub fn on_stock(&self, stock: VirtualStock) -> PipelineSettings {
        let mut out = self.clone();
        let reference = stock.reference();
        out.virtual_film = Some(VirtualFilm { stock, exposure: self.virtual_film.map_or(0.0, |f| f.exposure) });
        out.gains.stops = reference.stops;
        out.levels = reference.levels();
        out.dye_set = stock.dye_set();
        // A slide is projected, not printed; a negative taken after one prints on the default paper again.
        let was_slide = self.film().is_some_and(|f| f.stock.entry().reversal);
        if stock.entry().reversal {
            out.print_paper = None;
        } else if was_slide && out.print_paper.is_none() {
            out.print_paper = Some(PaperStock::default());
        }
        out
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
            let (from, to) = (PipelineSettings::rest(self.mode, self.decoded_from_mosaic), PipelineSettings::rest(next, self.decoded_from_mosaic));
            let (black, to_black) = (Levels::resting_black(self.mode, self.density_ceiling), Levels::resting_black(next, moved.density_ceiling));
            let windows = [&mut moved.levels.red, &mut moved.levels.green, &mut moved.levels.blue];
            for ((window, rest), next_rest) in windows.into_iter().zip(from.levels.windows()).zip(to.levels.windows()) {
                if *window == rest {
                    *window = next_rest;
                } else if window.black == black {
                    window.set_black(to_black);
                }
            }
            // A digital photo's film, its gains and its dyes rest on the stock's reference.
            if self.gains == from.gains {
                moved.gains = to.gains;
            }
            if self.dye_set == from.dye_set {
                moved.dye_set = to.dye_set;
            }
            if self.virtual_film == from.virtual_film {
                moved.virtual_film = to.virtual_film;
            }
            if self.print_paper == from.print_paper {
                moved.print_paper = to.print_paper;
            }
            if self.grain.amount == from.grain.amount {
                moved.grain.amount = to.grain.amount;
            }
        }
        moved
    }

    /// The lens corrections' radial map on a scan of `size` pixels: a digital photo's alone, so a film
    /// mode renders none it hides; `None` at rest.
    pub fn lens(&self, size: (f64, f64)) -> Option<Radial> {
        self.lens_correction.radial(size).filter(|_| self.mode.is_digital())
    }

    /// The optical vignetting's stops at the corner, a digital photo's alone; `None` at rest.
    pub fn optical_vignetting(&self) -> Option<f32> {
        self.lens_correction.vignetting_stops().filter(|_| self.mode.is_digital())
    }

    /// The map between the scan of `size` pixels and the frame stage 0 hands on, the lens included.
    pub fn placing(&self, size: (f64, f64)) -> Placing {
        self.geometry.placing(size).through(self.lens(size))
    }

    /// The virtual film a digital photo is shot on, its stock's default where none is stored; none on a
    /// film mode, which keeps it unrendered.
    pub fn film(&self) -> Option<VirtualFilm> {
        self.mode.is_digital().then(|| self.virtual_film.unwrap_or_default())
    }

    /// A digital photo on a film it is offered no more reads on the first one its menu lists; a key no file defines keeps
    /// its key until the file is back.
    pub fn on_offered_film(self) -> PipelineSettings {
        let Some(film) = self.film() else { return self };
        let catalogue = crate::stocks::current();
        if catalogue.virtual_film(film.stock).is_none_or(|e| e.offered()) {
            return self;
        }
        let first = catalogue.films().iter().filter_map(|k| k.film).find(|v| catalogue.virtual_film(*v).is_some_and(|e| e.offered()));
        first.map_or(self.clone(), |v| self.on_stock(v))
    }

    /// A black and white picture: a monochrome roll's, or a digital photo's on a black and white film, whose
    /// negative holds silver alone and is read as a monochrome roll's is.
    pub fn is_monochrome(&self) -> bool {
        self.mode.is_monochrome() || self.film().is_some_and(|f| f.stock.is_silver())
    }

    /// Every neighbourhood stage off, for thumbnails and per-pixel checks.
    pub fn per_pixel_only(&self) -> PipelineSettings {
        let mut out = self.clone();
        out.sharpen.amount = 0.0;
        out.chroma = ChromaDenoise::OFF;
        out.texture = Texture::default();
        out
    }
}

/// What a settings paste and Apply Settings to Roll carry: the Color and Details tabs, whole.
pub mod look_settings {
    use super::*;

    pub const FIELDS: [&str; 18] = [
        "balanceFilters",
        "print",
        "printPaper",
        "paperless",
        "legacyPrint",
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
        "grain",
        "halation",
        "lensCorrection",
    ];

    /// The frame's own facts, which neither a paste nor a roll gesture carries.
    pub const FRAME_FIELDS: [&str; 13] = [
        "film",
        "manualOrder",
        "sheet",
        "decodedFromMosaic",
        "primariesConverted",
        "geometry",
        "corrections",
        "rollReference",
        "measuredGains",
        "balanceMethod",
        "captureStops",
        "rgbPasses",
        "passOf",
    ];

    pub fn copied(donor: &PipelineSettings, target: &PipelineSettings) -> PipelineSettings {
        let mut out = target.clone();
        out.balance_filters = donor.balance_filters;
        out.print = donor.print;
        out.print_paper = donor.print_paper;
        out.paperless = donor.paperless;
        out.legacy_print = donor.legacy_print;
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
        out.grain = donor.grain;
        out.halation = donor.halation;
        out.lens_correction = donor.lens_correction;
        out
    }
}

/// What a roll shares: the Roll Settings pane's fields, one value on every frame of the roll.
mod projector;
pub mod roll_settings;
