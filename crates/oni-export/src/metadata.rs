//! What a file says about the photograph: only the fields the hand stated, never the scan's own.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
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

/// The head a scan's directories are looked for in first: a NEF's end within 40 KB, the
/// preview an RW2 keeps its ISO in near 520 KB.
const HEAD: u64 = 1 << 20;

/// The scan's Exif, or nothing, read from the file's head alone: a TIFF's grows fourfold while a
/// directory or a value runs past it, so a RAW's pixels are never read.
fn scan_exif(path: &Path) -> Option<exif::Exif> {
    head_exif(path, HEAD).map(|(found, _)| found)
}

/// The Exif and the bytes read to find it. RW2 swaps TIFF's 42 for 0x55 over the same layout.
fn head_exif(path: &Path, head: u64) -> Option<(exif::Exif, u64)> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let mut magic = Vec::with_capacity(4);
    (&mut file).take(4).read_to_end(&mut magic).ok()?;
    let mut reader = exif::Reader::new();
    reader.continue_on_error(true);
    if !matches!(magic.as_slice(), b"II*\0" | b"MM\0*" | b"IIU\0") {
        file.seek(SeekFrom::Start(0)).ok()?;
        let mut buffered = BufReader::new(file);
        let (found, _) = parsed(reader.read_from_container(&mut buffered));
        let read = buffered.stream_position().unwrap_or(len);
        return found.map(|f| (f, read));
    }
    let mut take = head.min(len);
    loop {
        let mut bytes = Vec::with_capacity(take as usize);
        file.seek(SeekFrom::Start(0)).ok()?;
        (&mut file).take(take).read_to_end(&mut bytes).ok()?;
        if bytes.starts_with(b"IIU\0") {
            bytes[2] = 0x2A;
        }
        match parsed(reader.read_raw(bytes)) {
            (_, true) if take < len => take = take.saturating_mul(4).min(len),
            (found, _) => return found.map(|f| (f, take)),
        }
    }
}

/// What a parse found, a partial one kept, and whether an error says the bytes were cut short.
fn parsed(result: Result<exif::Exif, exif::Error>) -> (Option<exif::Exif>, bool) {
    match result {
        Ok(found) => (Some(found), false),
        Err(exif::Error::PartialResult(partial)) => {
            let (found, errors) = partial.into_inner();
            let short = errors.iter().any(cut);
            (Some(found), short)
        }
        Err(e) => (None, cut(&e)),
    }
}

