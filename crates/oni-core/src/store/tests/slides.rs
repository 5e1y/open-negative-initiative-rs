//! A slide roll, which no measure gives a reference, shares Roll Settings and Paste to Roll over its
//! folder's slides, each keeping its own levels.

use super::*;
use crate::knob::Channel;

fn slide(white: f32) -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Positive, false);
    s.levels.red.white = white;
    s
}

/// A slide its gains-only measure balanced on `blue`, its gains resting there.
fn measured(white: f32, blue: f32) -> PipelineSettings {
    let mut s = slide(white);
    s.gains.stops = [0.0, 0.0, blue];
    s.measured_gains = Some(s.gains.stops);
    s
}

fn written(path: &str) -> PipelineSettings {
    sidecar::read(Path::new(path), false, true).unwrap().settings
}

/// Three slides, a negative never measured beside them and a slide in the folder next door.
fn slides(label: &str) -> (EditorStore, std::path::PathBuf, Vec<String>) {
    let dir = std::env::temp_dir().join(format!("oni-core-store-slides-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let laid = [("Roll/a.tif", measured(0.9, 0.3)), ("Roll/b.tif", measured(0.8, 0.1)), ("Roll/c.tif", PipelineSettings::default()), ("Roll 2/d.tif", slide(0.7))];
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
    assert!((b.gains.stops[2] - 0.2).abs() < 1e-6, "the other slide takes the move on its own measure, 0.1 up from 0.1: {:?}", b.gains);
    assert_eq!(b.measured_gains, Some([0.0, 0.0, 0.1]), "its measure stays its rest");
    assert_eq!(b.levels.red.white, 0.8, "and keeps the white its own Auto levels placed");
    // Twins: the move was not laid whole (0.4), and the negative beside and the next folder's slide stay.
    assert_ne!(b.gains.stops[2], 0.4);
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
