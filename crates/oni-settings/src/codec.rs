//! The sidecar's document: settings decoded over the resting document, so an absent key reads
//! neutral, and every migration a stored version still needs, run on the raw JSON first.

use crate::blocks::{chroma_decompression, separation};
use crate::curves::CurveSet;
use crate::date::Date;
use crate::film::Film;
use crate::frame::RgbPasses;
use crate::json;
use crate::legacy;
use crate::lighting_catch_up;
use crate::modes::{ConversionMode, Medal, ScannerMachine};
use crate::scanner::ScannerLook;
use crate::settings::PipelineSettings;
use crate::sidecar::{ExifMetadata, RollDraft};
use crate::tolerant::{self, Object};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::sync::OnceLock;

/// Format version, bumped only for a field whose meaning changes, never for one added.
pub const VERSION: i64 = 19;

/// Long side the fraction scale was calibrated on, hence what a stored fraction meant.
pub const RADIUS_REFERENCE_SIDE: f64 = 8368.0;

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

/// The document a sidecar holds; the version stays out of the settings' equality.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingsDocument {
    pub version: i64,
    #[serde(rename = "importedAt", skip_serializing_if = "Option::is_none", default)]
    pub imported_at: Option<Date>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub label: Option<Medal>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub exif: Option<ExifMetadata>,
    #[serde(rename = "rollDraft", skip_serializing_if = "Option::is_none", default)]
    pub roll_draft: Option<RollDraft>,
    #[serde(rename = "trashedAt", skip_serializing_if = "Option::is_none", default)]
    pub trashed_at: Option<Date>,
    pub settings: PipelineSettings,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    NotAnObject,
    Malformed(String),
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::NotAnObject => f.write_str("The settings file is not a JSON object."),
            Failure::Malformed(why) => write!(f, "Unreadable settings: {why}"),
        }
    }
}

impl std::error::Error for Failure {}

/// A decode's result; the version is returned as stored, the caller deciding about a newer one.
#[derive(Debug, Clone, PartialEq)]
pub struct Decoded {
    pub settings: PipelineSettings,
    pub version: i64,
    pub imported_at: Option<Date>,
    pub label: Medal,
    pub exif: Option<ExifMetadata>,
    pub roll_draft: Option<RollDraft>,
    pub trashed_at: Option<Date>,
}

impl Decoded {
    pub fn is_from_future(&self) -> bool {
        self.version > VERSION
    }
}

/// What a document says around its settings, for writing them back and for the library.
#[derive(Debug, Clone, PartialEq)]
pub struct Envelope {
    pub imported_at: Option<Date>,
    pub label: Medal,
    pub exif: Option<ExifMetadata>,
    pub roll_draft: Option<RollDraft>,
    pub trashed_at: Option<Date>,
    pub film: Option<Film>,
    pub manual_order: Option<f64>,
    pub mode: ConversionMode,
    pub pass_of: Option<String>,
    pub leads_passes: bool,
}

/// The document as the sidecar's text, sorted and indented so it reads and diffs by hand.
pub fn encode(
    settings: &PipelineSettings,
    imported_at: Option<Date>,
    label: Option<Medal>,
    exif: Option<ExifMetadata>,
    roll_draft: Option<RollDraft>,
    trashed_at: Option<Date>,
) -> Result<String, json::Error> {
    // An empty box is no EXIF at all, so an untouched frame's sidecar does not grow empty strings.
    let exif = exif.filter(|e| !e.is_empty());
    let document = SettingsDocument {
        version: VERSION,
        imported_at,
        label,
        exif,
        roll_draft,
        trashed_at,
        settings: settings.clone(),
    };
    Ok(json::pretty_sorted(&json::to_node(&document)?))
}

pub fn encode_settings(settings: &PipelineSettings) -> Result<String, json::Error> {
    encode(settings, None, None, None, None, None)
}

/// The resting document every stored one is laid over, built once.
pub fn neutral_document() -> &'static Value {
    static NEUTRAL: OnceLock<Value> = OnceLock::new();
    NEUTRAL.get_or_init(|| {
        let document = SettingsDocument {
            version: VERSION,
            imported_at: None,
            label: None,
            exif: None,
            roll_draft: None,
            trashed_at: None,
            settings: PipelineSettings::default(),
        };
        json::to_node(&document).map(|n| n.to_value()).unwrap_or(Value::Null)
    })
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

