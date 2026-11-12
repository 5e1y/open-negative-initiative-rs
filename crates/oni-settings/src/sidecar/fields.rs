//! The field setters: each reads the sidecar, changes one field and writes it back, the sidecar
//! held from the read to the rename.

use super::{Error, ExifLine, WriteOptions, held, original_present, read, written};
use crate::date::Date;
use crate::modes::Medal;
use crate::settings::PipelineSettings;
use std::path::Path;

/// Reads, transforms and writes back; false on a sidecar kept read-only and when nothing changed.
pub fn update(original: &Path, mosaic: bool, transform: impl FnOnce(&PipelineSettings) -> PipelineSettings) -> Result<bool, Error> {
    let _held = held::hold(original);
    let loaded = read(original, mosaic)?;
    if !loaded.writable {
        return Ok(false);
    }
    let next = transform(&loaded.settings);
    if next == loaded.settings {
        return Ok(false);
    }
    written(&next, original, WriteOptions::default())?;
    Ok(true)
}

/// Sets a label without touching the settings or the revision.
pub fn set_label(label: Medal, original: &Path, mosaic: bool) -> Result<(), Error> {
    let _held = held::hold(original);
    original_present(original)?;
    // An unreadable sidecar is an error, never rewritten as defaults around the one field.
    let existing = read(original, mosaic)?;
    existing.standing()?;
    let settings = existing.settings.clone();
    written(
        &settings,
        original,
        WriteOptions {
            imported_at: existing.imported_at,
            label: Some(label),
            preserving_revision: true,
            ..WriteOptions::default()
        },
    )
}

/// Sets the frame's EXIF, the settings and the revision untouched; a frame never touched gains no
/// sidecar for an empty line.
pub fn set_exif(exif: ExifLine, original: &Path, mosaic: bool) -> Result<(), Error> {
    let _held = held::hold(original);
    original_present(original)?;
    // An unreadable sidecar is an error, never rewritten as defaults around the one field.
    let existing = read(original, mosaic)?;
    existing.standing()?;
    // The same line, an empty one beside no sidecar included, writes nothing.
    if existing.exif == exif {
        return Ok(());
    }
    let settings = existing.settings.clone();
    written(
        &settings,
        original,
        WriteOptions {
            imported_at: existing.imported_at,
            label: Some(existing.label),
            exif: Some(exif),
            preserving_revision: true,
            ..WriteOptions::default()
        },
    )
}

/// Marks the photo in the ONI Trash, or back on `None`; false on a newer format, an unreadable one or
/// one held back an error, and a missing original `Unreachable` before its absent sidecar could read as a fresh photo.
pub fn set_trashed(date: Option<Date>, original: &Path, mosaic: bool) -> Result<bool, Error> {
    let _held = held::hold(original);
    original_present(original)?;
    let existing = read(original, mosaic)?;
    if let Some(why) = existing.held_back {
        return Err(Error::Held(why));
    }
    if !existing.writable {
        return Ok(false);
    }
    if existing.trashed_at == date {
        return Ok(true);
    }
    let options = WriteOptions { trashed_at: Some(date), preserving_revision: true, ..WriteOptions::default() };
    written(&existing.settings, original, options)?;
    Ok(true)
}
