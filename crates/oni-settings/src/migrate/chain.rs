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
