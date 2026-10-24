//! What a sidecar write refuses: a missing original, a newer or unreadable sidecar; and what it
//! keeps: the keys a newer app added, which this version does not know.

use oni_settings::cg::Point;
use oni_settings::curves::CurveSet;
use oni_settings::sidecar::{self, Error, url_for};
use oni_settings::{Date, Medal, PipelineSettings, codec};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("oni-guarded-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn edited() -> PipelineSettings {
    let mut s = PipelineSettings::default();
    s.print.exposure = 0.7;
    s
}

#[test]
fn a_write_beside_a_missing_original_is_refused_as_unreachable() {
    let dir = scratch("missing");
    let gone = dir.join("Away").join("P1.RW2");
    let moved = dir.join("P2.RW2");
    for result in [
        sidecar::write(&edited(), &moved, Default::default()).map(|_| true),
        sidecar::set_label(Medal::Gold, &moved, true).map(|_| true),
        sidecar::set_trashed(Some(Date::from_unix_seconds(1.0)), &moved, true),
        sidecar::set_trashed(None, &gone, true),
        sidecar::set_roll_draft(None, &moved, true).map(|_| true),
    ] {
        assert!(matches!(result, Err(Error::Unreachable)), "{result:?}");
    }
    assert!(sidecar::flush(&edited(), Some(&moved), None, true).is_some(), "a flush says it");
    assert!(!url_for(&moved).exists() && !url_for(&gone).exists(), "no sidecar beside nothing");
    // Twin: the same gestures on a present photo write its sidecar.
    std::fs::write(&moved, b"").unwrap();
    sidecar::set_label(Medal::Gold, &moved, true).unwrap();
    assert_eq!(sidecar::read(&moved, true, true).unwrap().label, Medal::Gold);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_newer_or_unparseable_sidecar_is_never_overwritten() {
    let dir = scratch("stale");
    let photo = dir.join("P1.NEF");
    std::fs::write(&photo, b"").unwrap();
    sidecar::write(&edited(), &photo, Default::default()).unwrap();
    let opened = sidecar::read(&photo, true, true).unwrap();
    assert!(opened.writable);
    // Another app rewrites the sidecar after the photo opened here: the deferred write still trusts it.
    let newer = br#"{"version": 99, "label": "gold", "settings": {"density": 0.4}}"#;
    let unparseable = b"{\"version\": 19, \"settings\": {\"density\": 0.4, trunc";
    for (bytes, newer_format) in [(&newer[..], true), (&unparseable[..], false)] {
        std::fs::write(url_for(&photo), bytes).unwrap();
        let written = sidecar::write(&opened.settings, &photo, Default::default());
        match newer_format {
            true => assert!(matches!(written, Err(Error::Newer)), "{written:?}"),
            false => assert!(matches!(written, Err(Error::Codec(_))), "{written:?}"),
        }
        let said = sidecar::flush(&PipelineSettings::default(), Some(&photo), Some(&opened.settings), opened.writable);
        assert!(said.is_some(), "the refusal is said, never silent");
        assert!(sidecar::set_trashed(Some(Date::from_unix_seconds(1.0)), &photo, true).map_or(true, |done| !done));
        assert_eq!(std::fs::read(url_for(&photo)).unwrap(), bytes, "left as it was");
    }
    // Twin: today's version, read back whole, takes the write.
    std::fs::write(url_for(&photo), codec::encode_settings(&PipelineSettings::default()).unwrap()).unwrap();
    sidecar::write(&opened.settings, &photo, Default::default()).unwrap();
    assert_eq!(sidecar::read(&photo, true, true).unwrap().settings.print.exposure, 0.7);
    let _ = std::fs::remove_dir_all(&dir);
}

fn document(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn an_unknown_key_survives_a_medal() {
    let dir = scratch("unknown");
    let photo = dir.join("P1.NEF");
    std::fs::write(&photo, b"").unwrap();
    sidecar::write(&edited(), &photo, Default::default()).unwrap();
    let mut stored = document(&url_for(&photo));
    // A newer Swift adds fields without moving the version: at the top, in the settings, deeper.
    stored["swiftLater"] = json!({"kind": "dust", "amount": 0.25});
    stored["settings"]["grainLater"] = json!(0.3);
    stored["settings"]["print"]["toeLater"] = json!([1, 2, 3]);
    std::fs::write(url_for(&photo), serde_json::to_vec_pretty(&stored).unwrap()).unwrap();
    sidecar::set_label(Medal::Gold, &photo, true).unwrap();
    sidecar::set_trashed(Some(Date::from_unix_seconds(5.0)), &photo, true).unwrap();
    let mut later = edited();
    later.density = 0.2;
    sidecar::write(&later, &photo, Default::default()).unwrap();
    let kept = document(&url_for(&photo));
    assert_eq!(kept["swiftLater"], json!({"kind": "dust", "amount": 0.25}));
    assert_eq!((&kept["settings"]["grainLater"], &kept["settings"]["print"]["toeLater"]), (&json!(0.3), &json!([1, 2, 3])));
    let read = sidecar::read(&photo, true, true).unwrap();
    assert_eq!((read.label, read.settings.density, read.trashed_at.is_some()), (Medal::Gold, 0.2, true));
    // A known key cleared stays cleared: the restore drops the mark, never carried back as unknown.
    sidecar::set_trashed(None, &photo, true).unwrap();
    assert!(document(&url_for(&photo)).get("trashedAt").is_none());
    // Twin: a bare encode of the same document loses every one of them.
    let bare: Value = serde_json::from_str(&codec::encode(&read.settings, None, Some(Medal::Gold), None, None, None).unwrap()).unwrap();
    assert!(bare.get("swiftLater").is_none() && bare["settings"].get("grainLater").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_retired_key_is_never_carried_to_be_read_twice() {
    let dir = scratch("retired");
    let photo = dir.join("P1.NEF");
    std::fs::write(&photo, b"").unwrap();
    sidecar::write(&edited(), &photo, Default::default()).unwrap();
    let mut colour = CurveSet::default();
    colour.linked.add(Point { x: 0.5, y: 0.7 });
    let mut stored = document(&url_for(&photo));
    stored["settings"]["curves"] = serde_json::to_value(&colour).unwrap();
    std::fs::write(url_for(&photo), serde_json::to_vec_pretty(&stored).unwrap()).unwrap();
    let folded = sidecar::read(&photo, true, true).unwrap();
    assert!(!folded.settings.manual_curves.is_neutral(), "the retired curves fold on read");
    sidecar::write(&folded.settings, &photo, Default::default()).unwrap();
    let written = document(&url_for(&photo));
    assert!(written["settings"].get("curves").is_none(), "a key this version reads is never carried");
    assert_eq!(sidecar::read(&photo, true, true).unwrap().settings, folded.settings, "folded once, not twice");
    // Twin: the retired key, carried as is, would fold a second time on the next read.
    let mut twice = written.clone();
    twice["settings"]["curves"] = stored["settings"]["curves"].clone();
    let again = codec::decode(&serde_json::to_vec(&twice).unwrap()).unwrap();
    assert_ne!(again.settings, folded.settings);
    let _ = std::fs::remove_dir_all(&dir);
}
