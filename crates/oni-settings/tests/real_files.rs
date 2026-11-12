//! The sidecar on disk, on real files: a corpus sidecar brought to this format, then read, edited and
//! written back; and the roll #8 marker.

use oni_settings::document;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{ConversionMode, Date, Medal, PipelineSettings, roll_folder};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn value(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("oni-settings-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A corpus sidecar of format 19, its EXIF line kept, beside an empty stand-in: read once, it stands
/// in this format; written once, it names the stand-in. Its text as it then stands.
fn copied(dir: &Path) -> (PathBuf, String) {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/19-rust/040-P1073163.RW2.json");
    let original = dir.join("P1073163.RW2");
    std::fs::write(&original, b"").unwrap();
    std::fs::copy(&corpus, sidecar::url_for(&original)).unwrap();
    let loaded = sidecar::read(&original, true).unwrap();
    assert!(loaded.writable && !loaded.is_new && !loaded.exif.is_empty());
    sidecar::write(&loaded.settings, &original, WriteOptions::default()).unwrap();
    (original.clone(), std::fs::read_to_string(sidecar::url_for(&original)).unwrap())
}

#[test]
fn a_sidecar_read_and_written_back_is_the_same_file() {
    let dir = scratch("rewrite");
    let (original, text) = copied(&dir);
    assert_eq!(value(&text)["format"], json!(document::FORMAT));
    let loaded = sidecar::read(&original, true).unwrap();
    // The settings alone are written: the envelope is read back from the file.
    sidecar::write(&loaded.settings, &original, WriteOptions::default()).unwrap();
    assert_eq!(std::fs::read_to_string(sidecar::url_for(&original)).unwrap(), text);
    // Twin: an update that changes nothing writes nothing, one that changes a gain writes it.
    assert!(!sidecar::update(&original, true, |s| s.clone()).unwrap());
    assert!(sidecar::update(&original, true, |s| {
        let mut s = s.clone();
        s.gains.stops[1] = 0.25;
        s
    })
    .unwrap());
    let after = std::fs::read_to_string(sidecar::url_for(&original)).unwrap();
    assert_ne!(after, text);
    assert_eq!(value(&after)["exifLine"], value(&text)["exifLine"]);
    assert_eq!(value(&after)["settings"]["gains"]["stops"][1], json!(0.25));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn label_and_trash_touch_nothing_else_and_keep_the_revision() {
    let dir = scratch("envelope");
    let (original, text) = copied(&dir);
    let path = sidecar::url_for(&original);
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    let revision = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    file.set_modified(revision).unwrap();
    drop(file);
    let modified = || std::fs::metadata(&path).unwrap().modified().unwrap();
    let start = sidecar::read(&original, true).unwrap();

    sidecar::set_label(Medal::Gold, &original, true).unwrap();
    let labelled = value(&std::fs::read_to_string(&path).unwrap());
    assert_eq!(labelled["label"], json!("gold"));
    assert_eq!(modified(), revision);
    let mut without = labelled.clone();
    without.as_object_mut().unwrap().insert("label".into(), value(&text)["label"].clone());
    if value(&text).get("label").is_none() {
        without.as_object_mut().unwrap().remove("label");
    }
    assert_eq!(without, value(&text), "the label alone moved");

    let stamp = Date::from_unix_seconds(1_760_000_000.0);
    assert!(sidecar::set_trashed(Some(stamp), &original, true).unwrap());
    let trashed = value(&std::fs::read_to_string(&path).unwrap());
    assert_eq!(trashed["trashedAt"], json!(stamp.iso8601()));
    assert_eq!(modified(), revision);
    assert!(sidecar::set_trashed(start.trashed_at, &original, true).unwrap());
    sidecar::set_label(start.label, &original, true).unwrap();
    // Back where it started, byte for byte.
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_newer_format_opens_read_only_and_is_never_rewritten() {
    let dir = scratch("future");
    let (original, text) = copied(&dir);
    let path = sidecar::url_for(&original);
    let future = text.replacen(&format!("\"version\": {}", document::VERSION), &format!("\"version\": {}", document::VERSION + 1), 1);
    assert_ne!(future, text);
    std::fs::write(&path, &future).unwrap();
    let loaded = sidecar::read(&original, true).unwrap();
    assert!(!loaded.writable);
    assert!(!sidecar::update(&original, true, |s| PipelineSettings { density: 0.4, ..s.clone() }).unwrap());
    assert!(matches!(sidecar::set_label(Medal::Gold, &original, true), Err(sidecar::Error::Newer)), "the refusal is said");
    assert!(!sidecar::set_trashed(Some(Date::from_unix_seconds(1.0)), &original, true).unwrap());
    assert!(matches!(sidecar::write(&loaded.settings, &original, Default::default()), Err(sidecar::Error::Newer)));
    assert_eq!(sidecar::flush(&PipelineSettings::default(), Some(&original), Some(&loaded.settings), loaded.writable), None);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), future);
    // Twin: today's version is writable.
    std::fs::write(&path, &text).unwrap();
    assert!(sidecar::read(&original, true).unwrap().writable);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_absent_sidecar_is_a_new_photo_and_an_unreadable_one_an_error() {
    let dir = scratch("absent");
    let original = dir.join("DSC_9999.NEF");
    std::fs::write(&original, b"").unwrap();
    let loaded = sidecar::read(&original, true).unwrap();
    assert!(loaded.is_new && loaded.writable);
    assert_eq!(loaded.settings, PipelineSettings::rest(ConversionMode::Negative, true));
    assert!(!sidecar::should_write(&loaded.settings, &original));
    // Twin: a moved setting is worth writing, and once written a return to rest is too.
    let mut moved = loaded.settings.clone();
    moved.density = 0.2;
    assert!(sidecar::should_write(&moved, &original));
    assert_eq!(sidecar::flush(&moved, Some(&original), Some(&loaded.settings), true), None);
    assert!(sidecar::url_for(&original).exists());
    assert!(sidecar::should_write(&loaded.settings, &original));
    let back = sidecar::read(&original, true).unwrap();
    assert_eq!(back.settings, moved);
    assert!(!back.is_new);

    std::fs::write(sidecar::url_for(&original), "not JSON").unwrap();
    assert!(sidecar::read(&original, true).is_err());
    assert!(sidecar::set_trashed(Some(Date::from_unix_seconds(1.0)), &original, true).is_err());
    assert_eq!(std::fs::read_to_string(sidecar::url_for(&original)).unwrap(), "not JSON");
    sidecar::remove(&original).unwrap();
    assert!(!sidecar::url_for(&original).exists());
    sidecar::remove(&original).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_roll_8_marker_reads_and_writes_back() {
    let Some(roll) = oni_fixtures::raw_dir("the_roll_8_marker_reads_and_writes_back") else { return };
    let bytes = std::fs::read(roll.join(".oni-roll.json")).unwrap();
    let film = roll_folder::decode(&bytes).expect("the roll marker decodes");
    assert_eq!(film.name, "Roll #8");
    let written = roll_folder::encode(&film);
    assert_eq!(roll_folder::decode(written.as_bytes()), Some(film.clone()), "{written}");
    let (stored, now) = (value(&String::from_utf8_lossy(&bytes)), value(&written));
    let keys = |v: &Value| v.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
    assert_eq!(keys(&stored), keys(&now), "the marker's keys never move");
    assert!(!written.contains('\n') && !written.contains(": "), "compact: {written}");
    // Twin: a renamed roll is another document.
    let mut renamed = film.clone();
    renamed.name = "Roll #9".into();
    assert_ne!(roll_folder::decode(roll_folder::encode(&renamed).as_bytes()), Some(film));
}
