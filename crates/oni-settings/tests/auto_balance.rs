//! The Auto White Balance kept in the sidecar: it round-trips, a rewrite carries it, and it is never
//! written where no sidecar stands.

use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{AutoBalance, ConversionMode, PipelineSettings};
use std::path::{Path, PathBuf};

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oni-settings-auto-balance-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn photo(dir: &Path, name: &str) -> PathBuf {
    let original = dir.join(name);
    std::fs::write(&original, b"pixels").unwrap();
    original
}

fn edited() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, false);
    s.print.exposure = 0.4;
    s
}

fn balance(fingerprint: &str, filters: [f32; 3]) -> AutoBalance {
    AutoBalance { fingerprint: fingerprint.into(), filters, reference: None }
}

fn modified(original: &Path) -> std::time::SystemTime {
    std::fs::metadata(sidecar::url_for(original)).unwrap().modified().unwrap()
}

#[test]
fn the_kept_balance_round_trips_and_a_rewrite_carries_it() {
    let dir = folder("round-trip");
    let original = photo(&dir, "A.tif");
    sidecar::write(&edited(), &original, WriteOptions::default()).unwrap();
    let before = (sidecar::read(&original, false).unwrap(), modified(&original));
    let value = balance("f00d", [0.19486168, 0.0, -0.26149464]);
    assert!(sidecar::set_auto_balance(&original, false, &value).unwrap());
    assert_eq!(sidecar::auto_balance(&original, false), Some(value.clone()));
    // The settings, the metadata and the revision stand as they were; an f32 keeps its own digits.
    let after = sidecar::read(&original, false).unwrap();
    assert_eq!((after.settings, after.label, after.imported_at), (before.0.settings.clone(), before.0.label, before.0.imported_at));
    assert_eq!(modified(&original), before.1, "the revision moved");
    let text = std::fs::read_to_string(sidecar::url_for(&original)).unwrap();
    assert!(text.contains("0.19486168,"), "{text}");
    // The same value again writes nothing.
    assert!(!sidecar::set_auto_balance(&original, false, &value).unwrap());
    // An edit's rewrite carries it, and the edit lands whole.
    let mut next = edited();
    next.print.exposure = -0.3;
    sidecar::write(&next, &original, WriteOptions::default()).unwrap();
    assert_eq!(sidecar::auto_balance(&original, false), Some(value), "a rewrite dropped the balance");
    assert_eq!(sidecar::read(&original, false).unwrap().settings, next);
    // Twin: another value replaces it.
    let other = balance("beef", [0.0; 3]);
    assert!(sidecar::set_auto_balance(&original, false, &other).unwrap());
    assert_eq!(sidecar::auto_balance(&original, false), Some(other));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_photo_with_no_sidecar_gains_none_for_its_balance_alone() {
    let dir = folder("bare");
    let bare = photo(&dir, "B.tif");
    assert!(!sidecar::set_auto_balance(&bare, false, &balance("f00d", [0.1; 3])).unwrap());
    assert!(!sidecar::url_for(&bare).exists(), "a sidecar was seeded for the balance");
    assert_eq!(sidecar::auto_balance(&bare, false), None);
    // Twin: once the photo has its sidecar, the balance is kept in it.
    sidecar::write(&edited(), &bare, WriteOptions::default()).unwrap();
    assert!(sidecar::set_auto_balance(&bare, false, &balance("f00d", [0.1; 3])).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}
