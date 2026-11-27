use super::*;

/// A frame edited while its disk is away waits, unwritten, for the very original it opened on; another file standing
/// at its path, a disk of the same name, takes nothing.
#[test]
fn edits_a_refused_write_left_wait_for_the_original_they_were_made_on() {
    let dir = std::env::temp_dir().join(format!("oni-core-store-unwritten-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (folder, away) = (dir.join("Roll"), dir.join("Roll away"));
    std::fs::create_dir_all(&folder).unwrap();
    let photo = folder.join("P1.RW2").to_string_lossy().into_owned();
    std::fs::write(&photo, b"raw").unwrap();
    let mut store = EditorStore::new(Archive::new(vec![photo.clone()], |_: &Path| true));
    store.open(&photo).unwrap();
    assert!(!store.unwritten(), "a frame just opened holds nothing unwritten");
    std::fs::rename(&folder, &away).unwrap();
    store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.5 }, 0.0);
    assert!(store.flush().is_some(), "the write is refused while the disk is away");
    assert!(!store.unwritten(), "nothing is written beside an original out of sight");
    std::fs::rename(&away, &folder).unwrap();
    assert!(store.unwritten(), "back, the edit waits for its write");
    assert_eq!(store.flush(), None);
    assert!(!store.unwritten(), "written, nothing waits");
    // Twin: another file at the path never takes the edits.
    store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.2 }, 0.0);
    assert!(store.unwritten());
    std::fs::write(&photo, b"another photo").unwrap();
    assert!(!store.unwritten(), "another file at the path takes nothing");
    let before = std::fs::read(sidecar::url_for(Path::new(&photo))).unwrap();
    assert_eq!(store.flush().as_deref(), Some(tr(super::opening::ANOTHER)), "its write is refused");
    assert_eq!(std::fs::read(sidecar::url_for(Path::new(&photo))).unwrap(), before, "and nothing is written beside it");
    let _ = std::fs::remove_dir_all(&dir);
}
