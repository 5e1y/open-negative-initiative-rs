//! Gestures on photos whose originals are not on disk: no sidecar is written beside nothing, no step
//! is recorded for them, and the gesture says which were left out.

use oni_core::knob::Knob;
use oni_core::{Archive, Command, EditorStore, Surface};
use oni_settings::sidecar::{self, url_for};
use oni_settings::{Medal, PipelineSettings};
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oni-core-missing-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn side(path: &str) -> Option<Vec<u8>> {
    std::fs::read(url_for(Path::new(path))).ok()
}

fn edited() -> PipelineSettings {
    let mut s = PipelineSettings::default();
    s.print.exposure = 0.7;
    s
}

#[test]
fn a_gesture_on_a_missing_original_writes_no_sidecar() {
    let dir = scratch("gestures");
    let [open, present, moved] = ["O.tif", "A.tif", "B.tif"].map(|n| text(&dir.join(n)));
    let away = text(&dir.join("Away").join("C.tif"));
    for p in [&open, &present, &moved] {
        std::fs::write(p, b"").unwrap();
    }
    sidecar::write(&edited(), Path::new(&present), Default::default()).unwrap();
    // The Finder moved B's original and left its sidecar: rewriting it would make an orphan.
    sidecar::write(&edited(), Path::new(&moved), Default::default()).unwrap();
    std::fs::remove_file(&moved).unwrap();
    let orphan = side(&moved);
    let targets = [present.clone(), moved.clone(), away.clone()];
    let untouched = |store: &mut EditorStore, gesture: &str| {
        assert_eq!(side(&moved), orphan, "{gesture}: the orphan sidecar is left as it was");
        assert_eq!(side(&away), None, "{gesture}: no sidecar beside nothing");
        let said = store.failure.take().unwrap_or_default();
        assert!(said.contains("2 photos were left out") && said.contains("cannot be found"), "{gesture}: {said}");
    };
    let mut store = EditorStore::new(Archive::new(vec![open.clone(), present.clone(), moved.clone(), away.clone()], |_: &Path| false));
    store.open(&open).unwrap();

    assert_eq!(store.reset_frames(&targets), 1);
    untouched(&mut store, "reset");
    assert_ne!(sidecar::read(Path::new(&present), false, true).unwrap().settings.print.exposure, 0.7, "reset");
    store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.5 }, 0.0);
    store.copy_settings();
    assert_eq!(store.paste_settings(&targets), 1);
    untouched(&mut store, "paste");
    assert_eq!(store.move_to_trash(&targets), 1);
    let said = store.failure.take().unwrap_or_default();
    assert!(said.contains("2 photos were not moved to the ONI Trash") && said.contains("cannot be found"), "{said}");
    for path in [&moved, &away] {
        assert!(matches!(sidecar::set_label(Medal::Gold, Path::new(path), false), Err(sidecar::Error::Unreachable)));
    }
    assert_eq!((side(&moved), side(&away)), (orphan.clone(), None), "medal: nothing written beside nothing");

    // Twin: the present photo took every gesture in its sidecar.
    sidecar::set_label(Medal::Gold, Path::new(&present), false).unwrap();
    let read = sidecar::read(Path::new(&present), false, true).unwrap();
    assert_eq!((read.label, read.trashed_at.is_some(), read.settings.vignette.stops), (Medal::Gold, true, 0.5));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_restore_on_a_disk_away_changes_nothing_and_says_so() {
    let dir = scratch("restore");
    let disk = dir.join("T7");
    std::fs::create_dir_all(disk.join("Roll")).unwrap();
    let frames: Vec<String> = ["P1.RW2", "P2.RW2"].iter().map(|n| text(&disk.join("Roll").join(n))).collect();
    frames.iter().for_each(|f| std::fs::write(f, b"").unwrap());
    let mut store = EditorStore::new(Archive::new(frames.clone(), |_: &Path| true));
    assert_eq!(store.move_to_trash(&frames), 2);
    let trashed: Vec<Option<Vec<u8>>> = frames.iter().map(|f| side(f)).collect();
    let label = store.gallery_history().undo_label().map(str::to_owned);

    std::fs::rename(&disk, dir.join("T7 away")).unwrap();
    assert_eq!(store.restore_from_trash(&frames), 0);
    let said = store.failure.take().unwrap_or_default();
    assert!(said.starts_with("2 photos were not restored") && said.contains("cannot be found"), "{said}");
    assert_eq!(store.gallery_history().undo_label().map(str::to_owned), label, "no Restore step is recorded");
    assert!(!disk.exists(), "nothing is made where the disk was");
    // Undoing the trash while the disk is away writes nothing either.
    store.undo(Surface::Gallery);
    assert!(store.failure.take().is_some_and(|s| s.contains("cannot be found")));
    std::fs::rename(dir.join("T7 away"), &disk).unwrap();
    assert_eq!(frames.iter().map(|f| side(f)).collect::<Vec<_>>(), trashed, "the marks wait on the disk as they were");

    // Twin: the disk back, the same restore lands and opens its step.
    assert_eq!(store.restore_from_trash(&frames), 2);
    assert_eq!((store.failure.take(), store.gallery_history().undo_label()), (None, Some("Restore 2 Photos")));
    assert!(frames.iter().all(|f| sidecar::read(Path::new(f), true, true).unwrap().trashed_at.is_none()));
    let _ = std::fs::remove_dir_all(&dir);
}
