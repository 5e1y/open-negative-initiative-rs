//! The sidecar: one JSON per photo, next to the original, and the only source of truth. It reads
//! and edits by hand, and survives a folder move because it travels with the original.

use crate::document::{self, AutoBalance, Document, Failure, Original};
use crate::date::Date;
use crate::migrate::{self, Refusal, Standing};
use crate::modes::{ConversionMode, Medal};
use crate::settings::PipelineSettings;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) use crate::atomic::write_atomically;
use std::time::{Duration, SystemTime};

mod exif_line;
mod fields;
pub(crate) mod held;
pub mod identity;
mod said;

pub use exif_line::{ExifLine, ExifMetadata};
pub use fields::{set_exif, set_label, set_trashed, update};

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
    /// False for a sidecar from a newer format, or one of an earlier format kept as it is
    /// (`held_back`): the photo opens read-only, never rewritten.
    pub writable: bool,
    pub is_new: bool,
    pub imported_at: Option<Date>,
    pub label: Medal,
    pub exif: ExifLine,
    pub trashed_at: Option<Date>,
    pub auto_balance: Option<AutoBalance>,
    /// Why a sidecar of an earlier format could not be brought to this one; shown in memory meanwhile.
    pub held_back: Option<Refusal>,
    /// The sidecar beside it was written for another file of that name: read as none, and set aside
    /// at the first write.
    pub foreign: bool,
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Codec(Failure),
    /// The original is not on disk, its disk away or the file moved: nothing is written beside nothing.
    Unreachable,
    /// The sidecar on disk is from a newer format, which this version never rewrites.
    Newer,
    /// The sidecar on disk is of an earlier format that could not be brought to this one.
    Held(Refusal),
    /// The photo's name and `.json` pass the 255 characters a file system allows a name.
    TooLong,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Codec(e) => write!(f, "{e}"),
            Error::Unreachable => f.write_str("The original cannot be found; its disk may be disconnected. Nothing was saved."),
            Error::Newer => f.write_str("A newer version of ONI wrote these settings, and this one never rewrites them."),
            Error::Held(why) => write!(f, "These settings are in an earlier format and were kept as they are: {why}."),
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

/// The guard of every write: a sidecar beside a missing original would be adopted by a later namesake.
pub fn original_present(original: &Path) -> Result<(), Error> {
    match original.try_exists() {
        Ok(true) => Ok(()),
        Ok(false) => Err(Error::Unreachable),
        Err(e) => Err(Error::Io(e)),
    }
}

fn fresh(mosaic: bool, foreign: bool) -> Loaded {
    Loaded {
        settings: PipelineSettings::rest(ConversionMode::Negative, mosaic),
        writable: true,
        is_new: true,
        imported_at: None,
        label: Medal::None,
        exif: ExifLine::default(),
        trashed_at: None,
        auto_balance: None,
        held_back: None,
        foreign,
    }
}

/// Reads an original's sidecar (`mosaic`: it decodes through the RAW engine's mosaic), writing nothing:
/// unreadable is an error, never a new photo; an earlier format read-only while a fact it asks is missing.
pub fn read(original: &Path, mosaic: bool) -> Result<Loaded, Error> {
    let Some(bytes) = stored(original)? else { return Ok(fresh(mosaic, false)) };
    if identity::foreign_bytes(&bytes, original) {
        return Ok(fresh(mosaic, true));
    }
    standing(&bytes, original, mosaic)
}

/// A sidecar's `bytes` read, written for this original.
fn standing(bytes: &[u8], original: &Path, mosaic: bool) -> Result<Loaded, Error> {
    match migrate::standing(bytes) {
        Standing::Current => Ok(Loaded::of(document::decode(bytes)?, true, None)),
        Standing::NotASidecar => Err(Error::Codec(Failure::NotAnObject)),
        Standing::Newer(_) => {
            let (c, _) = migrate::asked(original, Some(mosaic), |facts| migrate::reading(bytes, facts, true)).map_err(refused)?;
            Ok(Loaded::of(c.document, false, None))
        }
        Standing::Older(_) => {
            let (c, missing) = migrate::asked(original, Some(mosaic), |facts| migrate::reading(bytes, facts, false)).map_err(refused)?;
            Ok(Loaded::of(c.document, missing.is_none(), missing))
        }
    }
}

fn refused(why: Refusal) -> Error {
    match why {
        Refusal::Unreadable(e) => Error::Codec(e),
        Refusal::Newer(_) => Error::Newer,
        other => Error::Held(other),
    }
}

