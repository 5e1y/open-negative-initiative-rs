//! A sidecar that cannot be read is never rewritten around one field: its edits stay on disk.

use oni_settings::sidecar::{self, set_label, url_for};
use oni_settings::{Medal, PipelineSettings};

#[test]
fn an_unreadable_sidecar_is_left_as_it_was() {
    let dir = std::env::temp_dir().join(format!("oni-unreadable-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let broken = dir.join("A.NEF");
    std::fs::write(url_for(&broken), b"{\"settings\": {\"exposure\": 1.2, trunc").unwrap();
    assert!(set_label(Medal::Gold, &broken, true).is_err(), "a broken sidecar is an error");
    assert_eq!(std::fs::read(url_for(&broken)).unwrap(), b"{\"settings\": {\"exposure\": 1.2, trunc", "left untouched");
    // Twin: a readable one takes the label, its settings kept.
    let kept = dir.join("B.NEF");
    let mut s = PipelineSettings::default();
    s.print.exposure = 0.7;
    sidecar::write(&s, &kept, Default::default()).unwrap();
    set_label(Medal::Gold, &kept, true).unwrap();
    let read = sidecar::read(&kept, true, true).unwrap();
    assert_eq!((read.label, read.settings.print.exposure), (Medal::Gold, 0.7));
    let _ = std::fs::remove_dir_all(&dir);
}
