//! Applying a preset through the store: its rows on every target, the framing kept, one gallery
//! step for the gesture and one editor step for the open frame, and no write beyond the targets.

use oni_core::presets::{Preset, Tint};
use oni_core::{Archive, Command, EditorStore, Surface};
use oni_settings::frame::RollReference;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{Id, PipelineSettings};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn roll(label: &str, names: &[&str]) -> (PathBuf, Vec<String>) {
    let dir = std::env::temp_dir().join(format!("oni-core-presets-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    (dir.clone(), names.iter().map(|n| dir.join(n).to_string_lossy().into_owned()).collect())
}

fn on_disk(path: &str) -> PipelineSettings {
    sidecar::read(Path::new(path), false, true).unwrap().settings
}

fn rows(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|n| n.to_string()).collect()
}

fn soft() -> Preset {
    let mut donor = PipelineSettings::default();
    donor.sharpen.amount = 2.0;
    donor.texture.amount = 0.5;
    donor.geometry.angle = 7.0;
    donor.density_ceiling = 2.9;
    Preset::new("Soft", rows(&["sharpen", "texture", "densityCeiling"]), Tint::Moss, donor)
}

#[test]
fn a_preset_lays_its_rows_on_every_target_as_one_step() {
    let (dir, frames) = roll("apply", &["a.tif", "b.tif", "c.tif", "d.tif"]);
    let mut framed = PipelineSettings::default();
    framed.geometry.angle = -2.0;
    for f in &frames[1..] {
        sidecar::write(&framed, Path::new(f), WriteOptions::default()).unwrap();
    }
    let mut store = EditorStore::new(Archive::new(frames.clone(), |_: &Path| false));
    store.open(&frames[0]).unwrap();
    store.apply(Command::Write(Box::new(framed.clone())));
    let preset = soft();

    let targets = frames[..3].to_vec();
    assert_eq!(store.apply_preset(&preset, &targets, None), 3);
    for f in &targets[1..] {
        let s = on_disk(f);
        assert_eq!((s.sharpen.amount, s.texture.amount, s.geometry.angle), (2.0, 0.5, -2.0), "{f}");
    }
    let open = store.settings();
    assert_eq!((open.sharpen.amount, open.geometry.angle), (2.0, -2.0), "the framing is never carried");
    assert_eq!(store.gallery_history().undo_label(), Some("Apply \u{201c}Soft\u{201d} to 3 photos"));
    // Twin: a frame outside the targets is never written.
    assert_eq!(on_disk(&frames[3]).sharpen, framed.sharpen);

    // One gallery undo takes the whole gesture back, on disk and on the open frame.
    store.undo(Surface::Gallery);
    assert_eq!(store.settings().sharpen, framed.sharpen);
    for f in &targets[1..] {
        assert_eq!(on_disk(f).sharpen, framed.sharpen, "{f}");
    }
    store.redo(Surface::Gallery);
    assert_eq!(on_disk(&targets[1]).sharpen.amount, 2.0);
    // The open frame alone: one editor step, one gallery step named after the photo count.
    let _ = store.undo(Surface::Gallery);
    assert_eq!(store.apply_preset(&preset, &targets[..1], None), 1);
    assert_eq!(store.gallery_history().undo_label(), Some("Apply \u{201c}Soft\u{201d}"));
    store.undo(Surface::Editor);
    assert_eq!(store.settings().sharpen, framed.sharpen, "one editor undo takes the preset off");

    // Twin: a preset carrying nothing writes nothing and opens no step.
    let empty = Preset { parameters: BTreeSet::new(), ..preset };
    let label = store.gallery_history().undo_label().map(str::to_string);
    assert_eq!(store.apply_preset(&empty, &targets, None), 0);
    assert_eq!(store.gallery_history().undo_label().map(str::to_string), label);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_preset_carrying_a_roll_field_stays_on_its_targets() {
    let (dir, frames) = roll("roll", &["a.tif", "b.tif"]);
    let reference = RollReference { id: Id::new(), stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None };
    let measured = PipelineSettings { roll_reference: Some(reference), ..PipelineSettings::default() };
    sidecar::write(&measured, Path::new(&frames[1]), WriteOptions::default()).unwrap();
    let mut store = EditorStore::new(Archive::new(frames.clone(), |_: &Path| false));
    store.open(&frames[0]).unwrap();
    store.apply(Command::Write(Box::new(measured.clone())));
    let preset = soft();
    store.apply_preset(&preset, &frames[..1], None);
    assert_eq!(store.settings().density_ceiling, 2.9);
    assert_eq!(on_disk(&frames[1]).density_ceiling, measured.density_ceiling, "a preset is not a Roll Settings gesture");
    // Twin: the same field moved by hand on the open frame does reach the roll's other frame.
    store.apply(Command::Set { knob: oni_core::Knob::DensityCeiling, value: 2.5 });
    assert_eq!(on_disk(&frames[1]).density_ceiling, 2.5);
    let _ = std::fs::remove_dir_all(&dir);
}
