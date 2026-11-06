//! One history across frames and modes: a step taken on a frame left, or on several from the gallery,
//! is reached by the same undo wherever the hand now stands.

use super::*;
use oni_settings::Medal;

/// Two fresh frames, a and b, and a store over them, none open.
fn two(label: &str) -> (EditorStore, std::path::PathBuf, String, String) {
    let dir = std::env::temp_dir().join(format!("oni-core-store-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let [a, b] = ["a.tif", "b.tif"].map(|n| dir.join(n).to_string_lossy().into_owned());
    for p in [&a, &b] {
        std::fs::write(p, b"").unwrap();
    }
    (EditorStore::new(Archive::new(vec![a.clone(), b.clone()], |_: &Path| false)), dir, a, b)
}

fn read(path: &str) -> PipelineSettings {
    sidecar::read(Path::new(path), false, true).unwrap().settings
}

#[test]
fn an_edit_on_a_frame_left_is_undone_after_the_switch_through_its_sidecar() {
    let (mut store, dir, a, b) = two("switch");
    let rest = Knob::Vignette.get(&PipelineSettings::default());
    store.open(&a).unwrap();
    store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.5 }, 0.0);
    store.open(&b).unwrap();
    assert!(store.can_undo(), "the frame left keeps its step");
    store.apply_at(Command::Set { knob: Knob::Texture, value: 0.3 }, 1.0);
    store.undo();
    assert_eq!(Knob::Texture.get(store.settings()), Knob::Texture.get(&PipelineSettings::default()), "b's own edit first");
    assert_eq!(Knob::Vignette.get(&read(&a)), 0.5, "a untouched yet");
    store.undo();
    assert_eq!(Knob::Vignette.get(&read(&a)), rest, "then a's, written beside it");
    assert_eq!(store.path(), Some(b.as_str()), "the open frame stays where it is");
    store.redo();
    assert_eq!(Knob::Vignette.get(&read(&a)), 0.5);
    // Twin: a frame opened and left untouched lays no step between.
    store.open(&a).unwrap();
    store.open(&b).unwrap();
    store.undo();
    assert_eq!(Knob::Vignette.get(&read(&a)), rest, "one undo reaches a's edit again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_gesture_from_the_gallery_and_an_edit_in_the_editor_take_turns_on_one_history() {
    let (mut store, dir, a, b) = two("modes");
    let medal = |p: &str| sidecar::read(Path::new(p), false, true).unwrap().label;
    store.lay_medal(std::slice::from_ref(&b), Medal::Gold);
    store.open(&a).unwrap();
    store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.5 }, 0.0);
    assert_eq!(store.history().undo_label(), None, "an edit has no name, the menu saying Undo");
    store.undo();
    assert_eq!(Knob::Vignette.get(store.settings()), Knob::Vignette.get(&PipelineSettings::default()));
    assert_eq!(store.history().undo_label(), Some("Medal on 1 photo"), "the gallery's step is next, from the editor");
    store.undo();
    assert_eq!(medal(&b), Medal::None);
    assert!(!store.can_undo());
    // Twin: the redo lays them back in the order they were taken.
    store.redo();
    assert_eq!((medal(&b), Knob::Vignette.get(store.settings())), (Medal::Gold, Knob::Vignette.get(&PipelineSettings::default())));
    store.redo();
    assert_eq!(Knob::Vignette.get(store.settings()), 0.5);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_edit_still_waiting_is_recorded_before_a_gesture_lands_over_it() {
    let (mut store, dir, a, b) = two("waiting");
    store.open(&a).unwrap();
    store.hold();
    store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.5 }, 0.0);
    store.lay_medal(std::slice::from_ref(&b), Medal::Gold);
    store.undo();
    assert_eq!(Knob::Vignette.get(store.settings()), 0.5, "the medal is on top, the edit under it");
    store.undo();
    assert_eq!(Knob::Vignette.get(store.settings()), Knob::Vignette.get(&PipelineSettings::default()));
    // Twin: with nothing waiting, the medal alone is the step.
    let (mut idle, dir2, _, b2) = two("waiting-idle");
    idle.lay_medal(std::slice::from_ref(&b2), Medal::Gold);
    idle.undo();
    assert!(!idle.can_undo());
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}
