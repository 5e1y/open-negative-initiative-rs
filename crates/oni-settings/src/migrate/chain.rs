//! The steps run on a stored document's raw JSON at its stored names, before any typed read, each
//! for the versions written before its change, in the order the format moved.

use crate::blocks::{chroma_decompression, separation};
use crate::modes::ScannerMachine;
use crate::settings::PipelineSettings;
use crate::tolerant::{self, Object};
use serde_json::Value;
use std::sync::OnceLock;

/// The version Chroma Decompression came to rest halfway.
pub const DECOMPRESSION_MID_VERSION: i64 = 14;
/// The version a file outside the RAW engine came to keep the channels it holds.
pub const PRIMARIES_HELD_VERSION: i64 = 15;
/// The version the dye separation came back to rest on the whole spill, a key writing it.
pub const SEPARATION_BY_HAND_VERSION: i64 = 14;
/// The version the dye separation came to rest on the literature's dose.
pub const SEPARATION_LITERATURE_VERSION: i64 = 16;
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

/// `document` aged to this format's names and units; `mosaic` is `None` for a document of no file.
pub(super) fn aged(document: Object, version: i64, mosaic: Option<bool>) -> Object {
    let mut aged = if version < super::before_1_0::FIRST_RELEASED_VERSION {
        super::before_1_0::from_before_1_0(document, version, &|| mosaic == Some(true))
    } else {
        document
    };
    if version < SEPARATION_BY_HAND_VERSION {
        aged = keeping_former_separation(aged);
    }
    if version < SEPARATION_LITERATURE_VERSION {
        aged = off_previous_separation_rest(aged);
    }
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
        s.insert("scannerMachine".into(), Value::from(ScannerMachine::Frontier.raw_value()));
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

fn off_previous_separation_rest(document: Object) -> Object {
    with_settings(document, |s| {
        if tolerant::number_f32(s.get("separation")) == Some(separation::PREVIOUS_BASE)
            && !is_true(s.get("separationByHand"))
        {
            s.insert("separation".into(), number(separation::BASE));
        }
    })
}

/// A frame from before renders on at its dose until a key balances it; a dose off the former rest was the hand's.
fn keeping_former_separation(document: Object) -> Object {
    with_settings(document, |s| match tolerant::number_f32(s.get("separation")) {
        Some(stored) => {
            s.insert("separationByHand".into(), Value::Bool(stored != separation::FORMER_REST));
        }
        None => {
            s.insert("separation".into(), number(separation::FORMER_REST));
        }
    })
}
