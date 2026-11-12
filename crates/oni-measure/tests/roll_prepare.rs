//! Prepare the Roll's measure and Reset to Default on roll #8's own sidecars, copied to a folder of
//! the test's own: what lands on every frame, as one gallery step that undoes to the same bytes.

use oni_core::store::roll::{WrittenFrame, measure_label, reset_label};
use oni_core::{Archive, EditorStore};
use oni_library::roll_membership;
use oni_settings::{ConversionMode, PipelineSettings, sidecar};
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
        let source = oni_fixtures::raw_dir(label)?;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("oni-core-prepare-{label}-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut frames = Vec::new();
        for entry in std::fs::read_dir(&source).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".NEF.json") && name != ".oni-roll.json" {
                continue;
            }
            std::fs::copy(entry.path(), dir.join(&name)).unwrap();
            // An empty stand-in for each NEF: no sidecar is ever written beside a missing original.
            if let Some(original) = name.strip_suffix(".json").filter(|_| !name.starts_with('.')) {
                std::fs::write(dir.join(original), b"").unwrap();
                frames.push(dir.join(original).to_string_lossy().into_owned());
            }
        }
        frames.sort();
        // Each sidecar as a write beside its stand-in leaves it, so the bytes a check compares are its gesture's.
        for frame in &frames {
            let loaded = sidecar::read(Path::new(frame), true).unwrap();
            sidecar::write(&loaded.settings, Path::new(frame), sidecar::WriteOptions::default()).unwrap();
        }
        Some(Roll { dir, frames })
    }

    fn store(&self) -> EditorStore {
        EditorStore::new(Archive::new(self.frames.clone(), |p: &Path| p.extension().is_some_and(|e| e == "NEF")))
    }

    fn bytes(&self) -> BTreeMap<String, Vec<u8>> {
        self.frames.iter().map(|f| (f.clone(), std::fs::read(sidecar::url_for(Path::new(f))).unwrap())).collect()
    }

    fn settings(&self, frame: &str) -> PipelineSettings {
        sidecar::read(Path::new(frame), true).unwrap().settings
    }
}

fn moved(a: &BTreeMap<String, Vec<u8>>, b: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    a.keys().filter(|k| a[*k] != b[*k]).cloned().collect()
}

/// What a measure writes on a frame, read as Swift wrote it: the reference, the balance and the window.
fn measured_fields(s: &PipelineSettings) -> String {
    format!(
        "{:?} {:?} {:?} {:?} {:?} {:?} {}",
        s.roll_reference, s.gains.stops, s.levels.red, s.levels.green, s.levels.blue, s.geometry.measure_window, s.capture_stops
    )
}

/// Every frame of the roll measured again on the reference Swift stored: `RollReference.written`
/// onto the frame brought to rest on the roll's film and window.
fn rewritten(roll: &Roll, open: &str, live: &PipelineSettings) -> Vec<WrittenFrame> {
    roll.frames
        .iter()
        .map(|f| {
            let before = if f == open { live.clone() } else { roll.settings(f) };
            let reference = before.roll_reference.clone().expect("every frame of roll #8 stands on its roll");
            let window = before.geometry.measure_window.expect("the window Swift measured through");
            let start = oni_measure::roll::start(&before, ConversionMode::Negative, window, None, before.capture_stops);
            let after = oni_core::balance::written(&reference, &start);
            (f.clone(), before, after)
        })
        .collect()
}

