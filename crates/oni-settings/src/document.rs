//! The sidecar's document, `oni-frame`: indented JSON written by `serde_json` straight from these
//! types, in their declaration order. A stored one of an earlier version is read by `migrate` alone.

use crate::date::Date;
use crate::film::Film;
use crate::frame::{RgbPasses, RollReference};
use crate::modes::{ConversionMode, Medal};
use crate::settings::PipelineSettings;
use crate::sidecar::ExifLine;
use crate::tolerant::{self, Object};
use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// What a sidecar names itself, so a walk tells one from another program's JSON.
pub const FORMAT: &str = "oni-frame";
/// Moved at every change of the document's shape, an added field included.
pub const VERSION: i64 = 22;

/// Long side the fraction scale was calibrated on, hence what a stored fraction meant.
pub const RADIUS_REFERENCE_SIDE: f64 = 8368.0;

/// A photo's sidecar: what the app says around its settings, then the settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Document {
    pub format: String,
    pub version: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original: Option<Original>,
    #[serde(rename = "importedAt", skip_serializing_if = "Option::is_none")]
    pub imported_at: Option<Date>,
    #[serde(skip_serializing_if = "is_none")]
    pub label: Medal,
    #[serde(rename = "trashedAt", skip_serializing_if = "Option::is_none")]
    pub trashed_at: Option<Date>,
    #[serde(rename = "exifLine", skip_serializing_if = "ExifLine::is_empty")]
    pub exif_line: ExifLine,
    #[serde(rename = "autoBalance", skip_serializing_if = "Option::is_none")]
    pub auto_balance: Option<AutoBalance>,
    pub settings: PipelineSettings,
}

fn is_none(medal: &Medal) -> bool {
    *medal == Medal::None
}

impl Default for Document {
    fn default() -> Self {
        Document::of(PipelineSettings::default())
    }
}

impl Document {
    /// `settings` with nothing said around them.
    pub fn of(settings: PipelineSettings) -> Document {
        Document {
            format: FORMAT.into(),
            version: VERSION,
            original: None,
            imported_at: None,
            label: Medal::None,
            trashed_at: None,
            exif_line: ExifLine::default(),
            auto_balance: None,
            settings,
        }
    }
}

/// The original the sidecar was written for: its print (the library's fingerprint) and the digest
/// of its last block, so a namesake copied in later never adopts its settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Original {
    pub print: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tail: Option<String>,
}

/// Auto White Balance kept with the frame, for the fingerprint of what it read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutoBalance {
    pub fingerprint: String,
    pub filters: [f32; 3],
    /// The roll the frame was read on; none for a slide, read as it stands.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reference: Option<RollReference>,
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

/// The document's text, a newline last.
pub fn encode(document: &Document) -> Result<Vec<u8>, Failure> {
    let mut text = serde_json::to_vec_pretty(document).map_err(|e| Failure::Malformed(e.to_string()))?;
    text.push(b'\n');
    Ok(text)
}

/// The text of `settings` with nothing around them: a paste's board, a check's fixture.
pub fn encode_settings(settings: &PipelineSettings) -> Result<String, Failure> {
    let text = encode(&Document::of(settings.clone()))?;
    String::from_utf8(text).map_err(|e| Failure::Malformed(e.to_string()))
}

/// A document of this version, typed in one pass; any other version is `migrate`'s.
pub fn decode(bytes: &[u8]) -> Result<Document, Failure> {
    let document = typed(bytes)?;
    if document.version != VERSION || document.format != FORMAT {
        return Err(Failure::Malformed(format!("not an {FORMAT} document of version {VERSION}")));
    }
    Ok(document)
}

/// The document as far as these types read it, whatever its version says.
pub(crate) fn typed(bytes: &[u8]) -> Result<Document, Failure> {
    serde_json::from_slice(bytes).map_err(|e| match serde_json::from_slice::<Value>(bytes) {
        Ok(Value::Object(_)) => Failure::Malformed(e.to_string()),
        _ => Failure::NotAnObject,
    })
}

/// What a sniff reads: the name and the version, every other key skipped unparsed.
#[derive(Deserialize)]
struct Sniff {
    #[serde(default)]
    format: Option<Value>,
    #[serde(default)]
    version: Option<Value>,
    #[serde(default)]
    settings: Option<IgnoredAny>,
}

/// What a sniff finds at the top of an object: its name, its version, whether it holds settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sniffed {
    pub named: bool,
    pub version: i64,
    pub settings: bool,
}

/// The top of a document alone; `None` for text that is not a JSON object. A version that is no
/// integer reads 0, as the oldest documents carried none.
pub fn sniffed(bytes: &[u8]) -> Option<Sniffed> {
    let sniff: Sniff = serde_json::from_slice(bytes).ok()?;
    let named = sniff.format.as_ref().and_then(Value::as_str) == Some(FORMAT);
    Some(Sniffed { named, version: tolerant::ns_integer(sniff.version.as_ref()).unwrap_or(0), settings: sniff.settings.is_some() })
}

/// The version alone, the rest unread; `None` for text that is not a JSON object.
pub fn version_of(bytes: &[u8]) -> Option<i64> {
    sniffed(bytes).map(|s| s.version)
}

/// What a document says around its settings, for the library.
#[derive(Debug, Clone, PartialEq)]
pub struct Envelope {
    pub imported_at: Option<Date>,
    pub label: Medal,
    pub trashed_at: Option<Date>,
    pub film: Option<Film>,
    pub manual_order: Option<f64>,
    pub mode: ConversionMode,
    pub pass_of: Option<String>,
    pub leads_passes: bool,
    /// The file is a contact sheet laid from a roll's frames, never one of them.
    pub sheet: bool,
}

/// The envelope alone, its settings left undecoded; `None` on a document that is not JSON. It reads
/// only keys every version keeps where they were, so it answers alike on an earlier document.
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
        trashed_at: read(map, "trashedAt"),
        film: fact("film").and_then(|v| serde_json::from_value(v).ok()),
        manual_order: fact("manualOrder").and_then(|v| v.as_f64()),
        mode: fact("mode").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default(),
        pass_of: fact("passOf").and_then(|v| v.as_str().map(str::to_owned)),
        leads_passes: fact("rgbPasses").and_then(|v| serde_json::from_value::<RgbPasses>(v).ok()).is_some(),
        sheet: fact("sheet").is_some(),
    })
}
