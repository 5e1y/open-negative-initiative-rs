//! The sidecar: one JSON per photo, next to the original, and the only source of truth. It reads
//! and edits by hand, and survives a folder move because it travels with the original.

use crate::cg::Rect;
use crate::codec::{self, Failure, Source};
use crate::date::Date;
use crate::frame::{FlatField, PassOrder};
use crate::id::Id;
use crate::modes::{ConversionMode, Medal};
use crate::settings::PipelineSettings;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Codec(Failure),
    Encode(crate::json::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Codec(e) => write!(f, "{e}"),
            Error::Encode(e) => write!(f, "{e}"),
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

/// Reads an original's sidecar; `mosaic` says the original decodes through the RAW engine's mosaic.
/// Absent and unreadable stay apart: an unreadable sidecar is an error, never a new photo.
pub fn read(original: &Path, mosaic: bool, catching_up: bool) -> Result<Loaded, Error> {
    let path = url_for(original);
    if !path.exists() {
        return Ok(Loaded {
            settings: PipelineSettings::rest(ConversionMode::Negative, mosaic),
            writable: true,
            is_new: true,
            imported_at: None,
            label: Medal::None,
            exif: None,
            roll_draft: None,
            trashed_at: None,
        });
    }
    let data = std::fs::read(&path)?;
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

/// Writes the sidecar atomically, reading back the metadata the caller left out.
pub fn write(settings: &PipelineSettings, original: &Path, options: WriteOptions) -> Result<(), Error> {
    let path = url_for(original);
    let prior = if options.preserving_revision { modified(&path) } else { None };
    let existing = std::fs::read(&path).ok().and_then(|d| codec::envelope(&d));
    let text = codec::encode(
        settings,
        options.imported_at.or(existing.as_ref().and_then(|e| e.imported_at)),
        options.label.or(existing.as_ref().map(|e| e.label)),
        options.exif.or(existing.as_ref().and_then(|e| e.exif.clone())),
        match options.roll_draft {
            Some(draft) => draft,
            None => existing.as_ref().and_then(|e| e.roll_draft.clone()),
        },
        existing.as_ref().and_then(|e| e.trashed_at),
    )?;
    write_atomically(&path, text.as_bytes())?;
    restore_modified(&path, prior);
    Ok(())
}

/// Reads, transforms and writes back; false on a newer format and when nothing changed.
pub fn update(original: &Path, mosaic: bool, transform: impl FnOnce(&PipelineSettings) -> PipelineSettings) -> Result<bool, Error> {
    let loaded = read(original, mosaic, true)?;
    if !loaded.writable {
        return Ok(false);
    }
    let next = transform(&loaded.settings);
    if next == loaded.settings {
        return Ok(false);
    }
    write(&next, original, WriteOptions::default())?;
    Ok(true)
}

/// Sets a label without touching the settings or the revision.
pub fn set_label(label: Medal, original: &Path, mosaic: bool) -> Result<(), Error> {
    let existing = read(original, mosaic, true).ok();
    if existing.as_ref().is_some_and(|e| !e.writable) {
        return Ok(());
    }
    let settings = existing.as_ref().map(|e| e.settings.clone()).unwrap_or_default();
    write(
        &settings,
        original,
        WriteOptions {
            imported_at: existing.as_ref().and_then(|e| e.imported_at),
            label: Some(label),
            preserving_revision: true,
            ..WriteOptions::default()
        },
    )
}

/// Sets the frame's EXIF, the settings and the revision untouched.
pub fn set_exif(exif: ExifMetadata, original: &Path, mosaic: bool) -> Result<(), Error> {
    let existing = read(original, mosaic, true).ok();
    if existing.as_ref().is_some_and(|e| !e.writable) {
        return Ok(());
    }
    let settings = existing.as_ref().map(|e| e.settings.clone()).unwrap_or_default();
    write(
        &settings,
        original,
        WriteOptions {
            imported_at: existing.as_ref().and_then(|e| e.imported_at),
            label: existing.as_ref().map(|e| e.label),
            exif: Some(exif),
            preserving_revision: true,
            ..WriteOptions::default()
        },
    )
}

/// Sets or clears the roll a frame is prepared in; a frame never touched gains no sidecar for a clear.
pub fn set_roll_draft(draft: Option<RollDraft>, original: &Path, mosaic: bool) -> Result<(), Error> {
    let existing = read(original, mosaic, true).ok();
    if existing.as_ref().is_some_and(|e| !e.writable) || existing.as_ref().and_then(|e| e.roll_draft.as_ref()) == draft.as_ref()
    {
        return Ok(());
    }
    if draft.is_none() && existing.as_ref().map(|e| e.is_new) != Some(false) {
        return Ok(());
    }
    let settings = existing.as_ref().map(|e| e.settings.clone()).unwrap_or_default();
    write(
        &settings,
        original,
        WriteOptions {
            imported_at: existing.as_ref().and_then(|e| e.imported_at),
            label: existing.as_ref().map(|e| e.label),
            roll_draft: Some(draft),
            preserving_revision: true,
            ..WriteOptions::default()
        },
    )
}

/// Marks the photo in the ONI Trash, or back on `None`; false on a newer format, an unreadable one an error.
pub fn set_trashed(date: Option<Date>, original: &Path, mosaic: bool) -> Result<bool, Error> {
    // Caught up as every rewrite is: the document is stamped with today's version.
    let existing = read(original, mosaic, true)?;
    if !existing.writable {
        return Ok(false);
    }
    if existing.trashed_at == date {
        return Ok(true);
    }
    let path = url_for(original);
    let prior = modified(&path);
    let text = codec::encode(
        &existing.settings,
        existing.imported_at,
        Some(existing.label),
        existing.exif,
        existing.roll_draft,
        date,
    )?;
    write_atomically(&path, text.as_bytes())?;
    restore_modified(&path, prior);
    Ok(true)
}

/// Deletes only the sidecar, never the original beside it; an absent sidecar is not an error.
pub fn remove(original: &Path) -> io::Result<()> {
    match std::fs::remove_file(url_for(original)) {
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

/// A temporary file beside the target, then a rename: a reader never sees half a document.
pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let directory = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let temporary = directory.join(format!(".{name}.{}.{stamp}.tmp", std::process::id()));
    let result = (|| {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}
