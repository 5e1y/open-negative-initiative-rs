use super::*;
use crate::fields::Field;
use crate::knob::Knob;

/// What a gesture leaves with the open frame: its undo steps, its kept edits, its moves.
mod kept;

fn fresh(label: &str) -> (EditorStore, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("oni-core-store-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let frame = dir.join("frame.tif").to_string_lossy().into_owned();
    std::fs::write(&frame, b"").unwrap();
    let mut store = EditorStore::new(Archive::new(vec![frame.clone()], |_: &Path| false));
    store.open(&frame).unwrap();
    (store, dir)
}

#[test]
fn a_notch_reports_its_field_once_and_an_unmoved_value_reports_nothing() {
    let (mut store, dir) = fresh("changes");
    store.take_changes();
    let revision = store.revision();
    store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.5 }, 0.0);
    assert_eq!(store.take_changes().iter().collect::<Vec<_>>(), vec![Field::Vignette]);
    assert!(store.take_changes().is_empty());
    assert_eq!(store.revision(), revision + 1);
    // Twin: the same value again moves nothing, bumps nothing and opens no step.
    let applied = store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.5 }, 5.0);
    assert!(applied.changed.is_empty() && store.take_changes().is_empty());
    assert_eq!(store.revision(), revision + 1);
    store.undo(Surface::Editor);
    assert!(!store.can_undo(Surface::Editor));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Two presses on two settings, the second landing `gap` seconds after the first lifts.
fn two_presses(store: &mut EditorStore, gap: f64) {
    store.hold();
    store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.5 }, 0.0);
    store.release_deferred_at(0.1);
    let pressed = 0.1 + gap;
    let _ = store.settle_if_due_at(pressed);
    store.hold();
    store.apply_at(Command::Set { knob: Knob::Texture, value: 0.3 }, pressed);
    store.release_deferred_at(pressed + 0.05);
    assert!(store.settle_if_due_at(pressed + 0.05 + SETTLE_DELAY - 0.01).is_some(), "still waiting");
    assert_eq!(store.settle_if_due_at(pressed + 0.05 + SETTLE_DELAY + 0.01), None);
}

#[test]
fn a_press_within_the_settle_delay_joins_the_step_before_it() {
    let (mut store, dir) = fresh("settle");
    let rest = (Knob::Vignette.get(store.settings()), Knob::Texture.get(store.settings()));
    two_presses(&mut store, 0.1);
    store.undo(Surface::Editor);
    assert_eq!((Knob::Vignette.get(store.settings()), Knob::Texture.get(store.settings())), rest, "one step for both presses");
    assert!(!store.can_undo(Surface::Editor));
    // Twin: the second press after the delay makes a step of its own.
    let (mut apart, dir2) = fresh("settle-apart");
    two_presses(&mut apart, SETTLE_DELAY + 0.05);
    apart.undo(Surface::Editor);
    assert_eq!((Knob::Vignette.get(apart.settings()), Knob::Texture.get(apart.settings())), (0.5, rest.1));
    assert!(apart.can_undo(Surface::Editor));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}

#[test]
fn an_unheld_change_moves_the_waiting_step_on_and_a_plain_release_records_it() {
    let (mut store, dir) = fresh("settle-moved");
    store.hold();
    store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.5 }, 0.0);
    store.release_deferred_at(0.0);
    store.apply_at(Command::Set { knob: Knob::Texture, value: 0.3 }, 0.1);
    assert!(store.settle_if_due_at(0.2).is_some(), "the key's change restarted the delay");
    assert!(!store.can_undo(Surface::Editor), "twin: nothing recorded before the delay");
    // An open or a slider's release records what waits, so it never reaches the next frame.
    store.release_at(0.2);
    assert!(store.can_undo(Surface::Editor));
    assert_eq!(store.settle_if_due_at(1.0), None);
    store.undo(Surface::Editor);
    assert!(!store.can_undo(Surface::Editor), "both edits are one step");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The remembered flat-field sliders are where the history starts, as `History(opened)` does.
