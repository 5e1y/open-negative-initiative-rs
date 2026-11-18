//! A frame made digital rests its white on its own As Shot, read once from its file, whatever gesture
//! made it digital; it turns to the Scanner face as a film does.

use super::*;
use oni_settings::white::White;

const A: White = White { kelvin: 5200.0, tint: 4.0 };
const B: White = White { kelvin: 3300.0, tint: -2.0 };

/// Two RAWs and a plain file in a fresh folder, each As Shot its own; `edit` laid on every sidecar.
fn frames(label: &str, edit: impl Fn(&mut PipelineSettings)) -> (EditorStore, std::path::PathBuf, [String; 3]) {
    let dir = std::env::temp_dir().join(format!("oni-core-store-digital-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let paths = ["a.NEF", "b.NEF", "c.jpg"].map(|n| dir.join(n).to_string_lossy().into_owned());
    for p in &paths {
        std::fs::write(p, b"").unwrap();
        let mut s = PipelineSettings::rest(ConversionMode::Negative, p.ends_with("NEF"));
        edit(&mut s);
        sidecar::write(&s, Path::new(p), WriteOptions::default()).unwrap();
    }
    let mut store = EditorStore::new(Archive::new(paths.to_vec(), |p: &Path| p.extension().is_some_and(|e| e == "NEF")));
    store.read_shots(|p: &Path| match p.file_name()?.to_str()? {
        "a.NEF" => Some(A),
        "b.NEF" => Some(B),
        _ => None,
    });
    store.open(&paths[0]).unwrap();
    (store, dir, paths)
}

fn read(p: &str) -> PipelineSettings {
    sidecar::read(Path::new(p), p.ends_with("NEF")).unwrap().settings
}

#[test]
fn frames_made_digital_together_each_rest_on_their_own_as_shot_in_one_step() {
    let (mut store, dir, [a, b, c]) = frames("together", |_| {});
    store.apply_mode(ConversionMode::Digital, &[a.clone(), b.clone(), c.clone()]);
    store.flush();
    assert_eq!((store.settings().shot, store.settings().white), (Some(A), Some(A)));
    assert_eq!((read(&b).mode, read(&b).shot, read(&b).white), (ConversionMode::Digital, Some(B), Some(B)));
    assert_eq!((read(&c).shot, read(&c).white), (None, None), "a plain file rests on its own white");
    // Twin: one white laid on every frame would have given b a's light.
    assert_ne!(read(&b).white, store.settings().white);
    store.undo();
    store.flush();
    assert_eq!((store.settings().mode, store.settings().shot), (ConversionMode::Negative, None), "one step back");
    assert_eq!((read(&b).mode, read(&b).shot), (ConversionMode::Negative, None));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_roll_made_digital_from_its_open_frame_reads_each_frames_own_as_shot_once() {
    let roll = Id::new();
    let (mut store, dir, [a, b, _]) = frames("roll", |s| {
        s.roll_reference = Some(oni_settings::frame::RollReference { id: roll, stops: [0.0; 3], black: [0.0; 3], white: [3.0; 3], base: None, median: None });
    });
    store.apply_mode(ConversionMode::Digital, std::slice::from_ref(&a));
    store.flush();
    assert_eq!((read(&b).mode, read(&b).shot, read(&b).white), (ConversionMode::Digital, Some(B), Some(B)), "the roll's frame its own");
    // A white moved, the roll back to negative then digital again: the white stays the hand's.
    store.apply(Command::Set { knob: Knob::WhiteKelvin, value: 6100.0 });
    store.apply_mode(ConversionMode::Negative, std::slice::from_ref(&a));
    store.apply_mode(ConversionMode::Digital, std::slice::from_ref(&a));
    assert_eq!(store.settings().white, Some(White { kelvin: 6100.0, ..A }));
    // Twin: the white moved is no longer the As Shot it rested on.
    assert_ne!(store.settings().white, store.settings().shot);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_digital_frame_turns_to_the_scanner_keeps_its_two_faces_curves_apart_and_shares_its_levels() {
    use crate::knob::Handle;
    use oni_settings::cg::Point;
    use oni_settings::levels::LevelsChannel;
    let (mut store, dir, [a, _, _]) = frames("face", |_| {});
    store.apply_mode(ConversionMode::Digital, std::slice::from_ref(&a));
    assert!(!store.apply(Command::Face(Face::Scanner)).changed.is_empty());
    assert_eq!((store.face(), store.settings().face), (Face::Scanner, Face::Scanner));
    let mut view = store.face_view();
    view.manual_curves.linked.add(Point::new(0.3, 0.6));
    store.apply(Command::Finish(Box::new(view)));
    store.apply(Command::Set { knob: Knob::Picture(LevelsChannel::Red, Handle::White), value: 0.8 });
    let s = store.settings().clone();
    assert!(s.manual_curves.is_neutral() && !s.scanner_finish.manual_curves.is_neutral(), "the curve is the Scanner's own");
    assert!((s.picture_levels.red.white - 0.8).abs() < 1e-6, "the levels are the photo's, on either face");
    // Twin: back on Pro, Pro's curves are as they were and the levels the same.
    store.apply(Command::Face(Face::Pro));
    assert_eq!(store.face(), Face::Pro);
    assert!(store.face_view().manual_curves.is_neutral());
    assert_eq!(store.face_view().picture_levels, s.picture_levels);
    assert_eq!(channel(ConversionMode::Digital), Channel::Positive);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_scanner_asks_a_film_roll_to_be_measured_again_and_never_a_digital_photo() {
    let reference = oni_settings::frame::RollReference { id: Id::new(), stops: [0.0; 3], black: [0.0; 3], white: [3.0; 3], base: None, median: None };
    let digital = PipelineSettings { roll_reference: Some(reference.clone()), ..PipelineSettings::rest(ConversionMode::Digital, true) };
    assert!(!crate::store::remeasure(&digital));
    // Twin: the same reference on a negative asks for its median.
    let negative = PipelineSettings { roll_reference: Some(reference), ..PipelineSettings::rest(ConversionMode::Negative, true) };
    assert!(crate::store::remeasure(&negative));
}

#[test]
fn a_roll_prepared_as_digital_reads_each_as_shot_once_and_leaves_its_reference_in_one_step() {
    use std::sync::{Arc, Mutex};
    let roll = Id::new();
    let reference = oni_settings::frame::RollReference { id: roll, stops: [0.0; 3], black: [0.0; 3], white: [3.0; 3], base: None, median: None };
    let (mut store, dir, [a, b, c]) = frames("prepared", |s| s.roll_reference = Some(reference.clone()));
    let reads: Arc<Mutex<Vec<String>>> = Arc::default();
    let counted = reads.clone();
    store.read_shots(move |p: &Path| {
        let name = p.file_name()?.to_str()?.to_string();
        counted.lock().unwrap().push(name.clone());
        match name.as_str() {
            "a.NEF" => Some(A),
            "b.NEF" => Some(B),
            _ => None,
        }
    });
    let all = [a.clone(), b.clone(), c.clone()];
    assert_eq!(store.make_digital(&all), 3);
    assert_eq!(store.history().undo_label(), Some("Prepare the roll of 3 photos"));
    store.flush();
    for (p, shot) in [(&a, Some(A)), (&b, Some(B)), (&c, None)] {
        let s = read(p);
        assert_eq!((s.mode, s.shot, s.white, s.roll_reference.is_none()), (ConversionMode::Digital, shot, shot, true), "{p}");
    }
    // One undo takes the three back; a redo, another frame open, lays the white the first read found.
    store.undo();
    store.flush();
    for p in &all {
        assert_eq!((read(p).mode, read(p).roll_reference.map(|r| r.id)), (ConversionMode::Negative, Some(roll)), "{p}");
    }
    store.open(&c).unwrap();
    store.redo();
    assert_eq!((read(&a).mode, read(&a).shot), (ConversionMode::Digital, Some(A)));
    // A plain file names no As Shot, so nothing read is kept for it.
    let mut raws: Vec<String> = reads.lock().unwrap().iter().filter(|n| n.ends_with("NEF")).cloned().collect();
    raws.sort();
    assert_eq!(raws, ["a.NEF", "b.NEF"], "each RAW's As Shot read once, the redo's too");
    // Twin: the measure's landing writes the frames beside the open one as they come, no As Shot read.
    let (mut landing, dir2, [_, b2, _]) = frames("prepared-twin", |s| s.roll_reference = Some(reference.clone()));
    let was = read(&b2);
    landing.land_roll(&[(b2.clone(), was.clone(), crate::store::roll::digital(&was))]);
    assert_eq!((read(&b2).mode, read(&b2).shot), (ConversionMode::Digital, None));
    let _ = (std::fs::remove_dir_all(&dir), std::fs::remove_dir_all(&dir2));
}
