//! The sidecar: one JSON per photo, next to the original, and the only source of truth. It reads
//! and edits by hand, and survives a folder move because it travels with the original.

use crate::cg::Rect;
use crate::codec::{self, Decoded, Failure, Source};
use crate::date::Date;
use crate::frame::{FlatField, PassOrder};
use crate::id::Id;
use crate::modes::{ConversionMode, Medal};
use crate::settings::PipelineSettings;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) use crate::atomic::write_atomically;
use std::time::{Duration, SystemTime};

mod fields;
mod held;
pub mod identity;

pub use fields::{set_exif, set_label, set_roll_draft, set_trashed, update};

/// The EXIF a hand set for the export; every key is required, as the document writes them all.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ExifMetadata {
    pub camera: String,
    pub lens: String,
    pub film: String,
    pub iso: String,
    /// EXIF form, `YYYY:MM:DD HH:MM:SS`.
    pub date: String,
    pub artist: String,
    pub copyright: String,
}

impl ExifMetadata {
    pub fn is_empty(&self) -> bool {
        *self == ExifMetadata::default()
    }
}

/// A roll being prepared, written with each of its frames so a preparation left halfway resumes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RollDraft {
    pub id: Id,
    /// The step reached, by its name, so a step added later moves no stored one.
    pub step: String,
    pub film: ConversionMode,
    pub capture: String,
    #[serde(rename = "passOrder", skip_serializing_if = "Option::is_none", default)]
    pub pass_order: Option<PassOrder>,
    #[serde(rename = "flatField", skip_serializing_if = "Option::is_none", default)]
    pub flat_field: Option<FlatField>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub base: Option<DraftBase>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub window: Option<Rect>,
    pub ticked: bool,
    pub flagged: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftBase {
    pub path: String,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
}

/// Delay after the last slider movement, coalescing writes instead of one per frame.
pub const WRITE_DELAY: Duration = Duration::from_millis(400);

/// The suffix goes after the extension, so two originals sharing a base name never share a sidecar.
pub fn url_for(original: &Path) -> PathBuf {
    let mut path = original.as_os_str().to_owned();
    path.push(".json");
    PathBuf::from(path)
}

/// What a read learns about a photo.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    pub settings: PipelineSettings,
    /// False for a sidecar from a newer format: the photo opens read-only, never rewritten.
    pub writable: bool,
    pub is_new: bool,
    pub imported_at: Option<Date>,
    pub label: Medal,
    pub exif: Option<ExifMetadata>,
    pub roll_draft: Option<RollDraft>,
    pub trashed_at: Option<Date>,
    /// The sidecar beside it was written for another file of that name: read as none, and set aside
    /// at the first write.
    pub foreign: bool,
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Codec(Failure),
    Encode(crate::json::Error),
    /// The original is not on disk, its disk away or the file moved: nothing is written beside nothing.
    Unreachable,
    /// The sidecar on disk is from a newer format, which this version never rewrites.
    Newer,
    /// The photo's name and `.json` pass the 255 characters a file system allows a name.
    TooLong,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Codec(e) => write!(f, "{e}"),
            Error::Encode(e) => write!(f, "{e}"),
            Error::Unreachable => f.write_str("The original cannot be found; its disk may be disconnected. Nothing was saved."),
            Error::Newer => f.write_str("A newer version of ONI wrote these settings, and this one never rewrites them."),
            Error::TooLong => f.write_str("The photo's name is too long for a settings file beside it. Shorten the name to keep its edits."),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<Failure> for Error {
    fn from(e: Failure) -> Self {
        Error::Codec(e)
    }
}

impl From<crate::json::Error> for Error {
    fn from(e: crate::json::Error) -> Self {
        Error::Encode(e)
    }
}

/// The guard of every write: a sidecar beside a missing original would be adopted by a later namesake.
pub fn original_present(original: &Path) -> Result<(), Error> {
    match original.try_exists() {
        Ok(true) => Ok(()),
        Ok(false) => Err(Error::Unreachable),
        Err(e) => Err(Error::Io(e)),
    }
}

/// Reads an original's sidecar; `mosaic` says the original decodes through the RAW engine's mosaic.
/// Absent and unreadable stay apart: an unreadable sidecar is an error, never a new photo.
pub fn read(original: &Path, mosaic: bool, catching_up: bool) -> Result<Loaded, Error> {
    let path = url_for(original);
    let fresh = |foreign: bool| Loaded {
        settings: PipelineSettings::rest(ConversionMode::Negative, mosaic),
        writable: true,
        is_new: true,
        imported_at: None,
        label: Medal::None,
        exif: None,
        roll_draft: None,
        trashed_at: None,
        foreign,
    };
    if !path.exists() {
        return Ok(fresh(false));
    }
    let data = std::fs::read(&path)?;
    if let Ok(value) = serde_json::from_slice::<Value>(&data)
        && value.get(identity::KEY).is_some()
        && identity::foreign(&value, identity::of(original).as_deref(), || identity::tail(original))
    {
        return Ok(fresh(true));
    }
    let source = Source { mosaic: &|| mosaic, plain: &|| !mosaic };
    let decoded = codec::decode_with(&data, &source, catching_up)?;
    Ok(Loaded {
        writable: !decoded.is_from_future(),
        is_new: false,
        settings: decoded.settings,
        imported_at: decoded.imported_at,
        label: decoded.label,
        exif: decoded.exif,
        roll_draft: decoded.roll_draft,
        trashed_at: decoded.trashed_at,
        foreign: false,
    })
}

