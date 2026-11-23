//! The steps run on a stored document's raw JSON at its stored names, before any typed read, each
//! for the versions written before its change, in the order the format moved.

use crate::blocks::chroma_decompression;
use crate::settings::PipelineSettings;
use crate::tolerant::{self, Object};
use serde_json::Value;
use std::sync::OnceLock;

/// The version Chroma Decompression came to rest halfway.
pub const DECOMPRESSION_MID_VERSION: i64 = 14;
/// The version a file outside the RAW engine came to keep the channels it holds.
pub const PRIMARIES_HELD_VERSION: i64 = 15;
/// The version a scanner click became half the machine's measured step.
pub const SCANNER_HALF_STEP_VERSION: i64 = 17;
/// The version ONI's own machine came to rest on the Scanner face.
pub const ONI_MACHINE_VERSION: i64 = 18;
/// The version Scanner Emulation left, folded into the photo's Curves and Color Density.
pub const LOOK_FOLDED_VERSION: i64 = 19;
/// The version the camera matrices left the RAW decode, their move kept in the gains.
pub const MATRICES_RETIRED_VERSION: i64 = 20;
/// The version a dust stroke came to be kept on the scan rather than on the cropped, oriented frame.
pub const STROKES_ON_SCAN_VERSION: i64 = 21;
/// The version the document came to name itself `oni-frame`, in the shape this format types.
pub const NAMED_VERSION: i64 = 22;
/// The version an EXIF line came to keep no field equal to the scan's own fact.
pub const SCAN_LINE_RELEASED_VERSION: i64 = 23;
/// The version each channel came to keep its own white and black points, absent read as zero.
pub const CHANNEL_POINTS_VERSION: i64 = 24;
/// The version a digital photo came to keep its own levels on the picture, absent read as rest.
pub const PICTURE_LEVELS_VERSION: i64 = 27;
/// The version a digital photo came to keep its lens corrections, absent read as none.
pub const LENS_CORRECTION_VERSION: i64 = 28;
/// The version each channel came to keep its own mids and contrast, and the halation its sensitivity,
/// absent read as rest.
pub const CHANNEL_TONE_VERSION: i64 = 29;
/// The version a digital photo came to print on the Scanner face's machine, one stored there before on Pro.
pub const DIGITAL_SCANNER_VERSION: i64 = 30;
/// The version the grain came to keep its strength by tone, absent read as rest.
pub const GRAIN_CURVE_VERSION: i64 = 31;
/// The version the grain came to keep its colour along the tone, absent read as its rest.
pub const GRAIN_COLOUR_VERSION: i64 = 32;
/// The version the grain's colour came to deepen the photo's own hue where the grain darkens it, in place of
/// a hue drawn at random: its numbers carried, as no step keeps a random hue's look.
pub const GRAIN_HUE_VERSION: i64 = 33;
/// The version a roll came to name its lens, absent read as none: a build before it would drop the lens it rewrites.
pub const ROLL_LENS_VERSION: i64 = 34;
/// The version a digital photo came to print in black and white, absent read as colour, and its Auto White
/// Balance to keep the white it found: a build before it would drop either as it rewrites the file.
pub const DIGITAL_BLACK_AND_WHITE_VERSION: i64 = 35;
/// The version a dust correction came to name the gesture that laid it, absent read as laid alone: a build before
/// it would drop the gesture it rewrites.
pub const GESTURE_VERSION: i64 = 36;
/// The version a digital photo came to be shot on a virtual film, absent read as none: a build before it
/// would drop the film it rewrites.
pub const VIRTUAL_FILM_VERSION: i64 = 37;
/// The version a digital photo came to be read by the negative's pipeline, always on its film: its white,
/// picture levels and Black and White switch dropped, its film's reference moved into its gains and windows.
pub const DIGITAL_ON_FILM_VERSION: i64 = 38;
/// The version the dye separation came to unmix the stock's measured dyes, the family's rows gone: a dose still on
/// their rest takes the whole unmix, a hand's stays.
pub const STOCK_DYES_VERSION: i64 = 39;
/// The version a frame's measured dyes came to name each stock of a line, a key a build before it cannot read: no
/// step, as nothing a document of 39 holds moves.
pub const STOCK_SPLIT_VERSION: i64 = 40;
/// The version a negative came to print on a colour paper: every photo of an earlier one stays on ONI's own print.
pub const PAPER_VERSION: i64 = 41;
/// The version the print's curve came to take Exposure's distance from its rest twice: each value's distance halved.
pub const EXPOSURE_GEAR_VERSION: i64 = 42;
/// The version Lighting's colour compensation came to be switched off, a key a build before it cannot read: no step,
/// as every older document reads it on.
pub const COLOUR_COMPENSATION_VERSION: i64 = 43;
/// The version a paper's print came to keep its Brightness and True blacks, keys a build before it cannot read: no step,
/// as every older document reads Brightness 0 and True blacks off.
pub const PAPER_ENDS_VERSION: i64 = 44;
/// The version a photo came to print on the Frontier SP-3000, a paper key a build before it cannot read: no step, as
/// no older document names it.
pub const FRONTIER_PRINT_VERSION: i64 = 45;
/// The version the Scanner face left: a photo it rendered keeps that edit alone, its machine's print in Lighting's fields
/// and its keys and look in a legacy print, a key a build before it cannot read.
pub const ONE_FACE_VERSION: i64 = 46;
/// The version a slide came to print on a reversal paper, a paper key a build before it cannot read: no step, as no
/// older document names it.
pub const REVERSAL_PAPER_VERSION: i64 = 52;
/// The version a paper's black came to rest on pure black, Paper blacks keeping the paper's own: True blacks retired, a key
/// a build before it cannot read.
pub const PAPER_BLACKS_VERSION: i64 = 53;
/// The version a digital photo came to be shot on any colour negative then profiled, a stock key a build before it cannot
/// read: no step, as every older document names Portra 800.
pub const VIRTUAL_STOCKS_VERSION: i64 = 54;
/// The version Grain size came to be a grain's width on the film, in micrometres of its long side, in place of a
/// share of its track laid on the scan's width.
pub const GRAIN_ON_FILM_VERSION: i64 = 55;
/// The version a digital photo came to keep its own film's gains and dyes, a folder's roll never laying another film's.
pub const OWN_FILM_VERSION: i64 = 56;
/// The version a slide came to print on a negative paper through an internegative, the reversal paper retired: a slide
/// that named a negative paper rendered on ONI's print, one on the reversal paper printed on paper.
pub const SLIDE_INTERNEGATIVE_VERSION: i64 = 58;
/// The version every film stock and paper came to be profiled from its maker's data sheet, spektrafilm's profiles
/// retired: a digital photo's gains and windows move onto its stock's new reference, and a build before it would print
/// every key on spektrafilm's profile.
pub const DATASHEET_VERSION: i64 = 60;
/// The version Lighting came to act on the negative in every pipe, each print at its rest: a legacy print's look came to
/// read its machine's entry itself, its photo's Exposure moved off it to Lighting's rest.
pub const LIGHTING_ON_NEGATIVE_VERSION: i64 = 61;
/// The version Lighting's Exposure came to be the departure from its pipe's rest, 0 at rest in every mode: each value
/// moved by its mode's former rest.
pub const EXPOSURE_DEPARTURE_VERSION: i64 = 64;
/// The version each colour profile came to develop its dyes' amounts, its sheet's curves unmixed: a digital photo's gains
/// and windows move onto its stock's new reference.
pub const UNMIXED_VERSION: i64 = 66;
/// The version Paper blacks came to be Print lift, keeping the paper's white with its black, off laying that white on pure
/// white: its stored key renamed, a key a build before it cannot read.
pub const PRINT_LIFT_VERSION: i64 = 67;
/// The version the dye separation left both prints, each reading the negative's own per-channel density: a digital
/// photo's virtual negative codes each layer's density on its own channel, its gains and windows moving onto its stock's
/// new reference; the dose and its key dropped, as no print reads them.
pub const LAYER_DENSITIES_VERSION: i64 = 69;
/// The version a slide came to be projected on a white screen, never printed: a scanned slide's paper and the paper a
/// digital photo on a reversal film named dropped, its scan read back to its roll's reversal film, a key a build before
/// it would read as another paper's print.
pub const SLIDE_PROJECTOR_VERSION: i64 = 70;

