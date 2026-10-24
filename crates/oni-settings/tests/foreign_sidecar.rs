//! A sidecar left by a photo deleted or renamed is never adopted by a later namesake: each write
//! names its original by its print, and a stray one is set aside whole at the next write.

use oni_settings::sidecar::{self, identity, url_for};
use oni_settings::{Date, Medal, PipelineSettings};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("oni-foreign-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A little-endian TIFF whose EXIF IFD holds `date`, then 200 KB of `fill`: the print's three
/// blocks sample the fill, never the header, as on a real photo.
fn shot(date: &str, fill: u8) -> Vec<u8> {
    let mut out = b"II*\0".to_vec();
    out.extend(8u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(0x8769u16.to_le_bytes());
    out.extend(4u16.to_le_bytes());
    out.extend(1u32.to_le_bytes());
    out.extend(26u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(0x9003u16.to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(20u32.to_le_bytes());
    out.extend(44u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(date.as_bytes());
    out.push(0);
    out.extend((0..200_000u32).map(|i| fill.wrapping_mul((i % 251) as u8)));
    out
}

fn stored(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(url_for(path)).unwrap()).unwrap()
}

#[test]
fn a_foreign_sidecar_is_not_adopted() {
    let dir = scratch("adopted");
    let photo = dir.join("DSC_0001.NEF");
    std::fs::write(&photo, shot("2024:05:01 10:00:00", 3)).unwrap();
    let mut edited = PipelineSettings::default();
    edited.print.exposure = 0.7;
    sidecar::write(&edited, &photo, Default::default()).unwrap();
    sidecar::set_trashed(Some(Date::from_unix_seconds(1.0)), &photo, false).unwrap();
    sidecar::set_label(Medal::Gold, &photo, false).unwrap();
    let print = stored(&photo)[identity::KEY].as_str().unwrap().to_owned();
    assert!(print.contains("-2024:05:01 10:00:00-"), "{print}");

    // The frame deleted, another body's DSC_0001 copied in: its own capture date.
    std::fs::remove_file(&photo).unwrap();
    std::fs::write(&photo, shot("2026:10:25 09:00:00", 5)).unwrap();
    let loaded = sidecar::read(&photo, false, false).unwrap();
    assert!(loaded.foreign && loaded.is_new, "read as no sidecar");
    assert_eq!((loaded.label, loaded.trashed_at), (Medal::None, None));
    assert_eq!(loaded.settings, PipelineSettings::rest(oni_settings::ConversionMode::Negative, false));
    let envelope = identity::envelope(&std::fs::read(url_for(&photo)).unwrap(), &photo).unwrap();
    assert_eq!((envelope.label, envelope.trashed_at), (Medal::None, None), "never in the ONI Trash");

    // Its first write sets the stray one aside whole, and names the new original.
    sidecar::set_label(Medal::Silver, &photo, false).unwrap();
    let aside = dir.join("DSC_0001.NEF (orphan).json");
    let kept: Value = serde_json::from_slice(&std::fs::read(&aside).unwrap()).unwrap();
    assert_eq!((kept["label"].as_str(), kept[identity::KEY].as_str()), (Some("gold"), Some(print.as_str())));
    assert!(kept.get("trashedAt").is_some(), "the old photo's settings kept whole");
    let now = stored(&photo);
    assert_eq!(now["label"].as_str(), Some("silver"));
    assert!(now[identity::KEY].as_str().unwrap().contains("-2026:10:25 09:00:00-"));
    assert!(now.get("trashedAt").is_none());

    // Twins: the same frame saved over by another app, its date kept, or its date set right in
    // place, its bytes kept, keeps its settings.
    std::fs::write(&photo, shot("2026:10:25 09:00:00", 7)).unwrap();
    let saved = sidecar::read(&photo, false, false).unwrap();
    assert!(!saved.foreign && saved.label == Medal::Silver);
    std::fs::write(&photo, shot("1999:12:31 23:59:59", 5)).unwrap();
    let redated = sidecar::read(&photo, false, false).unwrap();
    assert!(!redated.foreign && redated.label == Medal::Silver);
    std::fs::write(&photo, shot("2026:10:25 09:00:00", 5)).unwrap();
    // Twin: a sidecar Swift wrote, with no key, is adopted as before.
    let swift = dir.join("DSC_0002.NEF");
    std::fs::write(&swift, shot("2026:10:25 09:00:01", 3)).unwrap();
    sidecar::set_label(Medal::Bronze, &swift, false).unwrap();
    let mut value = stored(&swift);
    value.as_object_mut().unwrap().remove(identity::KEY);
    std::fs::write(url_for(&swift), serde_json::to_vec(&value).unwrap()).unwrap();
    std::fs::write(&swift, shot("2001:01:01 00:00:00", 9)).unwrap();
    assert_eq!(sidecar::read(&swift, false, false).unwrap().label, Medal::Bronze);
    // Twin: a second stray takes the next free name, the first one untouched.
    std::fs::write(&photo, shot("2027:01:01 00:00:00", 11)).unwrap();
    sidecar::set_label(Medal::Gold, &photo, false).unwrap();
    assert!(dir.join("DSC_0001.NEF (orphan 2).json").exists() && aside.exists());
}

#[test]
fn a_name_too_long_for_its_sidecar_says_so() {
    let dir = scratch("long");
    // 251 characters: the photo's own name fits, `.json` after it would not.
    let photo = dir.join(format!("{}.tif", "a".repeat(247)));
    std::fs::write(&photo, shot("2026:10:25 09:00:00", 1)).unwrap();
    let refused = sidecar::set_label(Medal::Gold, &photo, false);
    assert!(matches!(refused, Err(sidecar::Error::TooLong)), "{refused:?}");
    // Twin: four characters shorter, its sidecar is written.
    let fits = dir.join(format!("{}.tif", "a".repeat(246)));
    std::fs::write(&fits, shot("2026:10:25 09:00:00", 1)).unwrap();
    sidecar::set_label(Medal::Gold, &fits, false).unwrap();
    assert!(url_for(&fits).exists());
}
