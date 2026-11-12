//! The sidecar's document: its envelope, its settings, and the text a write lays; reading a stored
//! one, of whatever version, is `migrate`'s.

use crate::date::Date;
use crate::film::Film;
use crate::frame::RgbPasses;
use crate::json;
use crate::modes::{ConversionMode, Medal};
use crate::settings::PipelineSettings;
use crate::sidecar::{RollDraft, TypedExif};
use crate::tolerant::{self, Object};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// Format version, bumped only for a field whose meaning changes, never for one added.
pub const VERSION: i64 = 21;

/// Long side the fraction scale was calibrated on, hence what a stored fraction meant.
pub const RADIUS_REFERENCE_SIDE: f64 = 8368.0;

/// From it no migration consumes a document's keys, so a rewrite carries the ones it does not know.
pub const KEYS_CARRIED_FROM: i64 = crate::migrate::chain::LOOK_FOLDED_VERSION;

/// The document a sidecar holds; the version stays out of the settings' equality.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingsDocument {
    pub version: i64,
    #[serde(rename = "importedAt", skip_serializing_if = "Option::is_none", default)]
    pub imported_at: Option<Date>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub label: Option<Medal>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub exif: Option<TypedExif>,
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
    pub exif: Option<TypedExif>,
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
    pub exif: Option<TypedExif>,
    pub roll_draft: Option<RollDraft>,
    pub trashed_at: Option<Date>,
    pub film: Option<Film>,
    pub manual_order: Option<f64>,
    pub mode: ConversionMode,
    pub pass_of: Option<String>,
    pub leads_passes: bool,
    /// The file is a contact sheet laid from a roll's frames, never one of them.
    pub sheet: bool,
}

/// The document as the sidecar's text, sorted and indented so it reads and diffs by hand.
pub fn encode(
    settings: &PipelineSettings,
    imported_at: Option<Date>,
    label: Option<Medal>,
    exif: Option<TypedExif>,
    roll_draft: Option<RollDraft>,
    trashed_at: Option<Date>,
) -> Result<String, json::Error> {
    let document = document(settings, imported_at, label, exif, roll_draft, trashed_at);
    Ok(json::pretty_sorted(&json::to_node(&document)?))
}

/// The document `encode` writes, before its text.
pub(crate) fn document(
    settings: &PipelineSettings,
    imported_at: Option<Date>,
    label: Option<Medal>,
    exif: Option<TypedExif>,
    roll_draft: Option<RollDraft>,
    trashed_at: Option<Date>,
) -> SettingsDocument {
    // A line with no field set is no EXIF at all, so an untouched frame's sidecar holds no block.
    let exif = exif.filter(|e| !e.is_empty());
    SettingsDocument { version: VERSION, imported_at, label, exif, roll_draft, trashed_at, settings: settings.clone() }
}

pub fn encode_settings(settings: &PipelineSettings) -> Result<String, json::Error> {
    encode(settings, None, None, None, None, None)
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
        sheet: fact("sheet").is_some(),
    })
}
