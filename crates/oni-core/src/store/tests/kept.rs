use super::*;
use oni_library::gallery_history::Move;

/// A folder of empty frames, each with the settings given, and a store over them, none open.
fn frames(label: &str, sidecars: &[(&str, Option<PipelineSettings>)]) -> (EditorStore, std::path::PathBuf, Vec<String>) {
    let dir = std::env::temp_dir().join(format!("oni-core-store-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let paths: Vec<String> = sidecars.iter().map(|(n, _)| dir.join(n).to_string_lossy().into_owned()).collect();
    for (path, (_, settings)) in paths.iter().zip(sidecars) {
        std::fs::write(path, b"").unwrap();
        if let Some(settings) = settings {
            sidecar::write(settings, Path::new(path), WriteOptions::default()).unwrap();
        }
    }
    (EditorStore::new(Archive::new(paths.clone(), |_: &Path| false)), dir, paths)
}

fn read(path: &str) -> PipelineSettings {
    sidecar::read(Path::new(path), false, true).unwrap().settings
}

fn vignetted(stops: f32) -> PipelineSettings {
    let mut s = PipelineSettings::default();
    s.vignette.stops = stops;
    s
}

/// A paste on the open frame from the gallery, a gallery undo when `undone`, then an edit and its
/// undo in the editor; the vignette the frame is left with.
fn pasted_then_edited(label: &str, undone: bool) -> f32 {
    let (mut store, dir) = fresh(label);
    let frame = dir.join("frame.tif").to_string_lossy().into_owned();
    let rest = Knob::Vignette.get(store.settings());
    store.board.settings = Some(codec::encode_settings(&vignetted(-0.7)).unwrap());
    assert_eq!(store.paste_settings(std::slice::from_ref(&frame)), 1);
    assert_eq!(Knob::Vignette.get(store.settings()), -0.7);
    if undone {
        store.undo(Surface::Gallery);
        assert_eq!(Knob::Vignette.get(store.settings()), rest, "the gallery undo lays the frame back");
    }
    store.apply_at(Command::Set { knob: Knob::Texture, value: 0.3 }, 10.0);
    store.undo(Surface::Editor);
    assert_eq!(Knob::Texture.get(store.settings()), Knob::Texture.get(&PipelineSettings::default()));
    let _ = std::fs::remove_dir_all(&dir);
    Knob::Vignette.get(store.settings()) - rest
}

#[test]
fn a_paste_undone_in_the_gallery_never_comes_back_with_an_editor_undo() {
    assert_eq!(pasted_then_edited("paste-undone", true), 0.0, "the frame stays at its rest");
    // Twin: the paste left standing is where the editor undo of the edit lands.
    assert_ne!(pasted_then_edited("paste-kept", false), 0.0);
}

#[test]
fn ungroup_takes_the_open_frame_in_its_gallery_step_and_its_editor_step() {
    let film = oni_settings::Film::new("Roll 3");
    let grouped = PipelineSettings { film: Some(film.clone()), ..PipelineSettings::default() };
    let (mut store, dir, paths) = frames("ungroup", &[("a.tif", Some(grouped.clone())), ("b.tif", Some(grouped.clone()))]);
    let [a, b] = [paths[0].clone(), paths[1].clone()];
    store.open(&a).unwrap();
    store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.5 }, 0.0);
    assert_eq!(store.apply_film(&paths, None, "Ungroup"), 2);
    assert_eq!(store.gallery_history().undo_label(), Some("Ungroup · 2 photos"));
    assert!(store.settings().film.is_none() && read(&b).film.is_none());
    store.undo(Surface::Gallery);
    assert_eq!(store.settings().film.as_ref(), Some(&film), "the open frame is regrouped");
    assert_eq!(read(&b).film.as_ref(), Some(&film));
    // Twin: an editor undo takes a film change back alone, the edit before it kept.
    store.redo(Surface::Gallery);
    assert!(store.settings().film.is_none());
    store.set_film(std::slice::from_ref(&a), Some(film.clone()), "Set Film Stock");
    store.undo(Surface::Editor);
    assert!(store.settings().film.is_none(), "one editor undo takes the film back");
    assert_eq!(Knob::Vignette.get(store.settings()), 0.5, "and nothing before it");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The open frame edited, its sidecar refused when `refused`, then sent to the ONI Trash.
fn trashed_after(label: &str, refused: bool) -> (EditorStore, std::path::PathBuf, String) {
    let (mut store, dir) = fresh(label);
    let frame = dir.join("frame.tif").to_string_lossy().into_owned();
    store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.5 }, 0.0);
    if refused {
        // A folder where the sidecar goes: every write of it is refused, as a lock would.
        std::fs::create_dir(sidecar::url_for(Path::new(&frame))).unwrap();
    }
    store.mark_begun(std::slice::from_ref(&frame), Some(Date::now()));
    assert!(store.path().is_none(), "the frame closes as it goes");
    (store, dir, frame)
}

