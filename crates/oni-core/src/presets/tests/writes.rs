//! A preset's file written whole or not at all, each save through a temporary of its own.

use super::*;
use std::cell::RefCell;

/// The names in `dir`, sorted.
fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

fn refused(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::from(io::ErrorKind::PermissionDenied))
}

#[test]
fn a_save_under_a_refused_rename_keeps_the_old_file_and_leaves_no_temporary() {
    let dir = folder("refused-rename");
    let preset = sample();
    save(&preset, &dir).unwrap();
    let path = dir.join(file_name(&preset));
    let before = std::fs::read(&path).unwrap();
    let mut edited = preset.clone();
    edited.settings.sharpen.amount = 0.25;
    assert!(saved(&edited, &dir, &refused).is_err());
    assert_eq!((std::fs::read(&path).unwrap(), listing(&dir)), (before.clone(), vec![file_name(&preset)]));
    // Twin: the same save through a rename that lands changes the file the check above found unchanged.
    saved(&edited, &dir, &oni_settings::atomic::renamed).unwrap();
    assert_ne!(std::fs::read(&path).unwrap(), before);
    assert_eq!(listing(&dir), vec![file_name(&preset)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn two_saves_of_one_preset_never_share_a_temporary() {
    let dir = folder("two-saves");
    let preset = sample();
    let mut later = preset.clone();
    later.settings.sharpen.amount = 0.25;
    // The second save runs while the first's temporary waits for its rename.
    let seen = RefCell::new(Vec::new());
    let first = |from: &Path, to: &Path| {
        seen.borrow_mut().push(from.to_path_buf());
        saved(&later, &dir, &|f, t| {
            seen.borrow_mut().push(f.to_path_buf());
            oni_settings::atomic::renamed(f, t)
        })?;
        oni_settings::atomic::renamed(from, to)
    };
    saved(&preset, &dir, &first).unwrap();
    let seen = seen.into_inner();
    assert_eq!(seen.len(), 2);
    assert_ne!(seen[0], seen[1]);
    assert_eq!(load(&dir), [preset.clone()], "the first save, renamed last, stands whole");
    assert_eq!(listing(&dir), [file_name(&preset)]);
    // Twin: the same two saves with the first's temporary taken from under it: the first fails, the later stands.
    let lost = |from: &Path, to: &Path| {
        saved(&later, &dir, &oni_settings::atomic::renamed)?;
        std::fs::remove_file(from)?;
        oni_settings::atomic::renamed(from, to)
    };
    assert!(saved(&preset, &dir, &lost).is_err());
    assert_eq!(load(&dir)[0].settings.sharpen.amount, 0.25, "the first save is lost");
    let _ = std::fs::remove_dir_all(&dir);
}