#[test]
fn an_adjusted_opening_starts_the_history_on_the_adjusted_settings() {
    let (mut store, dir) = fresh("adjusted");
    let frame = dir.join("frame.tif").to_string_lossy().into_owned();
    store.open_adjusted(&frame, |s| s.flat_field.temperature = 0.4).unwrap();
    assert_eq!(store.settings().flat_field.temperature, 0.4);
    assert!(!store.can_undo(Surface::Editor), "the adjustment is no step of its own");
    store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.5 }, 0.0);
    store.undo(Surface::Editor);
    assert_eq!(store.settings().flat_field.temperature, 0.4, "an undo lands on the adjusted opening");
    // Twin: a plain opening of another frame reads its sidecar as it stands.
    std::fs::write(dir.join("other.tif"), b"").unwrap();
    store.open(&dir.join("other.tif").to_string_lossy()).unwrap();
    assert_eq!(store.settings().flat_field.temperature, 0.0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_untouched_frame_gains_no_sidecar_and_a_rested_edit_writes_one() {
    let (mut store, dir) = fresh("flush");
    let written = sidecar::url_for(&dir.join("frame.tif"));
    store.flush();
    assert!(!written.exists());
    store.apply_at(Command::Step { knob: Knob::Texture, by: 0.4 }, 0.0);
    assert_eq!(store.flush_if_due(0.1), None);
    assert!(!written.exists(), "a hand still moving writes nothing");
    store.flush_if_due(1.0);
    assert!(written.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_roll_set_on_its_frames_writes_the_others_at_once_keeps_their_dates_and_undoes_as_one_step() {
    let dir = std::env::temp_dir().join(format!("oni-core-store-film-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let [a, b, c] = ["a.tif", "b.tif", "c.tif"].map(|n| dir.join(n).to_string_lossy().into_owned());
    std::fs::write(&a, b"").unwrap();
    for p in [&b, &c] {
        std::fs::write(p, b"").unwrap();
        sidecar::write(&PipelineSettings::default(), Path::new(p), WriteOptions::default()).unwrap();
    }
    let stamp = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
    std::fs::File::options().write(true).open(sidecar::url_for(Path::new(&b))).unwrap().set_modified(stamp).unwrap();
    let mut store = EditorStore::new(Archive::new(vec![a.clone(), b.clone(), c.clone()], |_: &Path| false));
    store.open(&a).unwrap();
    let film = oni_settings::Film::new("Roll 3");
    let read = |p: &str| sidecar::read(Path::new(p), false, true).unwrap().settings.film;
    assert_eq!(store.set_film(&[a.clone(), b.clone()], Some(film.clone()), "Rename Roll"), 2);
    assert_eq!(store.settings().film.as_ref(), Some(&film), "the open frame takes it in memory");
    assert!(!sidecar::url_for(Path::new(&a)).exists(), "and writes it at rest, not at once");
    assert_eq!(read(&b), Some(film.clone()));
    let kept = std::fs::metadata(sidecar::url_for(Path::new(&b))).unwrap().modified().unwrap();
    assert_eq!(kept, stamp, "the revision holds, so the thumbnail key does");
    assert_eq!(read(&c), None, "twin: a frame not named keeps its roll");
    assert_eq!(store.gallery_history().undo_label(), Some("Rename Roll"));
    assert_eq!(store.set_film(&[b.clone()], Some(film.clone()), "Rename Roll"), 0, "an unchanged roll writes nothing");
    store.undo(Surface::Gallery);
    assert_eq!(read(&b), None, "one undo takes the others back");
    // Twin: `None` ungroups.
    store.set_film(&[b.clone()], Some(film.clone()), "Set Film Stock");
    store.set_film(&[b.clone()], None, "Ungroup");
    assert_eq!(read(&b), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_frame_whose_original_is_gone_opens_read_only_and_writes_nothing() {
    let dir = std::env::temp_dir().join(format!("oni-core-store-gone-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // A roll on a disk that was unplugged: neither the original nor its sidecar can be seen.
    let gone = dir.join("Unplugged").join("P1.RW2").to_string_lossy().into_owned();
    let mut store = EditorStore::new(Archive::new(vec![gone.clone()], |_: &Path| true));
    let opened = store.open(&gone);
    assert_eq!(opened.map_err(|e| e.to_string()), Err(super::opening::UNREACHABLE.to_string()));
    assert!(!store.writable(), "an unreachable frame is no new photo to write");
    store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.5 }, 0.0);
    assert_eq!(store.flush(), None);
    std::fs::create_dir_all(dir.join("Unplugged")).unwrap();
    store.flush();
    assert!(!sidecar::url_for(Path::new(&gone)).exists(), "the disk come back takes no sidecar from it");
    // Twin: an original present without a sidecar is a new photo, open to edits.
    let present = dir.join("P2.RW2").to_string_lossy().into_owned();
    std::fs::write(&present, b"").unwrap();
    store.open(&present).unwrap();
    assert!(store.writable());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A frame on a disk of its own, edited, then the disk taken away as the hand opens another photo.
fn edited_then_switched(label: &str, unplug: bool) -> (EditorStore, std::path::PathBuf, String, String) {
    let dir = std::env::temp_dir().join(format!("oni-core-store-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("T7/Roll")).unwrap();
    let a = dir.join("T7/Roll/A.tif").to_string_lossy().into_owned();
    let b = dir.join("B.tif").to_string_lossy().into_owned();
    std::fs::write(&a, b"a").unwrap();
    std::fs::write(&b, b"b").unwrap();
    let mut store = EditorStore::new(Archive::new(vec![a.clone(), b.clone()], |_: &Path| false));
    store.open(&a).unwrap();
    store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.5 }, 0.0);
    if unplug {
        std::fs::rename(dir.join("T7"), dir.join("T7 away")).unwrap();
    }
    store.open(&b).unwrap();
    (store, dir, a, b)
}

#[test]
fn a_flush_refused_on_switch_keeps_the_edits_and_asks() {
    let (mut store, dir, a, b) = edited_then_switched("unsaved", true);
    let kept: Vec<_> = store.unsaved().iter().map(|u| (u.path.clone(), Knob::Vignette.get(&u.settings))).collect();
    assert_eq!(kept, [(a.clone(), 0.5)], "the edits stay in memory, and their presence is the host's cue to ask");
    assert!(!store.unsaved()[0].message.is_empty(), "with the disk's refusal");
    assert!(store.retry_unsaved().is_some() && store.unsaved().len() == 1, "a retry while the disk is away keeps them");
    // Opened again while away, the frame shows the kept edits, read-only and deaf to every hand.
    assert!(store.open(&a).is_err());
    assert_eq!(Knob::Vignette.get(store.settings()), 0.5);
    assert!(store.inert() && !store.writable());
    store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.9 }, 1.0);
    assert_eq!(Knob::Vignette.get(store.settings()), 0.5, "a hand on a frame out of sight moves nothing");
    store.open(&b).unwrap();
    assert_eq!(store.unsaved().len(), 1, "leaving it again keeps its edits");
    // A disk of the same name holding another A.tif takes none of them.
    std::fs::create_dir_all(dir.join("T7/Roll")).unwrap();
    std::fs::write(&a, b"another photo").unwrap();
    assert_eq!(store.retry_unsaved().as_deref(), Some(super::opening::ANOTHER));
    assert!(!sidecar::url_for(Path::new(&a)).exists() && store.unsaved().len() == 1);
    std::fs::remove_dir_all(dir.join("T7")).unwrap();
    std::fs::rename(dir.join("T7 away"), dir.join("T7")).unwrap();
    assert_eq!(store.retry_unsaved(), None, "the disk back, the retry writes");
    assert!(store.unsaved().is_empty());
    let written = sidecar::read(Path::new(&a), false, true).unwrap().settings;
    assert_eq!(Knob::Vignette.get(&written), 0.5);
    // Discard drops what a refusal kept.
    store.open(&a).unwrap();
    store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.7 }, 2.0);
    std::fs::rename(dir.join("T7"), dir.join("T7 away")).unwrap();
    store.open(&b).unwrap();
    store.discard_unsaved();
    assert!(store.unsaved().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_flush_taken_on_switch_stays_silent() {
    // Twin: the disk there, the write takes the edits and nothing is kept to ask about.
    let (store, dir, a, _) = edited_then_switched("saved", false);
    assert!(store.unsaved().is_empty() && !store.inert());
    let written = sidecar::read(Path::new(&a), false, true).unwrap().settings;
    assert_eq!(Knob::Vignette.get(&written), 0.5);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The library's paths laid again keep a roll's frames found while every one is still there, so a
/// lay reads no sidecar; a frame gone has its roll scanned again on the next ask.
#[test]
fn a_roll_s_frames_found_outlive_a_lay_that_keeps_them() {
    let paths: Vec<String> = ["/r/a.tif", "/r/b.tif", "/r/c.tif"].iter().map(|p| p.to_string()).collect();
    let mut store = EditorStore::new(Archive::new(paths.clone(), |_: &Path| false));
    let (kept, lost) = (oni_settings::Id::new(), oni_settings::Id::new());
    store.measured.insert(kept, ["/r/a.tif".to_string()].into());
    store.measured.insert(lost, ["/r/c.tif".to_string()].into());
    let mut more = paths.clone();
    more.push("/r/d.tif".into());
    store.set_archive_paths(more.clone());
    assert_eq!((store.measured_group(kept).map(|m| m.len()), store.measured_group(lost).map(|m| m.len())), (Some(1), Some(1)));
    // Twin: a frame gone takes its roll's found frames with it.
    more.retain(|p| p != "/r/c.tif");
    store.set_archive_paths(more);
    assert!(store.measured_group(kept).is_some() && store.measured_group(lost).is_none());
}
