//! A sidecar of format 22 and under drops from its EXIF line the values holding the scan's own facts,
//! once, as its original says them; a clear stays, and an original that cannot say holds nothing back.

use oni_settings::document::{self, VERSION};
use oni_settings::migrate::{self, Facts, Mosaic, Outcome, Refusal, Standing, chain, reshape};
use oni_settings::sidecar::{self, url_for};
use oni_settings::{Document, ExifLine, ExifMetadata, PipelineSettings};
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("oni-scan-line-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// What the digitising camera's file says of itself, as the export reads it.
fn z_f() -> ExifMetadata {
    ExifMetadata { camera: "NIKON CORPORATION NIKON Z f".into(), lens: String::new(), iso: "100".into(), date: "2026:09:14 10:21:03".into(), ..ExifMetadata::default() }
}

/// The line 1.5.6 kept whole on export: the scan's camera, ISO and date, a lens typed by hand, the
/// film and the artist cleared, the copyright empty as the scan's.
fn whole() -> ExifLine {
    ExifLine {
        camera: Some("NIKON CORPORATION NIKON Z f".into()),
        lens: Some("Summicron 35".into()),
        film: Some(String::new()),
        iso: Some("100".into()),
        date: Some("2026:09:14 10:21:03".into()),
        artist: Some(String::new()),
        copyright: Some(String::new()),
    }
}

fn released() -> ExifLine {
    ExifLine { lens: Some("Summicron 35".into()), film: Some(String::new()), artist: Some(String::new()), copyright: Some(String::new()), ..ExifLine::default() }
}

fn edited() -> Document {
    let mut d = Document::of(PipelineSettings::default());
    d.settings.print.exposure = 0.3;
    d.exif_line = whole();
    d
}

/// `document` as the build of `version` wrote it: format 22 in this shape, earlier ones in theirs.
fn stored_at(document: &Document, version: i64) -> Vec<u8> {
    if version >= chain::NAMED_VERSION {
        return document::encode(&Document { version, ..document.clone() }).unwrap();
    }
    serde_json::to_vec_pretty(&reshape::as_stored(document, version)).unwrap()
}

/// A photo beside `bytes` as its sidecar.
fn laid(dir: &Path, bytes: &[u8]) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let photo = dir.join("DSC_6439.NEF");
    std::fs::write(&photo, b"").unwrap();
    std::fs::write(url_for(&photo), bytes).unwrap();
    photo
}

fn answering(exif: &dyn Fn() -> Option<ExifMetadata>, then: impl FnOnce(&Facts)) {
    then(&Facts { mosaic: Mosaic::Known(true), matrix: &|| Ok(None), scan: &|_| Err(Refusal::ScanUnknown), exif });
}

