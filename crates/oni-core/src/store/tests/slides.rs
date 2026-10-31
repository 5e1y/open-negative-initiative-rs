//! A slide roll, which no measure gives a reference, shares Roll Settings and Paste to Roll over its
//! folder's slides, each keeping its own levels.

use super::*;
use crate::knob::Channel;

fn slide(white: f32) -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Positive, false);
    s.levels.red.white = white;
    s
}

fn written(path: &str) -> PipelineSettings {
    sidecar::read(Path::new(path), false, true).unwrap().settings
}

/// Three slides, a negative never measured beside them and a slide in the folder next door.
fn slides(label: &str) -> (EditorStore, std::path::PathBuf, Vec<String>) {
    let dir = std::env::temp_dir().join(format!("oni-core-store-slides-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let laid = [("Roll/a.tif", slide(0.9)), ("Roll/b.tif", slide(0.8)), ("Roll/c.tif", PipelineSettings::default()), ("Roll 2/d.tif", slide(0.7))];
    let mut paths = Vec::new();
    for (name, settings) in laid {
        let path = dir.join(name).to_string_lossy().into_owned();
        std::fs::create_dir_all(Path::new(&path).parent().unwrap()).unwrap();
        std::fs::write(&path, b"").unwrap();
        sidecar::write(&settings, Path::new(&path), WriteOptions::default()).unwrap();
        paths.push(path);
    }
    (EditorStore::new(Archive::new(paths.clone(), |_: &Path| false)), dir, paths)
}

#[test]
fn a_slide_s_source_balance_reaches_its_folder_s_slides_and_leaves_their_levels() {
    let (mut store, dir, paths) = slides("balance");
    store.open(&paths[0]).unwrap();
    store.apply(Command::Set { knob: Knob::Gain(Channel::Blue), value: 0.4 });
    let b = written(&paths[1]);
    assert_eq!(b.gains.stops[2], 0.4, "the other slide of the roll takes the balance");
    assert_eq!(b.levels.red.white, 0.8, "and keeps the white its own Auto levels placed");
    // Twins: the negative beside and the slide of the next folder stay as they were.
    assert_eq!(written(&paths[2]).gains.stops[2], 0.0);
    assert_eq!(written(&paths[3]).gains.stops[2], 0.0);
    assert_eq!(store.measured_with(store.roll_key().unwrap()).len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn paste_to_roll_on_a_slide_reaches_its_folder_s_slides() {
    let (mut store, dir, paths) = slides("paste");
    store.open(&paths[0]).unwrap();
    let mut donor = slide(0.9);
    donor.vignette.stops = -0.6;
    store.board.settings = Some(codec::encode_settings(&donor).unwrap());
    store.paste_to_roll();
    assert_eq!(store.settings().vignette.stops, -0.6);
    assert_eq!(written(&paths[1]).vignette.stops, -0.6, "the roll's other slide takes the look");
    // Twin: the negative beside is on no roll and takes nothing.
    assert_eq!(written(&paths[2]).vignette.stops, PipelineSettings::default().vignette.stops);
    let _ = std::fs::remove_dir_all(&dir);
}
