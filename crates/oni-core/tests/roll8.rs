//! The editing model on roll #8's own sidecars, copied to a folder of the test's own: the reference
//! roll is read, never written.

use oni_core::balance::{Outcome, Strategy};
use oni_core::{Archive, Channel, Command, EditorStore, Family, FieldSet, Knob, PasteCheck, Scan, Surface};
use oni_settings::{PipelineSettings, sidecar};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

struct Roll {
    dir: PathBuf,
    frames: Vec<String>,
}

impl Drop for Roll {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Roll {
    fn copy(label: &str) -> Option<Roll> {
        let source = oni_fixtures::raw_dir(&format!("roll #8 {label}"))?;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("oni-core-{label}-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut frames = Vec::new();
        for entry in std::fs::read_dir(&source).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            // The roll's frames and its marker; a contact sheet's sidecar is no frame of the roll.
            if !name.ends_with(".NEF.json") && name != ".oni-roll.json" {
                continue;
            }
            std::fs::copy(entry.path(), dir.join(&name)).unwrap();
            // An empty placeholder for each NEF: no sidecar is ever written beside a missing original.
            if let Some(original) = name.strip_suffix(".json").filter(|_| !name.starts_with('.')) {
                std::fs::write(dir.join(original), b"").unwrap();
                frames.push(dir.join(original).to_string_lossy().into_owned());
            }
        }
        frames.sort();
        Some(Roll { dir, frames })
    }

    fn store(&self) -> EditorStore {
        EditorStore::new(Archive::new(self.frames.clone(), |p: &Path| p.extension().is_some_and(|e| e == "NEF")))
    }

    /// Opens a frame on the face its sidecar was left on, so opening alone rewrites nothing.
    fn open(&self, store: &mut EditorStore, frame: &str) {
        store.scanner_face = self.settings(frame).face == oni_settings::Face::Scanner;
        store.open(frame).unwrap();
    }

    fn bytes(&self) -> BTreeMap<String, Vec<u8>> {
        self.frames.iter().map(|f| (f.clone(), std::fs::read(sidecar::url_for(Path::new(f))).unwrap())).collect()
    }

    /// Every file of the copy but the sidecars, by name and bytes.
    fn originals(&self) -> BTreeMap<String, Vec<u8>> {
        std::fs::read_dir(&self.dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| !n.ends_with(".json"))
            .map(|n| (n.clone(), std::fs::read(self.dir.join(&n)).unwrap()))
            .collect()
    }

