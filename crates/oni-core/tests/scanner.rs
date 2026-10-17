//! The Scanner face on the store: the keypad's commands, which the desktop keys and the phone send
//! alike, the face switch, and the rule that a gesture on one face never moves the other.

use oni_core::{Archive, Command, EditorStore, Knob, Scan, Surface};
use oni_link::scanner::{Effect, Letter, Machine};
use oni_settings::cg::Point;
use oni_settings::scanner::ScannerClicks;
use oni_settings::{ConversionMode, Face, PipelineSettings, ScannerMachine};
use std::path::{Path, PathBuf};

/// A store opened on the first of `frames` fresh frames with no sidecar.
fn fresh(label: &str, frames: usize) -> (EditorStore, Vec<String>, PathBuf) {
    let dir = std::env::temp_dir().join(format!("oni-core-scanner-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let paths: Vec<String> = (0..frames).map(|i| dir.join(format!("frame{i}.tif")).to_string_lossy().into_owned()).collect();
    let mut store = EditorStore::new(Archive::new(paths.clone(), |_: &Path| false));
    store.open(&paths[0]).unwrap();
    (store, paths, dir)
}

fn step(letter: Letter, by: i8) -> Command {
    Command::Scan(Scan::Step { letter, by })
}

fn effect(effect: Effect, by: i8) -> Command {
    Command::Scan(Scan::StepEffect { effect, by })
}

/// Everything but the keypad's own fields, which a key must leave alone.
fn without_keypad(s: &PipelineSettings) -> PipelineSettings {
    PipelineSettings { scanner: ScannerClicks::default(), scanner_finish: Default::default(), ..s.clone() }
}

/// `ScannerKey.selfCheck`, on the store's commands: every key moves its own count, a letter's own
/// key clears it alone, a held key stays on its stop, and the step keys land on their grids.
#[test]
fn the_keypad_writes_the_scanner_s_clicks_and_nothing_of_pro() {
    let (mut store, _, dir) = fresh("keys", 1);
    store.apply(Command::Face(Face::Scanner));
    let start = without_keypad(store.settings());
    for _ in 0..3 {
        store.apply(step(Letter::C, 1));
    }
    store.apply(step(Letter::M, -2));
    store.apply(step(Letter::D, 3));
    store.apply(step(Letter::W, 4));
    assert_eq!(store.settings().scanner, ScannerClicks::new(3, 3, -2, 0, 4), "every key moves its own count and no other");
    store.apply(Command::Scan(Scan::Reset { letter: Letter::D }));
    assert_eq!(store.settings().scanner, ScannerClicks::new(0, 3, -2, 0, 4), "a letter's own key clears that letter alone");
    for _ in 0..30 {
        store.apply(step(Letter::Y, 3));
        store.apply(step(Letter::W, -3));
    }
    assert_eq!((store.settings().scanner.y, store.settings().scanner.w), (24, -10), "a key held past its stop stays on it");

    // `LabStepKeys.moved`: a click rounds to the grid, then moves by its steps, held by the track.
    let finish = |store: &mut EditorStore, contrast: f32, vignette: f32| {
        let mut view = store.face_view();
        (view.print.contrast, view.vignette.stops) = (contrast, vignette);
        store.apply(Command::Finish(Box::new(view)));
    };
    finish(&mut store, 0.12, -0.04);
    store.apply(effect(Effect::Contrast, 1));
    assert!((store.settings().scanner_finish.contrast - 0.15).abs() < 1e-6, "{}", store.settings().scanner_finish.contrast);
    store.apply(effect(Effect::Vignette, -3));
    assert!((store.settings().scanner_finish.vignette.stops + 0.3).abs() < 1e-6, "{}", store.settings().scanner_finish.vignette.stops);
    finish(&mut store, 0.45, 0.0);
    store.apply(effect(Effect::Contrast, 3));
    assert_eq!(store.settings().scanner_finish.contrast, 0.5, "a Contrast key stops at the track's end");
    assert_eq!(without_keypad(store.settings()), start, "no key moved anything of Pro");

    // Twin: on black and white's wider track the same +3 goes on, to 0.6.
    store.apply(Command::Mode(ConversionMode::Monochrome));
    finish(&mut store, 0.45, 0.0);
    store.apply(effect(Effect::Contrast, 3));
    assert!((store.settings().scanner_finish.contrast - 0.6).abs() < 1e-6, "{}", store.settings().scanner_finish.contrast);
    // Chrome has no key in black and white.
    let before = store.settings().scanner_finish.secret_sauce;
    store.apply(effect(Effect::Chrome, 1));
    assert_eq!(store.settings().scanner_finish.secret_sauce, before);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `ScannerFinish.selfCheck`: a Scanner edit lands in its own finishing and leaves Pro's; the twin,
/// a Pro edit, leaves the Scanner's.
#[test]
fn a_gesture_on_one_face_never_moves_the_other() {
    let (mut store, _, dir) = fresh("finish", 1);
    store.apply(Command::Set { knob: Knob::Contrast, value: 0.2 });
    store.apply(Command::Set { knob: Knob::Chrome, value: 0.5 });
    store.apply(Command::Face(Face::Scanner));
    store.apply(Command::Set { knob: Knob::Contrast, value: -0.3 });
    store.apply(Command::Set { knob: Knob::Vignette, value: 1.0 });
    let s = store.settings().clone();
    assert_eq!((s.scanner_finish.contrast, s.scanner_finish.vignette.stops), (-0.3, 1.0), "a Scanner edit lands in its own finishing");
    assert_eq!((s.print.contrast, s.secret_sauce, s.vignette.stops), (0.2, 0.5, 0.0), "Pro's contrast, Chrome and vignette untouched");
    let view = store.face_view();
    assert_eq!((view.print.contrast, view.secret_sauce), (-0.3, 0.0), "and it reads back on the Scanner face, which never saw Pro's Chrome");

    // A structured edit, a curve's point, follows the same rule.
    let mut curved = store.face_view();
    curved.manual_curves.linked.points.insert(1, Point::new(0.5, 0.6));
    store.apply(Command::Finish(Box::new(curved)));
    assert_eq!(store.settings().scanner_finish.manual_curves.linked.points.len(), 3);
    assert_eq!(store.settings().manual_curves.linked.points.len(), 2, "Pro's curves untouched");

    // Twin: on Pro the same gestures move Pro's and leave the Scanner's where they were.
    store.apply(Command::Face(Face::Pro));
    store.apply(Command::Set { knob: Knob::Contrast, value: 0.4 });
    let mut curved = store.face_view();
    curved.manual_curves.red.points.insert(1, Point::new(0.5, 0.4));
    store.apply(Command::Finish(Box::new(curved)));
    let s = store.settings();
    assert_eq!((s.print.contrast, s.scanner_finish.contrast), (0.4, -0.3));
    assert_eq!((s.manual_curves.red.points.len(), s.scanner_finish.manual_curves.red.points.len()), (3, 2));
    let _ = std::fs::remove_dir_all(&dir);
}

/// `toggleLab` and `adoptFace`: the switch is one undo step on the open frame; a frame opens on the
/// face it is opened in, its history starting there.
#[test]
fn the_face_is_one_step_and_a_frame_opens_on_the_face_it_is_opened_in() {
    let (mut store, paths, dir) = fresh("face", 2);
    assert_eq!(store.settings().face, Face::Pro);
    let applied = store.apply(Command::Face(Face::Scanner));
    assert!(!applied.changed.is_empty() && store.settings().face == Face::Scanner && store.scanner_face);
    assert!(store.can_undo(Surface::Editor));
    store.apply(Command::Scan(Scan::Machine(Machine::Frontier)));
    assert_eq!(store.settings().scanner_machine, ScannerMachine::Frontier);
    store.flush();

    // The next frame opens on the Scanner face, with nothing to undo.
    store.open(&paths[1]).unwrap();
    assert_eq!(store.settings().face, Face::Scanner);
    assert!(!store.can_undo(Surface::Editor));
    // Twin: on Pro, the frame its sidecar stores on the Scanner face opens on Pro.
    store.apply(Command::Face(Face::Pro));
    store.open(&paths[0]).unwrap();
    assert_eq!(store.settings().face, Face::Pro);
    assert_eq!(store.settings().scanner_machine, ScannerMachine::Frontier, "the machine is the frame's own");

    // With nothing open the face still switches, and writes nothing.
    store.close();
    store.apply(Command::Face(Face::Scanner));
    assert!(store.scanner_face);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `resetBlank`: on the Scanner face the bar's reset clears the clicks and keeps Pro's Chrome; on Pro,
/// the twin, it keeps the clicks and clears Chrome.
#[test]
fn the_bar_s_reset_clears_the_edit_of_the_face_it_is_pressed_on() {
    let (mut store, _, dir) = fresh("reset", 1);
    store.apply(Command::Set { knob: Knob::Chrome, value: 0.5 });
    store.apply(Command::Face(Face::Scanner));
    store.apply(step(Letter::D, 3));
    store.apply(Command::Reset);
    assert_eq!((store.settings().scanner.d, store.settings().secret_sauce), (0, 0.5));
    store.apply(Command::Undo);
    store.apply(Command::Face(Face::Pro));
    store.apply(Command::Reset);
    assert_eq!((store.settings().scanner.d, store.settings().secret_sauce), (3, 0.0));
    let _ = std::fs::remove_dir_all(&dir);
}