#[test]
fn a_measure_lands_what_swift_wrote_on_every_frame_as_one_gallery_step_and_undoes_byte_for_byte() {
    let Some(roll) = Roll::copy("measure") else { return };
    let kept = roll.frames.last().unwrap().clone();
    let open = roll.frames[3].clone();
    let mut store = roll.store();
    store.open(&open).unwrap();
    // The bytes after the opening's own write, the face it adopts among them.
    store.flush();
    let original = roll.bytes();
    let written = rewritten(&roll, &open, &store.settings().clone());
    for (f, before, after) in &written {
        assert_eq!(measured_fields(after), measured_fields(before), "{f}: the measure writes what Swift wrote");
    }
    // Twin: the same reference written onto the wrong film lands other levels than Swift's.
    let (_, before, _) = &written[0];
    let wrong = oni_measure::roll::start(before, ConversionMode::Positive, before.geometry.measure_window.unwrap(), None, before.capture_stops);
    assert_ne!(measured_fields(&oni_core::balance::written(before.roll_reference.as_ref().unwrap(), &wrong)), measured_fields(before));

    // One frame left out of the measure; the open frame lands in memory.
    let measured: Vec<WrittenFrame> = written.iter().filter(|(f, _, _)| *f != kept).cloned().collect();
    let frames = store.land_roll(&measured);
    assert_eq!(frames, roll.frames.len() - 1);
    assert_eq!(store.history().undo_label(), Some(measure_label(frames).as_str()));
    store.flush();
    let landed = roll.bytes();
    for (f, _, after) in &measured {
        assert_eq!(&roll.settings(f), after, "{f}");
    }
    // A frame left untouched since Swift's measure lands its own bytes again; the one left out stays.
    let mut expected: Vec<String> = measured.iter().filter(|(_, before, after)| before != after).map(|(f, _, _)| f.clone()).collect();
    expected.sort();
    assert!(expected.len() < frames, "some frames of roll #8 were left as Swift measured them");
    assert_eq!(moved(&original, &landed), expected);
    assert_eq!(roll.settings(&kept), written.iter().find(|(f, _, _)| *f == kept).unwrap().1);

    assert!(store.can_undo());
    store.undo();
    assert!(!store.can_undo(), "one measure, one step");
    store.flush();
    // The undo writes back every frame's settings as they were.
    assert_eq!(moved(&original, &roll.bytes()), Vec::<String>::new());
    // Redo lands the same bytes again.
    store.redo();
    store.flush();
    assert_eq!(moved(&landed, &roll.bytes()), Vec::<String>::new());
}

#[test]
fn reset_to_default_reaches_the_whole_roll_from_one_frame_and_undoes_byte_for_byte() {
    let Some(roll) = Roll::copy("reset") else { return };
    let open = roll.frames[5].clone();
    let mut store = roll.store();
    store.open(&open).unwrap();
    store.flush();
    let original = roll.bytes();
    let id = store.settings().roll_reference.as_ref().unwrap().id;
    assert_eq!(store.measured_with(id).len(), roll.frames.len());
    let reach = roll_membership::reset_reach(std::slice::from_ref(&roll.frames[0]), &roll.frames);
    assert_eq!(reach.frames.len(), roll.frames.len(), "a roll is reset whole, never one frame of it");
    let count = store.reset_to_default(&reach);
    assert_eq!(count, roll.frames.len());
    assert_eq!(store.history().undo_label(), Some(reset_label(count).as_str()));
    store.flush();
    let reset = roll.bytes();
    assert_eq!(moved(&original, &reset).len(), roll.frames.len());
    for f in &roll.frames {
        let s = roll.settings(f);
        assert!(s.roll_reference.is_none(), "{f} leaves its roll");
        assert_eq!(s.geometry.crop, PipelineSettings::default().geometry.crop, "{f} loses its crop");
    }
    assert!(store.measured_with(id).is_empty(), "the roll's measure is forgotten");
    store.undo();
    assert!(!store.can_undo(), "one reset, one step");
    store.flush();
    assert_eq!(moved(&original, &roll.bytes()), Vec::<String>::new());
    // Twin: a neighbour on another roll is never reached.
    let other = roll.frames[7].clone();
    let mut s = roll.settings(&other);
    s.roll_reference.as_mut().unwrap().id = oni_settings::Id::new();
    sidecar::write(&s, Path::new(&other), sidecar::WriteOptions::default()).unwrap();
    let reach = roll_membership::reset_reach(std::slice::from_ref(&roll.frames[0]), &roll.frames);
    assert!(!reach.frames.contains(&other) && reach.frames.len() == roll.frames.len() - 1);
}