#[test]
fn a_line_of_format_22_and_one_kept_under_exif_release_the_scan_s_facts() {
    for version in [chain::NAMED_VERSION, chain::STROKES_ON_SCAN_VERSION, 19] {
        let dir = scratch(&format!("release-{version}"));
        let photo = laid(&dir, &stored_at(&edited(), version));
        answering(&|| Some(z_f()), |f| assert_eq!(migrate::file_with(&photo, f), Ok(Outcome::Migrated { from: version })));
        let read = sidecar::read(&photo, true).unwrap();
        assert_eq!(read.exif, released(), "format {version}: the lens differing from the scan's and every clear stay");
        assert_eq!((read.settings.print.exposure, read.writable), (0.3, true), "format {version}");
        // Twin: a scan of another camera releases nothing of the camera.
        let other = laid(&dir.join("other"), &stored_at(&edited(), version));
        let leica = ExifMetadata { camera: "LEICA CAMERA AG LEICA Q3".into(), ..z_f() };
        answering(&|| Some(leica.clone()), |f| migrate::file_with(&other, f).map(|_| ()).unwrap());
        assert_eq!(sidecar::read(&other, true).unwrap().exif.camera.as_deref(), Some("NIKON CORPORATION NIKON Z f"), "format {version}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn an_original_that_cannot_say_leaves_the_line_and_the_file_still_comes_to_this_format() {
    let dir = scratch("unsaid");
    let photo = laid(&dir, &stored_at(&edited(), chain::NAMED_VERSION));
    // No facts installed, as in a tool, or an original in the cloud: the read takes the line as it stands, writable.
    let read = sidecar::read(&photo, true).unwrap();
    assert_eq!((read.exif.clone(), read.writable, read.held_back), (whole(), true, None));
    answering(&|| None, |f| assert_eq!(migrate::file_with(&photo, f), Ok(Outcome::Migrated { from: chain::NAMED_VERSION })));
    let bytes = std::fs::read(url_for(&photo)).unwrap();
    assert_eq!(migrate::standing(&bytes), Standing::Current);
    assert_eq!(document::decode(&bytes).unwrap().exif_line, whole());
    // Twin: the same file once at this format answers no later fact; its line is the hand's from now on.
    answering(&|| Some(z_f()), |f| assert_eq!(migrate::file_with(&photo, f), Ok(Outcome::Current)));
    assert_eq!(std::fs::read(url_for(&photo)).unwrap(), bytes);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_scan_whose_exif_says_nothing_releases_nothing() {
    let dir = scratch("unparsed");
    let photo = laid(&dir, &stored_at(&edited(), chain::NAMED_VERSION));
    answering(&|| Some(ExifMetadata::default()), |f| assert_eq!(migrate::file_with(&photo, f), Ok(Outcome::Migrated { from: chain::NAMED_VERSION })));
    assert_eq!(sidecar::read(&photo, true).unwrap().exif, whole(), "the clears and the values all stay");
    // Twin: the scan answering its facts releases the camera.
    let said = laid(&dir.join("said"), &stored_at(&edited(), chain::NAMED_VERSION));
    answering(&|| Some(z_f()), |f| migrate::file_with(&said, f).map(|_| ()).unwrap());
    assert_eq!(sidecar::read(&said, true).unwrap().exif.camera, None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_line_written_at_this_format_keeps_what_equals_the_scan() {
    let dir = scratch("current");
    let photo = laid(&dir, &stored_at(&edited(), VERSION));
    let bytes = std::fs::read(url_for(&photo)).unwrap();
    answering(&|| Some(z_f()), |f| {
        assert_eq!(migrate::file_with(&photo, f), Ok(Outcome::Current));
        assert_eq!(migrate::converted(&bytes, f).unwrap().document.exif_line, whole());
    });
    let typed = ExifLine { camera: Some(z_f().camera), ..ExifLine::default() };
    sidecar::set_exif(typed.clone(), &photo, true).unwrap();
    let written = std::fs::read(url_for(&photo)).unwrap();
    answering(&|| Some(z_f()), |f| assert_eq!(migrate::converted(&written, f).unwrap().document.exif_line, typed, "a camera typed now equal to the scan's is kept"));
    // Twin: the same document stamped one format earlier is released.
    answering(&|| Some(z_f()), |f| assert_eq!(migrate::converted(&stored_at(&edited(), chain::SCAN_LINE_RELEASED_VERSION - 1), f).unwrap().document.exif_line, released()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_settings_document_of_format_22_reads_its_settings_as_they_stand() {
    let mut d = edited();
    d.settings.scanner = oni_settings::scanner::ScannerClicks::new(3, -1, 0, 0, 0);
    let read = migrate::document(&stored_at(&d, chain::NAMED_VERSION)).unwrap();
    assert_eq!(read.document.settings, d.settings);
    // Twin: read as a format 21 document, its keypad's steps would be taken for whole units.
    let unnamed = serde_json::to_vec(&reshape::as_stored(&Document { version: chain::NAMED_VERSION, ..d.clone() }, chain::STROKES_ON_SCAN_VERSION)).unwrap();
    let mut misread: serde_json::Value = serde_json::from_slice(&unnamed).unwrap();
    misread["settings"]["scanner"] = serde_json::json!({"d": 3, "c": -1, "m": 0, "y": 0, "w": 0});
    assert_ne!(migrate::document(&serde_json::to_vec(&misread).unwrap()).unwrap().document.settings.scanner, d.settings.scanner);
}

/// What caches a sidecar reads its line as an export does, released in memory whether the file is
/// brought or not, and says the format it was stored at.
#[test]
fn the_envelope_reads_the_line_an_export_reads() {
    let dir = scratch("envelope");
    let photo = laid(&dir, &stored_at(&edited(), chain::NAMED_VERSION));
    let ours = photo.clone();
    migrate::install(Some(std::sync::Arc::new(move |p: &Path, _: Option<bool>, then: &mut dyn FnMut(&Facts)| {
        if p == ours {
            then(&Facts { exif: &|| Some(z_f()), ..Facts::bare(true) });
        }
    })));
    let data = std::fs::read(url_for(&photo)).unwrap();
    let envelope = sidecar::identity::envelope(&data, &photo).unwrap();
    let read = sidecar::read(&photo, true).unwrap();
    migrate::install(None);
    assert_eq!((&envelope.exif, envelope.format), (&read.exif, Some(chain::NAMED_VERSION)));
    assert_eq!(envelope.exif, released());
    // Twin: the stored document's own envelope still names the scan's camera.
    assert_eq!(document::envelope(&data).unwrap().exif, whole());
    let _ = std::fs::remove_dir_all(&dir);
}
