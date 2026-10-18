use super::*;
use crate::knob::Knob;

fn fresh(label: &str) -> (EditorStore, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("oni-core-store-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let frame = dir.join("frame.tif").to_string_lossy().into_owned();
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
    for p in [&b, &c] {
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
