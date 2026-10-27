//! The Auto White Balance kept in the sidecar under Rust's own key: it round-trips, a rewrite carries
//! it, a sidecar without it reads as before, and it is never written where no sidecar stands.

use oni_settings::sidecar::{self, AUTO_BALANCE, WriteOptions};
use oni_settings::{ConversionMode, PipelineSettings};
use serde_json::json;
use std::path::{Path, PathBuf};

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oni-settings-auto-balance-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn photo(dir: &Path) -> PathBuf {
    let original = dir.join("A.tif");
    std::fs::write(&original, b"pixels").unwrap();
    original
}

fn edited() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, false);
    s.print.exposure = 0.4;
    s
}

/// A value with every number a float: the file writes a whole float as an integer.
fn floats(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Number(n) => json!(n.as_f64()),
        serde_json::Value::Array(a) => serde_json::Value::Array(a.iter().map(floats).collect()),
        serde_json::Value::Object(o) => serde_json::Value::Object(o.iter().map(|(k, v)| (k.clone(), floats(v))).collect()),
        other => other.clone(),
    }
}

fn read_back(original: &Path) -> Option<serde_json::Value> {
    sidecar::auto_balance(original).map(|v| floats(&v))
}

fn modified(original: &Path) -> std::time::SystemTime {
    std::fs::metadata(sidecar::url_for(original)).unwrap().modified().unwrap()
}

#[test]
fn the_kept_balance_round_trips_and_a_rewrite_carries_it() {
    let dir = folder("round-trip");
    let original = photo(&dir);
    sidecar::write(&edited(), &original, WriteOptions::default()).unwrap();
    let before = (sidecar::read(&original, false, true).unwrap(), modified(&original));
    let value = json!({"fingerprint": "f00d", "filters": [0.25, 0.0, -0.25]});
    assert!(sidecar::set_auto_balance(&original, false, &value).unwrap());
    assert_eq!(read_back(&original), Some(floats(&value)));
    // The settings, the metadata and the revision stand as they were.
    assert_eq!(sidecar::read(&original, false, true).unwrap(), before.0);
    assert_eq!(modified(&original), before.1, "the revision moved");
    // The same value again writes nothing.
    assert!(!sidecar::set_auto_balance(&original, false, &value).unwrap());

    // An edit's rewrite carries the key, and the edit lands whole.
    let mut next = edited();
    next.print.exposure = -0.3;
    sidecar::write(&next, &original, WriteOptions::default()).unwrap();
    assert_eq!(read_back(&original), Some(floats(&value)), "a rewrite dropped the key");
    assert_eq!(sidecar::read(&original, false, true).unwrap().settings, next);
    // Twin: another value replaces it.
    let other = json!({"fingerprint": "beef", "filters": [0.0, 0.0, 0.0]});
    assert!(sidecar::set_auto_balance(&original, false, &other).unwrap());
    assert_eq!(read_back(&original), Some(floats(&other)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_sidecar_without_the_key_reads_as_before_and_none_is_seeded() {
    let dir = folder("swift");
    let original = photo(&dir);
    // Swift's shape: the settings document alone, none of Rust's keys beside it.
    sidecar::write(&edited(), &original, WriteOptions::default()).unwrap();
    let url = sidecar::url_for(&original);
    let mut value: serde_json::Value = serde_json::from_slice(&std::fs::read(&url).unwrap()).unwrap();
    let object = value.as_object_mut().unwrap();
    object.retain(|k, _| !k.starts_with("original"));
    std::fs::write(&url, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    let plain = sidecar::read(&original, false, true).unwrap();
    assert_eq!(plain.settings, edited());
    assert_eq!(sidecar::auto_balance(&original), None);
    // A key Swift wrote and Rust does not know stays through the balance's write.
    value.as_object_mut().unwrap().insert("swiftOnly".into(), json!({"kept": true}));
    std::fs::write(&url, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    assert!(sidecar::set_auto_balance(&original, false, &json!({"fingerprint": "f00d"})).unwrap());
    let after: serde_json::Value = serde_json::from_slice(&std::fs::read(&url).unwrap()).unwrap();
    assert_eq!(after.get("swiftOnly"), Some(&json!({"kept": true})), "the balance's write lost Swift's key");
    assert_eq!(sidecar::read(&original, false, true).unwrap().settings, plain.settings);
    value.as_object_mut().unwrap().remove("swiftOnly");
    // With the key, Rust reads the same settings: the key changes nothing the codec reads.
    value.as_object_mut().unwrap().insert(AUTO_BALANCE.into(), json!({"fingerprint": "f00d"}));
    std::fs::write(&url, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    assert_eq!(sidecar::read(&original, false, true).unwrap().settings, plain.settings);

    // A photo with no sidecar gains none for its balance alone.
    let bare = dir.join("B.tif");
    std::fs::write(&bare, b"pixels").unwrap();
    assert!(!sidecar::set_auto_balance(&bare, false, &json!({"fingerprint": "f00d"})).unwrap());
    assert!(!sidecar::url_for(&bare).exists(), "a sidecar was seeded for the balance");
    // Twin: once the photo has its sidecar, the balance is kept in it.
    sidecar::write(&edited(), &bare, WriteOptions::default()).unwrap();
    assert!(sidecar::set_auto_balance(&bare, false, &json!({"fingerprint": "f00d"})).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}
