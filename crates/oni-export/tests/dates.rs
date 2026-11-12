//! The date a file carries is the line's alone: a cleared one writes none, even from a dated scan.

use oni_colour::{PaperCorrection, Space};
use oni_export::metadata::tag;
use oni_export::{ExifMetadata, Format, Image, Settings, capture_date, converted, encoded, file_bytes};

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