    fn settings(&self, frame: &str) -> PipelineSettings {
        sidecar::read(Path::new(frame), true, true).unwrap().settings
    }
}

/// The frames whose sidecar bytes differ between two snapshots.
fn moved(a: &BTreeMap<String, Vec<u8>>, b: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    a.keys().filter(|k| a[*k] != b[*k]).cloned().collect()
}

#[test]
fn every_sidecar_of_the_roll_rewrites_to_its_own_bytes() {
    let Some(roll) = Roll::copy("rewrite") else { return };
    let before = roll.bytes();
    for frame in &roll.frames {
        let loaded = sidecar::read(Path::new(frame), true, true).unwrap();
        sidecar::write(&loaded.settings, Path::new(frame), sidecar::WriteOptions::default()).unwrap();
    }
    assert_eq!(moved(&before, &roll.bytes()), Vec::<String>::new());
    // Twin: one moved value is seen as a moved file.
    let first = &roll.frames[0];
    let mut s = roll.settings(first);
    s.print.exposure += 0.01;
    sidecar::write(&s, Path::new(first), sidecar::WriteOptions::default()).unwrap();
    assert_eq!(moved(&before, &roll.bytes()), vec![first.clone()]);
}

#[test]
fn a_roll_settings_gesture_writes_every_frame_of_the_roll_as_one_step_and_undoes_byte_for_byte() {
    let Some(roll) = Roll::copy("roll-wide") else { return };
    let original = roll.bytes();
    let open = roll.frames[3].clone();
    let mut store = roll.store();
    roll.open(&mut store, &open);
    let id = store.settings().roll_reference.as_ref().unwrap().id;
    let on_roll = store.measured_with(id);
    assert_eq!(on_roll.len(), roll.frames.len());

    let value = store.settings().gains.stops[0] + 0.25;
    let applied = store.apply_at(Command::Set { knob: Knob::Gain(Channel::Red), value }, 0.0);
    assert_eq!(applied.changed.iter().collect::<Vec<_>>(), vec![oni_core::Field::Gains]);
    store.flush();
    let gestured = roll.bytes();
    assert_eq!(moved(&original, &gestured).len(), roll.frames.len());
    for frame in &roll.frames {
        assert_eq!(roll.settings(frame).gains.stops[0], value, "{frame}");
    }

    assert!(store.can_undo(Surface::Editor));
    store.undo(Surface::Editor);
    assert!(!store.can_undo(Surface::Editor), "one gesture, one step");
    store.flush();
    assert_eq!(moved(&original, &roll.bytes()), Vec::<String>::new());

    store.redo(Surface::Editor);
    store.flush();
    assert_eq!(moved(&gestured, &roll.bytes()), Vec::<String>::new());

    // Twin: a look field on the same frame writes the open frame alone.
    store.undo(Surface::Editor);
    store.flush();
    store.apply_at(Command::Step { knob: Knob::Exposure, by: 0.3 }, 10.0);
    store.flush();
    assert_eq!(moved(&original, &roll.bytes()), vec![open]);
}

#[test]
fn a_drag_is_one_step_and_one_roll_write() {
    let Some(roll) = Roll::copy("drag") else { return };
    let original = roll.bytes();
    let mut store = roll.store();
    roll.open(&mut store, &roll.frames[0]);
    let start = store.settings().density_ceiling;
    store.hold();
    for i in 1..=12 {
        let applied = store.apply_at(Command::Step { knob: Knob::DensityCeiling, by: 0.01 }, i as f64 * 0.016);
        assert!(applied.changed.contains(oni_core::Field::DensityCeiling));
    }
    // Under the hand nothing reaches the other frames.
    assert_eq!(moved(&original, &roll.bytes()), Vec::<String>::new());
    store.release_at(1.0);
    let end = store.settings().density_ceiling;
    assert!((end - start - 0.12).abs() < 1e-4, "{start} {end}");
    assert_eq!(roll.settings(&roll.frames[5]).density_ceiling, end);
    store.undo(Surface::Editor);
    assert!(!store.can_undo(Surface::Editor));
    assert_eq!(roll.bytes()[&roll.frames[5]], original[&roll.frames[5]]);

    // Twin: the same notches spaced past the coalescing window are steps of their own.
    store.apply_at(Command::Step { knob: Knob::DensityCeiling, by: 0.01 }, 5.0);
    store.apply_at(Command::Step { knob: Knob::DensityCeiling, by: 0.01 }, 6.0);
    store.undo(Surface::Editor);
    assert!(store.can_undo(Surface::Editor));
}

#[test]
fn a_paste_carries_the_color_and_details_tabs_whole_and_undoes_byte_for_byte() {
    let Some(roll) = Roll::copy("paste") else { return };
    let original = roll.bytes();
    let mut store = roll.store();
    roll.open(&mut store, &roll.frames[0]);
    store.apply_at(Command::Set { knob: Knob::Exposure, value: 0.4 }, 0.0);
    store.apply_at(Command::Set { knob: Knob::SharpenAmount, value: 2.2 }, 1.0);
    store.apply_at(Command::Set { knob: Knob::Angle, value: 1.5 }, 2.0);
    store.flush();
    store.copy_settings();
    let donor = store.settings().clone();
    let snapshot = roll.bytes();
    let targets: Vec<String> = roll.frames[10..14].to_vec();
    assert_eq!(store.paste_check(&targets), PasteCheck::Ready);
    let befores: Vec<PipelineSettings> = targets.iter().map(|t| roll.settings(t)).collect();
    assert_eq!(store.paste_settings(&targets), 4);
    for (target, before) in targets.iter().zip(&befores) {
        let after = roll.settings(target);
        let moved = FieldSet::between(before, &after);
        assert!(!moved.is_empty());
        assert!(moved.iter().all(|f| f.family() == Some(Family::Look)), "{moved:?}");
        assert_eq!((after.print, after.sharpen), (donor.print, donor.sharpen));
        // Twin: the donor's framing differs, and stays behind.
        assert_ne!(after.geometry, donor.geometry);
    }
    store.undo(Surface::Gallery);
    assert_eq!(moved(&snapshot, &roll.bytes()), Vec::<String>::new());
    store.redo(Surface::Gallery);
    let mut expected = targets.clone();
    expected.sort();
    assert_eq!(moved(&snapshot, &roll.bytes()), expected);
    let _ = original;
}

#[test]
fn paste_to_roll_is_one_editor_step_over_the_whole_roll() {
    let Some(roll) = Roll::copy("paste-roll") else { return };
    let mut store = roll.store();
    roll.open(&mut store, &roll.frames[1]);
    store.apply_at(Command::Set { knob: Knob::Texture, value: 0.6 }, 0.0);
    store.copy_settings();
    store.close();
    let snapshot = roll.bytes();
    roll.open(&mut store, &roll.frames[20]);
    store.paste_to_roll();
    store.flush();
    for frame in &roll.frames {
        assert_eq!(roll.settings(frame).texture.amount, 0.6, "{frame}");
    }
    store.undo(Surface::Editor);
    store.flush();
    let mut back = moved(&snapshot, &roll.bytes());
    back.retain(|f| *f != roll.frames[1]);
    assert_eq!(back, Vec::<String>::new());
    // Twin: a paste onto another roll's frame asks first.
    let mut foreign = roll.settings(&roll.frames[2]);
    foreign.roll_reference.as_mut().unwrap().id = oni_settings::Id::new();
    sidecar::write(&foreign, Path::new(&roll.frames[2]), sidecar::WriteOptions::default()).unwrap();
    assert_eq!(store.paste_check(&[roll.frames[2].clone()]), PasteCheck::AnotherRoll);
}

#[test]
fn the_balance_key_writes_only_on_a_press_and_one_undo_takes_it_back() {
    let Some(roll) = Roll::copy("balance") else { return };
    let mut store = roll.store();
    let open = roll.frames[0].clone();
    roll.open(&mut store, &open);
    store.flush();
    let snapshot = roll.bytes();
    let reference = store.settings().roll_reference.clone().unwrap();
    let reads = std::sync::atomic::AtomicUsize::new(0);
    let measure = |_: &str, _: &PipelineSettings| {
        reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Some(Outcome { reference: reference.clone(), filters: [0.3, 0.0, -0.3] })
    };
    // Opening and editing read nothing: the key alone measures.
    store.apply_at(Command::Step { knob: Knob::Exposure, by: 0.1 }, 0.0);
    assert_eq!(reads.load(std::sync::atomic::Ordering::Relaxed), 0);
    let targets = vec![open.clone(), roll.frames[1].clone(), roll.frames[2].clone()];
    assert_eq!(store.auto_balance(&targets, Strategy::Each, &measure), None);
    assert_eq!(reads.load(std::sync::atomic::Ordering::Relaxed), 3);
    assert_eq!(store.settings().balance_filters, Some([0.3, 0.0, -0.3]));
    assert_eq!(roll.settings(&roll.frames[2]).balance_filters, Some([0.3, 0.0, -0.3]));
    store.undo(Surface::Editor);
    assert_eq!(roll.bytes()[&roll.frames[2]], snapshot[&roll.frames[2]]);
    assert_ne!(store.settings().balance_filters, Some([0.3, 0.0, -0.3]));
    // Twin: an unreadable frame stops the run there, and what read before it stays written.
    let blind = |path: &str, s: &PipelineSettings| if path == roll.frames[1] { None } else { measure(path, s) };
    assert!(store.auto_balance(&targets, Strategy::Each, &blind).is_some());
    assert_eq!(roll.bytes()[&roll.frames[2]], snapshot[&roll.frames[2]]);
}

#[test]
fn the_trash_marks_the_sidecar_and_never_moves_the_file() {
    let Some(roll) = Roll::copy("trash") else { return };
    let frame = roll.frames[7].clone();
    for other in &roll.frames {
        std::fs::write(other, format!("stand-in for {other}")).unwrap();
    }
    let mut store = roll.store();
    roll.open(&mut store, &frame);
    store.flush();
    let (before, files, settings) = (roll.bytes(), roll.originals(), roll.settings(&frame));
    assert_eq!(store.move_to_trash(std::slice::from_ref(&frame)), 1);
    assert!(store.path().is_none(), "the open frame closes");
    assert!(sidecar::read(Path::new(&frame), true, true).unwrap().trashed_at.is_some());
    assert_eq!(moved(&before, &roll.bytes()), [frame.clone()], "the marked frame's sidecar alone is written");
    assert_eq!(roll.originals(), files, "no original moves, vanishes or changes, nor any other member's");
    store.undo(Surface::Gallery);
    assert!(sidecar::read(Path::new(&frame), true, true).unwrap().trashed_at.is_none());
    assert_eq!(roll.settings(&frame), settings, "the mark and its removal leave the settings untouched");
    // Twin: a member's original moved away is seen, so a trash that moved a file would fail above.
    std::fs::rename(&roll.frames[3], roll.dir.join("moved.bin")).unwrap();
    assert_ne!(roll.originals(), files);
}

#[test]
fn the_bars_reset_keeps_framing_flat_field_and_roll() {
    let Some(roll) = Roll::copy("reset") else { return };
    let mut store = roll.store();
    // On the Pro face, whatever face the frame was left on.
    store.open(&roll.frames[4]).unwrap();
    store.apply_at(Command::Set { knob: Knob::Angle, value: 2.0 }, 0.0);
    store.apply_at(Command::Set { knob: Knob::SharpenAmount, value: 2.5 }, 1.0);
    store.apply_at(Command::Scan(Scan::Step { letter: oni_keys::Letter::D, by: 3 }), 2.0);
    let edited = store.settings().clone();
    store.apply_at(Command::Reset, 3.0);
    let blank = store.settings().clone();
    assert_eq!((blank.geometry, blank.flat_field.clone()), (edited.geometry, edited.flat_field.clone()));
    assert_eq!(blank.roll_reference, edited.roll_reference);
    assert_eq!(blank.scanner, edited.scanner, "the Pro face's reset leaves the scanner's clicks");
    assert_ne!(blank.sharpen, edited.sharpen);
    // Twin: on the Scanner face the clicks are the edit, and go.
    store.undo(Surface::Editor);
    store.scanner_face = true;
    store.apply_at(Command::Reset, 10.0);
    assert_eq!(store.settings().scanner.d, 0);
}