/// The sidecar's bytes, `None` when absent; a name too long for the file system names none.
fn stored(original: &Path) -> Result<Option<Vec<u8>>, Error> {
    match std::fs::read(url_for(original)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if matches!(e.kind(), io::ErrorKind::NotFound | io::ErrorKind::InvalidFilename) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

impl Loaded {
    /// A document as a read gives it.
    pub fn of(document: Document, writable: bool, held_back: Option<Refusal>) -> Loaded {
        Loaded {
            writable: writable && document.version <= document::VERSION,
            is_new: false,
            settings: document.settings,
            imported_at: document.imported_at,
            label: document.label,
            exif: document.exif_line,
            trashed_at: document.trashed_at,
            auto_balance: document.auto_balance,
            held_back,
            foreign: false,
        }
    }

    /// Writable, or the reason it is not.
    pub fn standing(&self) -> Result<(), Error> {
        match (&self.held_back, self.writable) {
            (_, true) => Ok(()),
            (Some(why), false) => Err(Error::Held(why.clone())),
            (None, false) => Err(Error::Newer),
        }
    }
}

/// What a write sets around the settings; `None` keeps what the file already says.
#[derive(Debug, Clone, Default)]
pub struct WriteOptions {
    pub imported_at: Option<Date>,
    pub label: Option<Medal>,
    pub exif: Option<ExifLine>,
    /// `Some(None)` clears the ONI Trash mark.
    pub trashed_at: Option<Option<Date>>,
    /// `Some(None)` drops a balance kept.
    pub auto_balance: Option<Option<AutoBalance>>,
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

/// Writes the sidecar atomically, reading back what the caller left out; never beside a missing
/// original, nor over a newer, unreadable or held-back sidecar.
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
    let mut bytes = stored(original)?;
    // Another file's settings are kept aside whole, never written over nor taken for this one's.
    let aside = bytes.as_ref().is_some_and(|b| identity::foreign_with(b, print.as_deref(), || tail.clone()));
    if aside {
        identity::set_aside(&path)?;
        bytes = None;
    }
    // A write brings an earlier file to this format first, its roll draft kept in its folder's file;
    // the settings it lays are its own, so a camera matrix it cannot learn holds nothing back.
    let mut whole = false;
    if let Some(b) = bytes.clone().filter(|b| matches!(migrate::standing(b), Standing::Older(_))) {
        match migrate::file(original, Some(settings.decoded_from_mosaic)) {
            Ok(_) => bytes = stored(original)?,
            Err(Refusal::MatrixUnknown | Refusal::CameraUnknown) => {
                migrate::draft::kept_from(original, &b).map_err(|e| Error::Held(Refusal::Drafts(e.to_string())))?;
                whole = true;
            }
            Err(why) => return Err(refused(why)),
        }
    }
    let prior = if options.preserving_revision && !aside { modified(&path) } else { None };
    let existing = match &bytes {
        Some(b) => {
            let loaded = standing(b, original, settings.decoded_from_mosaic)?;
            if !whole {
                loaded.standing()?;
            }
            Some(loaded)
        }
        None => None,
    };
    let existing = existing.as_ref();
    let document = Document {
        original: match print {
            Some(print) => Some(Original { print, tail }),
            None => bytes.as_deref().and_then(identity::original_in),
        },
        imported_at: options.imported_at.or(existing.and_then(|e| e.imported_at)),
        label: options.label.or(existing.map(|e| e.label)).unwrap_or_default(),
        trashed_at: options.trashed_at.unwrap_or_else(|| existing.and_then(|e| e.trashed_at)),
        exif_line: options.exif.or_else(|| existing.map(|e| e.exif.clone())).unwrap_or_default(),
        auto_balance: options.auto_balance.unwrap_or_else(|| existing.and_then(|e| e.auto_balance.clone())),
        ..Document::of(settings.clone())
    };
    write_atomically(&path, &document::encode(&document)?)?;
    restore_modified(&path, prior);
    Ok(())
}

/// The balance kept with the frame; `None` when absent, unreadable or written for another file.
pub fn auto_balance(original: &Path, mosaic: bool) -> Option<AutoBalance> {
    read(original, mosaic).ok().filter(|l| !l.foreign)?.auto_balance
}

/// Keeps `balance` with the frame, the settings and the revision as they stand; only in a sidecar
/// written for this file, never seeding one. Whether it wrote.
pub fn set_auto_balance(original: &Path, mosaic: bool, balance: &AutoBalance) -> Result<bool, Error> {
    let _held = held::hold(original);
    original_present(original)?;
    let existing = read(original, mosaic)?;
    if !existing.writable || existing.foreign || existing.is_new || existing.auto_balance.as_ref() == Some(balance) {
        return Ok(false);
    }
    let options = WriteOptions { auto_balance: Some(Some(balance.clone())), preserving_revision: true, ..WriteOptions::default() };
    written(&existing.settings, original, options)?;
    Ok(true)
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