fn cut(e: &exif::Error) -> bool {
    matches!(e, exif::Error::InvalidFormat(why) if why.starts_with("Truncated"))
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
    let Some(found) = scan_exif(path) else { return described(path) };
    let preview = embedded(&found);
    let sources: Vec<&exif::Exif> = std::iter::once(&found).chain(preview.as_ref()).collect();
    let text = |tag| sources.iter().find_map(|s| ascii(s, tag).filter(|t| !t.is_empty()));
    let make = text(exif::Tag::Make).unwrap_or_default().trim().to_owned();
    let model = text(exif::Tag::Model).unwrap_or_default().trim().to_owned();
    out.camera = camera(&make, &model);
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

/// The model alone when it already names its maker, as Swift joins them.
fn camera(make: &str, model: &str) -> String {
    if model.starts_with(make) || make.is_empty() { model.to_owned() } else { format!("{make} {model}") }
}

/// A RAW whose container the Exif reader cannot open (CR3, RAF, ORF), as LibRaw reads it; the date
/// is the one the file stores.
fn described(path: &Path) -> ExifMetadata {
    let mut out = ExifMetadata::default();
    if !oni_decode::is_raw(path) {
        return out;
    }
    let Ok(found) = oni_decode::camera::describe(path) else { return out };
    out.camera = camera(&found.make, &found.model);
    out.lens = found.lens;
    out.artist = found.artist;
    if found.iso >= 1.0 {
        out.iso = format!("{}", found.iso.round() as i64);
    }
    out.date = oni_settings::exif::capture_date(path).unwrap_or_default();
    out
}

/// The only field carried over on export: camera settings describe the scan, not the film
/// photograph, and would mislead.
pub fn capture_date(path: &Path) -> Option<String> {
    match scan_exif(path) {
        Some(found) => ascii(&found, exif::Tag::DateTimeOriginal),
        None => oni_settings::exif::capture_date(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn listed(dir: &Path, extensions: &[&str]) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|d| d.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        out.retain(|p| p.extension().and_then(|e| e.to_str()).is_some_and(|e| extensions.contains(&e.to_ascii_lowercase().as_str())));
        out.sort();
        out
    }

    /// The facts the box reads, compared whole.
    fn facts(found: &exif::Exif) -> Vec<Option<String>> {
        [exif::Tag::Make, exif::Tag::Model, exif::Tag::LensModel, exif::Tag::DateTimeOriginal].map(|t| ascii(found, t)).into()
    }

    fn heads(dir: &Path, extensions: &[&str]) {
        for path in listed(dir, extensions) {
            let whole = std::fs::metadata(&path).unwrap().len();
            let (head, read) = head_exif(&path, HEAD).unwrap();
            let (all, _) = head_exif(&path, whole).unwrap();
            println!("{}: {read} of {whole} bytes", path.display());
            assert!(read <= HEAD && read * 20 < whole, "{}: {read} of {whole} bytes", path.display());
            assert_eq!(facts(&head), facts(&all), "{}", path.display());
            assert_eq!(embedded(&head).is_some(), embedded(&all).is_some(), "{}", path.display());
            // Twin: a 4 KB head grows until it holds them, and parsed as it is, it lacks the lens.
            let (grown, read) = head_exif(&path, 4096).unwrap();
            assert!(read > 4096 && facts(&grown) == facts(&all), "{}", path.display());
            let mut cut = Vec::new();
            File::open(&path).unwrap().take(4096).read_to_end(&mut cut).unwrap();
            if cut.starts_with(b"IIU\0") {
                cut[2] = 0x2A;
            }
            let mut reader = exif::Reader::new();
            reader.continue_on_error(true);
            let (short, _) = parsed(reader.read_raw(cut));
            assert!(short.is_none_or(|s| facts(&s) != facts(&all)), "{}", path.display());
        }
    }

    #[test]
    fn a_scan_s_facts_come_from_its_head_alone() {
        if let Some(roll) = oni_fixtures::raw_dir("a_scan_s_facts_come_from_its_head_alone") {
            heads(&roll, &["nef"]);
        }
        if let Some(lumix) = oni_fixtures::lumix_dir("a_scan_s_facts_come_from_its_head_alone") {
            heads(&lumix, &["rw2"]);
        }
    }

    #[test]
    fn a_cr3_raf_or_orf_states_its_camera_lens_iso_and_date() {
        let Some(dir) = oni_fixtures::raw_dir("a_cr3_raf_or_orf_states_its_camera_lens_iso_and_date") else { return };
        let found = listed(&dir, &["cr3", "raf", "orf"]);
        if found.is_empty() {
            println!("SKIPPED a_cr3_raf_or_orf_states_its_camera_lens_iso_and_date: no CR3, RAF or ORF in {}", dir.display());
            return;
        }
        for path in found {
            let scan = scan_metadata(&path);
            println!("{}: {scan:?}", path.display());
            assert!(!scan.camera.is_empty() && !scan.lens.is_empty(), "{scan:?}");
            assert!(iso(&scan.iso).is_some(), "{scan:?}");
            assert!(scan.date.len() == 19 && scan.date.as_bytes()[4] == b':', "{scan:?}");
            assert_eq!(capture_date(&path).as_deref(), Some(scan.date.as_str()));
            // Twin: the Exif reader alone opens none of these containers.
            assert!(scan_exif(&path).is_none(), "{}", path.display());
        }
    }
}