/// The envelope alone, its settings left undecoded; `None` on a document that is not JSON.
pub fn envelope(data: &[u8]) -> Option<Envelope> {
    let value: Value = serde_json::from_slice(data).ok()?;
    let map = value.as_object()?;
    fn read<T: serde::de::DeserializeOwned>(map: &Object, key: &str) -> Option<T> {
        tolerant::opt::<T, serde_json::Error>(map, key).ok().flatten()
    }
    let facts = map.get("settings").and_then(Value::as_object);
    let fact = |key: &str| facts.and_then(|f| f.get(key)).filter(|v| !v.is_null()).cloned();
    Some(Envelope {
        imported_at: read(map, "importedAt"),
        label: read(map, "label").unwrap_or_default(),
        exif: read(map, "exif"),
        roll_draft: read(map, "rollDraft"),
        trashed_at: read(map, "trashedAt"),
        film: fact("film").and_then(|v| serde_json::from_value(v).ok()),
        manual_order: fact("manualOrder").and_then(|v| v.as_f64()),
        mode: fact("mode").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default(),
        pass_of: fact("passOf").and_then(|v| v.as_str().map(str::to_owned)),
        leads_passes: fact("rgbPasses").and_then(|v| serde_json::from_value::<RgbPasses>(v).ok()).is_some(),
    })
}

/// What a decode knows of the file beside the sidecar, asked only when a migration needs it.
pub struct Source<'a> {
    /// The file decodes through the RAW engine's mosaic; shifts gains only for a photo whose decode moved.
    pub mosaic: &'a dyn Fn() -> bool,
    /// The file is read outside the RAW engine.
    pub plain: &'a dyn Fn() -> bool,
}

impl Source<'static> {
    /// Neither: what a preset or a paste is decoded as.
    pub fn none() -> Source<'static> {
        Source { mosaic: &|| false, plain: &|| false }
    }
}

pub fn decode(data: &[u8]) -> Result<Decoded, Failure> {
    decode_with(data, &Source::none(), true)
}

/// `catching_up` off reads metadata without fitting a former document; never write those settings back.
pub fn decode_with(data: &[u8], source: &Source<'_>, catching_up: bool) -> Result<Decoded, Failure> {
    let stored: Value = serde_json::from_slice(data).map_err(|_| Failure::NotAnObject)?;
    if !stored.is_object() {
        return Err(Failure::NotAnObject);
    }
    decode_value(stored, source, catching_up)
}

pub fn decode_value(stored: Value, source: &Source<'_>, catching_up: bool) -> Result<Decoded, Failure> {
    let Value::Object(stored) = stored else { return Err(Failure::NotAnObject) };
    let version = tolerant::ns_integer(stored.get("version")).unwrap_or(0);
    // The unit changes run on the keys the document really carries, before the defaults are laid in.
    let mut aged = if version < legacy::FIRST_RELEASED_VERSION {
        legacy::from_before_1_0(stored, version, source.mosaic)
    } else {
        stored
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
    if version < PRIMARIES_HELD_VERSION && (source.plain)() {
        aged = keeping_converted_primaries(aged);
    }
    if version < SCANNER_HALF_STEP_VERSION {
        aged = on_half_steps(aged);
    }
    if version < ONI_MACHINE_VERSION {
        aged = on_frontier(aged);
    }
    let aged = Value::Object(aged);
    let completed = merged(&aged, neutral_document());
    let document: SettingsDocument =
        serde_json::from_value(completed).map_err(|e| Failure::Malformed(e.to_string()))?;
    let mut settings = document.settings;
    let raw = aged.get("settings").filter(|v| v.is_object());
    let colour = CurveSet::retired(raw, "curves");
    if catching_up && version < legacy::LIGHTING_VERSION {
        let empty = Value::Object(Object::new());
        if let Some(former) = lighting_catch_up::former(raw.unwrap_or(&empty)) {
            settings = lighting_catch_up::migrated(&settings, &former, &colour.clone().unwrap_or_default());
        }
    }
    if let Some(colour) = &colour {
        settings.manual_curves = settings.manual_curves.folding(colour);
    }
    if let Some(scanner) = CurveSet::retired(raw.and_then(|r| r.get("scannerFinish")), "curves") {
        settings.scanner_finish.manual_curves = settings.scanner_finish.manual_curves.folding(&scanner);
    }
    if version < LOOK_FOLDED_VERSION
        && let Some(look) = ScannerLook::stored(raw)
    {
        settings = look.folded(&settings);
    }
    Ok(Decoded {
        settings,
        version,
        imported_at: document.imported_at,
        label: document.label.unwrap_or_default(),
        exif: document.exif,
        roll_draft: document.roll_draft,
        trashed_at: document.trashed_at,
    })
}

// MARK: - The migrations every released version may still need

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