#[test]
fn a_frame_trashed_after_a_refused_write_keeps_its_edits_unsaved() {
    let (store, dir, frame) = trashed_after("trash-refused", true);
    let kept: Vec<_> = store.unsaved().iter().map(|u| (u.path.clone(), Knob::Vignette.get(&u.settings))).collect();
    assert_eq!(kept, [(frame, 0.5)]);
    let _ = std::fs::remove_dir_all(&dir);
    // Twin: a write taken keeps nothing back, the sidecar holding the edit.
    let (store, dir, frame) = trashed_after("trash-written", false);
    assert!(store.unsaved().is_empty());
    assert_eq!(Knob::Vignette.get(&read(&frame)), 0.5);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn edits_kept_unsaved_and_the_open_frame_follow_a_move() {
    let (mut store, dir, a, _) = edited_then_switched("follow", true);
    let moved = dir.join("T7 away/Roll/A.tif").to_string_lossy().into_owned();
    let away = Move { from: dir.join("T7").to_string_lossy().into_owned(), to: dir.join("T7 away").to_string_lossy().into_owned() };
    store.follow(std::slice::from_ref(&away));
    assert_eq!(store.unsaved()[0].path, moved);
    store.open(&moved).unwrap();
    assert_eq!(Knob::Vignette.get(store.settings()), 0.5, "the moved photo opens with its edits");
    assert!(store.unsaved().is_empty(), "and takes them as its own");
    // The open frame follows too: its next write lands beside the photo where it now is.
    std::fs::rename(dir.join("T7 away"), dir.join("T7")).unwrap();
    store.follow(&[Move { from: away.to.clone(), to: away.from.clone() }]);
    assert_eq!(store.path(), Some(a.as_str()));
    store.flush();
    assert_eq!(Knob::Vignette.get(&read(&a)), 0.5);
    let _ = std::fs::remove_dir_all(&dir);
    // Twin: unfollowed, the moved photo opens without them, kept under the old path.
    let (mut store, dir, _, _) = edited_then_switched("unfollowed", true);
    let moved = dir.join("T7 away/Roll/A.tif").to_string_lossy().into_owned();
    store.open(&moved).unwrap();
    assert_eq!(Knob::Vignette.get(store.settings()), 0.0);
    assert_eq!(store.unsaved().len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_undo_within_the_settle_delay_takes_back_the_step_still_waiting() {
    let (mut store, dir) = fresh("undo-waiting");
    store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.5 }, 0.0);
    store.defer_at(1.0);
    store.apply_at(Command::Set { knob: Knob::Texture, value: 0.3 }, 1.0);
    store.undo(Surface::Editor);
    assert_eq!(Knob::Vignette.get(store.settings()), 0.5, "the step before stays");
    assert_eq!(Knob::Texture.get(store.settings()), Knob::Texture.get(&PipelineSettings::default()));
    // Twin: the redo brings the waiting edit back, never lost.
    store.redo(Surface::Editor);
    assert_eq!(Knob::Texture.get(store.settings()), 0.3);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_gesture_on_several_leaves_an_open_frame_out_of_sight_alone() {
    let dir = std::env::temp_dir().join(format!("oni-core-store-inert-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let gone = dir.join("Unplugged/P1.tif").to_string_lossy().into_owned();
    let present = dir.join("P2.tif").to_string_lossy().into_owned();
    std::fs::write(&present, b"").unwrap();
    let mut store = EditorStore::new(Archive::new(vec![gone.clone(), present.clone()], |_: &Path| false));
    store.board.settings = Some(codec::encode_settings(&vignetted(-0.7)).unwrap());
    let _ = store.open(&gone);
    assert!(store.inert());
    let rest = Knob::Vignette.get(store.settings());
    store.paste_settings(&[gone.clone(), present.clone()]);
    store.paste_to_roll();
    store.set_film(std::slice::from_ref(&gone), Some(oni_settings::Film::new("Roll 3")), "Set Film Stock");
    assert_eq!(Knob::Vignette.get(store.settings()), rest, "a paste never shows on a frame it cannot save");
    assert!(store.settings().film.is_none());
    assert_eq!(store.gallery_history().undo_paths(), BTreeSet::from([present.clone()]), "only the frame in sight is in the step");
    // Twin: the frame in sight, open, takes the paste.
    store.open(&present).unwrap();
    store.paste_settings(std::slice::from_ref(&present));
    assert_eq!(Knob::Vignette.get(store.settings()), -0.7);
    let _ = std::fs::remove_dir_all(&dir);
}
