//! The date a file carries is the line's alone: a cleared one writes none, even from a dated scan,
//! and one read from a sidecar or a scan reaches it in EXIF's nineteen characters.

use oni_colour::{PaperCorrection, Space};
use oni_export::metadata::tag;
use oni_export::{ExifMetadata, Format, Image, Settings, capture_date, converted, encoded, file_bytes, scan_metadata};

const DATE: &str = "2001:02:03 04:05:06";

fn grey() -> Image {
    Image { width: 8, height: 8, pixels: [0.2, 0.2, 0.2, 1.0].repeat(64) }
}

/// The tags a reader finds in `file`, primary and Exif directories alike.
fn tags(file: &[u8]) -> Vec<u16> {
    let mut reader = exif::Reader::new();
    reader.continue_on_error(true);
    match reader.read_raw(file.to_vec()) {
        Ok(f) => f.fields().map(|f| f.tag.number()).collect(),
        Err(exif::Error::PartialResult(p)) => p.into_inner().0.fields().map(|f| f.tag.number()).collect(),
        Err(_) => Vec::new(),
    }
}

#[test]
fn a_cleared_date_writes_no_date_even_from_a_dated_source() {
    let mut export = Settings::default();
    export.set_format(Format::Tiff16);
    export.set_space(Space::Srgb);
    let dir = std::env::temp_dir().join(format!("oni-export-dates-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("scan.tif");
    let dated = ExifMetadata { date: DATE.into(), ..ExifMetadata::default() };
    std::fs::write(&source, encoded(&converted(&grey(), &export).unwrap(), &export, &dated).unwrap()).unwrap();
    // Twin: the source carries a date a fallback would leak.
    assert_eq!(capture_date(&source).as_deref(), Some(DATE));
    let paper = PaperCorrection::default();
    let cleared = file_bytes(grey(), &paper, &export, &ExifMetadata::default()).unwrap();
    let found = tags(&cleared);
    assert!(!found.contains(&tag::DATE_TIME) && !found.contains(&tag::DATE_TIME_ORIGINAL), "a cleared date leaks the scan's: {found:?}");
    let stated = file_bytes(grey(), &paper, &export, &dated).unwrap();
    let found = tags(&stated);
    assert!(found.contains(&tag::DATE_TIME) && found.contains(&tag::DATE_TIME_ORIGINAL), "{found:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The text of `tag` in `file`'s primary directory, or of the Exif directory's.
fn text(file: &[u8], tag: u16) -> Option<String> {
    let found = exif::Reader::new().read_raw(file.to_vec()).ok()?;
    let field = found.fields().find(|f| f.tag.number() == tag)?;
    let exif::Value::Ascii(ref parts) = field.value else { return None };
    parts.first().map(|b| String::from_utf8_lossy(b).into_owned())
}

#[test]
fn a_date_read_from_a_sidecar_or_a_scan_reaches_the_file_in_exifs_nineteen_characters() {
    let mut export = Settings::default();
    export.set_format(Format::Tiff16);
    export.set_space(Space::Srgb);
    let paper = PaperCorrection::default();
    let day = "2026:09:14";
    // A line a sidecar kept from an earlier build: a day in colons without its time.
    let stored = format!(r#"{{"camera": "", "lens": "", "film": "", "iso": "", "date": "{day}", "artist": "", "copyright": ""}}"#);
    let line: ExifMetadata = serde_json::from_str(&stored).unwrap();
    let file = file_bytes(grey(), &paper, &export, &line).unwrap();
    for tag in [tag::DATE_TIME, tag::DATE_TIME_ORIGINAL] {
        assert_eq!(text(&file, tag).as_deref(), Some("2026:09:14 00:00:00"), "tag {tag}");
    }
    // A scan whose own date is malformed opens its line on the settled one.
    let dir = std::env::temp_dir().join(format!("oni-export-dates-scan-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("scan.tif");
    let malformed = ExifMetadata { date: day.into(), ..ExifMetadata::default() };
    let scan = encoded(&converted(&grey(), &export).unwrap(), &export, &malformed).unwrap();
    std::fs::write(&source, &scan).unwrap();
    assert_eq!(scan_metadata(&source).date, "2026:09:14 00:00:00");
    // Twin: a line that skipped the reading writes its ten characters as they stand, as the scan holds them.
    assert_eq!(text(&scan, tag::DATE_TIME).as_deref(), Some(day));
    assert_eq!(capture_date(&source).as_deref(), Some(day));
    let _ = std::fs::remove_dir_all(&dir);
}