/// What a write sets around the settings; `None` keeps what the file already says.
#[derive(Debug, Clone, Default)]
pub struct WriteOptions {
    pub imported_at: Option<Date>,
    pub label: Option<Medal>,
    pub exif: Option<ExifMetadata>,
    /// `Some(None)` clears the draft on file.
    pub roll_draft: Option<Option<RollDraft>>,
    /// `Some(None)` clears the ONI Trash mark.
    pub trashed_at: Option<Option<Date>>,
    /// Keeps the modification date, so the thumbnail key holds.
    pub preserving_revision: bool,
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn restore_modified(path: &Path, time: Option<SystemTime>) {
    if let Some(time) = time
        && let Ok(file) = std::fs::OpenOptions::new().write(true).open(path)
    {
        let _ = file.set_modified(time);
    }
}

/// The sidecar as it stands when a write comes, `None` when absent. One from a newer format, or that
/// this version cannot read, is refused: its fields would be lost under defaults.
fn standing(path: &Path) -> Result<Option<(Value, Decoded)>, Error> {
    let data = match std::fs::read(path) {
        Ok(data) => data,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let value: Value = serde_json::from_slice(&data).map_err(|_| Failure::NotAnObject)?;
    if crate::tolerant::ns_integer(value.get("version")).unwrap_or(0) > codec::VERSION {
        return Err(Error::Newer);
    }
    let decoded = codec::decode_value(value.clone(), &Source::none(), false)?;
    Ok(Some((value, decoded)))
}

/// Writes the sidecar atomically, reading back the metadata the caller left out and keeping the
/// keys a newer app added; never beside a missing original, nor over a newer or unreadable sidecar.
pub fn write(settings: &PipelineSettings, original: &Path, options: WriteOptions) -> Result<(), Error> {
    let _held = held::hold(original);
    written(settings, original, options)
}

/// `write`, the sidecar already held.
fn written(settings: &PipelineSettings, original: &Path, options: WriteOptions) -> Result<(), Error> {
    original_present(original)?;
    let path = url_for(original);
    if path.file_name().is_some_and(|n| n.to_string_lossy().chars().count() > 255) {
        return Err(Error::TooLong);
    }
    let (print, tail) = (identity::of(original), identity::tail(original));
    // Another file's settings are kept aside whole, never written over nor taken for this one's.
    let aside = identity::stored(&path)?.is_some_and(|v| identity::foreign(&v, print.as_deref(), || tail.clone()));
    if aside {
        identity::set_aside(&path)?;
    }
    let prior = if options.preserving_revision && !aside { modified(&path) } else { None };
    let standing = standing(&path)?;
    let existing = standing.as_ref().map(|(_, decoded)| decoded);
    let document = codec::document(
        settings,
        options.imported_at.or(existing.and_then(|e| e.imported_at)),
        options.label.or(existing.map(|e| e.label)),
        options.exif.or(existing.and_then(|e| e.exif.clone())),
        options.roll_draft.unwrap_or_else(|| existing.and_then(|e| e.roll_draft.clone())),
        options.trashed_at.unwrap_or_else(|| existing.and_then(|e| e.trashed_at)),
    );
    let own: Vec<(&str, &str)> = [(identity::KEY, print.as_deref()), (identity::TAIL, tail.as_deref())].into_iter().filter_map(|(k, v)| Some((k, v?))).collect();
    let text = crate::kept::encoded(&document, standing.as_ref(), &own)?;
    write_atomically(&path, text.as_bytes())?;
    restore_modified(&path, prior);
    Ok(())
}

/// Deletes only the sidecar, never the original beside it; an absent sidecar is not an error.
pub fn remove(original: &Path) -> io::Result<()> {
    let sidecar = url_for(original);
    match std::fs::remove_file(&sidecar) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Seeds no neutral sidecar beside an untouched photo, but writes a return to neutral over one.
pub fn should_write(settings: &PipelineSettings, original: &Path) -> bool {
    *settings != PipelineSettings::rest(ConversionMode::Negative, settings.decoded_from_mosaic)
        || url_for(original).exists()
}

/// Writes when there is something to write; the message of a failure, `None` otherwise.
pub fn flush(settings: &PipelineSettings, original: Option<&Path>, persisted: Option<&PipelineSettings>, writable: bool) -> Option<String> {
    let original = original?;
    if !writable || Some(settings) == persisted || !should_write(settings, original) {
        return None;
    }
    write(settings, original, WriteOptions::default()).err().map(|e| e.to_string())
}