/// `document` aged to this format's names and units; `mosaic` is `None` for a document of no file.
pub(super) fn aged(document: Object, version: i64, mosaic: Option<bool>) -> Object {
    let mut aged = if version < super::before_1_0::FIRST_RELEASED_VERSION {
        super::before_1_0::from_before_1_0(document, version, &|| mosaic == Some(true))
    } else {
        document
    };
    if version < DECOMPRESSION_MID_VERSION {
        aged = off_former_key_decompression(aged);
    }
    if version < PRIMARIES_HELD_VERSION && mosaic == Some(false) {
        aged = keeping_converted_primaries(aged);
    }
    if version < SCANNER_HALF_STEP_VERSION {
        aged = on_half_steps(aged);
    }
    if version < ONI_MACHINE_VERSION {
        aged = on_frontier(aged);
    }
    aged
}

/// The resting settings every stored document of format 21 and under is laid over, built once.
pub fn neutral_document() -> &'static Value {
    static NEUTRAL: OnceLock<Value> = OnceLock::new();
    NEUTRAL.get_or_init(|| serde_json::json!({ "settings": serde_json::to_value(PipelineSettings::default()).unwrap_or(Value::Null) }))
}

/// Completes `stored` with the keys of `defaults`, recursively; the stored value always wins and
/// arrays are taken whole, so a curve keeps its length.
pub fn merged(stored: &Value, defaults: &Value) -> Value {
    match (stored, defaults) {
        (Value::Object(s), Value::Object(d)) => {
            let mut result = d.clone();
            for (key, value) in s {
                let under = d.get(key).unwrap_or(value);
                result.insert(key.clone(), merged(value, under));
            }
            Value::Object(result)
        }
        _ => stored.clone(),
    }
}

