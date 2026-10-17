//! What a file says about the photograph: only the fields the hand stated, never the scan's own.

use std::path::Path;

pub use oni_settings::ExifMetadata;

use crate::ifd::{Directory, Value};

pub mod tag {
    pub const IMAGE_DESCRIPTION: u16 = 270;
    pub const MODEL: u16 = 272;
    pub const DATE_TIME: u16 = 306;
    pub const ARTIST: u16 = 315;
    pub const COPYRIGHT: u16 = 33432;
    pub const EXIF_IFD: u16 = 34665;
    pub const ISO_SPEED_RATINGS: u16 = 34855;
    pub const DATE_TIME_ORIGINAL: u16 = 36867;
    pub const LENS_MODEL: u16 = 42036;
}

/// One frame's line on the export surface: the file name and EXIF it will carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub metadata: ExifMetadata,
}

/// The primary directory's stated fields and the Exif directory's, from the hand's metadata and
/// the date; an empty field writes no tag at all.
pub(crate) fn stated(metadata: &ExifMetadata, date: Option<&str>) -> (Directory, Directory) {
    let mut primary = Directory::default();
    let mut exif = Directory::default();
    let text = |s: &str| Value::Ascii(s.to_owned());
    if !metadata.camera.is_empty() {
        primary.set(tag::MODEL, text(&metadata.camera));
    }
    if !metadata.artist.is_empty() {
        primary.set(tag::ARTIST, text(&metadata.artist));
    }
    if !metadata.copyright.is_empty() {
        primary.set(tag::COPYRIGHT, text(&metadata.copyright));
    }
    if !metadata.film.is_empty() {
        primary.set(tag::IMAGE_DESCRIPTION, text(&metadata.film));
    }
    if !metadata.lens.is_empty() {
        exif.set(tag::LENS_MODEL, text(&metadata.lens));
    }
    if let Some(iso) = iso(&metadata.iso) {
        // A SHORT field: Exif states 65535 for any rating at or past it.
        exif.set(tag::ISO_SPEED_RATINGS, Value::Short(vec![iso.min(65535) as u16]));
    }
    if let Some(date) = date {
        primary.set(tag::DATE_TIME, text(date));
        exif.set(tag::DATE_TIME_ORIGINAL, text(date));
    }
    (primary, exif)
}

/// The digits of a typed ISO read as a number; nothing, or zero, writes no tag.
pub fn iso(typed: &str) -> Option<i64> {
    let digits: String = typed.chars().filter(|c| c.is_numeric()).collect();
    digits.parse::<i64>().ok().filter(|v| *v > 0)
}

/// The scan's Exif, or nothing. RW2 swaps TIFF's 42 for 0x55 over the same layout.
fn scan_exif(path: &Path) -> Option<exif::Exif> {
    let mut bytes = std::fs::read(path).ok()?;
    if bytes.starts_with(b"IIU\0") {
        bytes[2] = 0x2A;
    }
    let mut reader = exif::Reader::new();
    reader.continue_on_error(true);
    let parsed = if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        reader.read_raw(bytes)
    } else {
        reader.read_from_container(&mut std::io::Cursor::new(bytes))
    };
    match parsed {
        Ok(found) => Some(found),
        Err(exif::Error::PartialResult(partial)) => Some(partial.into_inner().0),
        Err(_) => None,
    }
}

fn ascii(found: &exif::Exif, tag: exif::Tag) -> Option<String> {
    let field = found.get_field(tag, exif::In::PRIMARY)?;
    match &field.value {
        exif::Value::Ascii(parts) => {
            let first = parts.first()?;
            let text = String::from_utf8_lossy(first);
            Some(text.trim_end_matches('\0').to_owned())
        }
        _ => None,
    }
}

/// Panasonic's JpgFromRaw (RW2 tag 0x2E): the ISO lives in this preview's Exif alone.
const PANASONIC_PREVIEW: u16 = 0x2E;

fn embedded(found: &exif::Exif) -> Option<exif::Exif> {
    let field = found.get_field(exif::Tag(exif::Context::Tiff, PANASONIC_PREVIEW), exif::In::PRIMARY)?;
    let exif::Value::Undefined(jpeg, _) = &field.value else { return None };
    exif::Reader::new().read_from_container(&mut std::io::Cursor::new(jpeg)).ok()
}

/// What the scan itself says: the scanning camera, its lens, its ISO, its date. Offered so the
/// hand sees whose facts these are before it keeps or replaces them.
pub fn scan_metadata(path: &Path) -> ExifMetadata {
    let mut out = ExifMetadata::default();
    let Some(found) = scan_exif(path) else { return out };
    let preview = embedded(&found);
    let sources: Vec<&exif::Exif> = std::iter::once(&found).chain(preview.as_ref()).collect();
    let text = |tag| sources.iter().find_map(|s| ascii(s, tag).filter(|t| !t.is_empty()));
    let make = text(exif::Tag::Make).unwrap_or_default().trim().to_owned();
    let model = text(exif::Tag::Model).unwrap_or_default().trim().to_owned();
    out.camera = if model.starts_with(&make) || make.is_empty() { model } else { format!("{make} {model}") };
    out.lens = text(exif::Tag::LensModel).unwrap_or_default();
    let iso = sources.iter().find_map(|s| {
        s.get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY).and_then(|f| f.value.get_uint(0))
    });
    if let Some(iso) = iso {
        out.iso = iso.to_string();
    }
    out.date = text(exif::Tag::DateTimeOriginal).or_else(|| text(exif::Tag::DateTime)).unwrap_or_default();
    out.artist = text(exif::Tag::Artist).unwrap_or_default();
    out.copyright = text(exif::Tag::Copyright).unwrap_or_default();
    out
}

/// The only field carried over on export: camera settings describe the scan, not the film
/// photograph, and would mislead.
pub fn capture_date(path: &Path) -> Option<String> {
    ascii(&scan_exif(path)?, exif::Tag::DateTimeOriginal)
}