/// Applies `edit` to the document's settings object, leaving a document without one as it is.
pub(crate) fn with_settings(mut document: Object, edit: impl FnOnce(&mut Object)) -> Object {
    if let Some(Value::Object(settings)) = document.get_mut("settings") {
        edit(settings);
    }
    document
}

/// Whether `settings` printed on a paper as the formats before 70 read it: any frame naming one, a slide included, but a
/// positive read as its own negative.
pub(crate) fn papered(settings: &PipelineSettings) -> bool {
    settings.print_paper.is_some() && !settings.inverted_source
}

pub(crate) fn number(value: f32) -> Value {
    Value::from(f64::from(value))
}

fn is_true(value: Option<&Value>) -> bool {
    tolerant::ns_bool(value) == Some(true)
}

/// A dose a key left on the film's colour moves to the rest; the hand's stays where it is.
fn off_former_key_decompression(document: Object) -> Object {
    with_settings(document, |s| {
        if tolerant::number_f32(s.get("chromaDecompression")) == Some(chroma_decompression::FORMER_KEY_REST)
            && !is_true(s.get("chromaDecompressionByHand"))
        {
            s.insert("chromaDecompression".into(), number(chroma_decompression::BASE));
        }
    })
}

/// Every frame read before keeps the decode its windows were placed on.
fn keeping_converted_primaries(document: Object) -> Object {
    with_settings(document, |s| {
        s.insert("primariesConverted".into(), Value::Bool(true));
    })
}

/// A frame written before renders through the Frontier, the one machine it knew.
fn on_frontier(document: Object) -> Object {
    with_settings(document, |s| {
        s.insert("scannerMachine".into(), Value::from("frontier"));
    })
}

/// Every D, C, M and Y count doubles, so a Scanner edit renders as it did.
fn on_half_steps(document: Object) -> Object {
    with_settings(document, |s| {
        if let Some(Value::Object(clicks)) = s.get_mut("scanner") {
            for key in ["d", "c", "m", "y"] {
                if let Some(count) = tolerant::ns_integer(clicks.get(key)) {
                    clicks.insert(key.into(), Value::from(count * 2));
                }
            }
        }
    })
}

