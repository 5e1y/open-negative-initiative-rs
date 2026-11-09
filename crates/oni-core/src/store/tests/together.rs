//! Frames edited together: each takes the step the open frame takes, a knob moved by its distance,
//! a value set or written as it lands, and nothing the step left alone.

use super::*;
use oni_settings::tone::Print;

#[test]
fn a_step_sets_the_same_value_on_the_frames_together_keeps_their_own_and_undoes_as_one() {
    let dir = std::env::temp_dir().join(format!("oni-core-store-together-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let [a, b, c] = ["a.tif", "b.tif", "c.tif"].map(|n| dir.join(n).to_string_lossy().into_owned());
    std::fs::write(&a, b"").unwrap();
    let mut theirs = PipelineSettings::default();
    theirs.vignette.stops = -0.7;
    theirs.manual_order = Some(4.0);
    theirs.face = oni_settings::Face::Scanner;
    for p in [&b, &c] {
        std::fs::write(p, b"").unwrap();
        sidecar::write(&theirs, Path::new(p), WriteOptions::default()).unwrap();
    }
    let mut store = EditorStore::new(Archive::new(vec![a.clone(), b.clone(), c.clone()], |_: &Path| false));
    store.open(&a).unwrap();
    store.set_together(vec![b.clone()]);
    let mut next = store.settings().clone();
    next.vignette.stops = 0.5;
    next.manual_order = Some(9.0);
    store.apply_at(Command::Write(Box::new(next)), 0.0);
    let read = |p: &str| sidecar::read(Path::new(p), false, true).unwrap().settings;
    assert_eq!(store.together_live()[0].1.vignette.stops, 0.5, "shown as written");
    assert_eq!(read(&b).vignette.stops, 0.5, "the value set, not the difference");
    assert_eq!(read(&b).manual_order, Some(4.0), "a frame's own field stays its own");
    assert_eq!(read(&b).face, store.settings().face, "on the face the step was taken on, or it would show none of it");
    assert_eq!(read(&c).vignette.stops, -0.7, "twin: a frame left out keeps its value");
    assert_eq!(read(&c).face, oni_settings::Face::Scanner, "and its face");
    store.undo();
    assert_eq!(read(&b).vignette.stops, -0.7, "one undo takes the others back");
    assert_eq!(store.together_live()[0].1.vignette.stops, -0.7, "and shows them back");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Frames a, b and c, b and c with a vignette of their own, and the store open on a.
fn three(label: &str, b_settings: PipelineSettings, c_settings: PipelineSettings) -> (EditorStore, std::path::PathBuf, [String; 3]) {
    let dir = std::env::temp_dir().join(format!("oni-core-store-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let [a, b, c] = ["a.tif", "b.tif", "c.tif"].map(|n| dir.join(n).to_string_lossy().into_owned());
    std::fs::write(&a, b"").unwrap();
    for (p, s) in [(&b, b_settings), (&c, c_settings)] {
        std::fs::write(p, b"").unwrap();
        sidecar::write(&s, Path::new(p), WriteOptions::default()).unwrap();
    }
    // c stays out of the archive, so no roll gesture reaches it but the step laid together.
    let mut store = EditorStore::new(Archive::new(vec![a.clone(), b.clone()], |_: &Path| false));
    store.open(&a).unwrap();
    (store, dir, [a, b, c])
}

fn read(p: &str) -> PipelineSettings {
    sidecar::read(Path::new(p), false, true).unwrap().settings
}

#[test]
fn a_paste_on_frames_together_is_one_step_undone_to_each_one_s_own_settings() {
    let own = |stops| PipelineSettings { vignette: oni_settings::blocks::Vignette { stops, ..Default::default() }, ..PipelineSettings::default() };
    let (mut store, dir, [a, b, c]) = three("together-paste", own(-0.7), own(-0.4));
    store.set_together(vec![b.clone(), c.clone()]);
    store.board.settings = Some(codec::encode_settings(&own(0.2)).unwrap());
    store.paste_settings(&[a.clone(), b.clone()]);
    assert_eq!((read(&b).vignette.stops, read(&c).vignette.stops), (0.2, 0.2), "a frame together left out of the targets takes the step");
    store.undo();
    assert_eq!(read(&b).vignette.stops, -0.7, "the undo lays its own settings back, not a mix");
    assert_eq!(read(&c).vignette.stops, -0.4, "and the frame no target named, in the same step");
    assert!(!store.can_undo(), "one step");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_roll_field_moved_together_stays_on_the_frames_of_that_roll() {
    let roll = |id| oni_settings::frame::RollReference { id, stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None };
    let (r1, r2) = (Id::new(), Id::new());
    let on = |id| PipelineSettings { roll_reference: Some(roll(id)), ..PipelineSettings::default() };
    let (mut store, dir, [_, b, c]) = three("together-rolls", on(r2), on(r1));
    let mut next = PipelineSettings { roll_reference: Some(roll(r1)), ..store.settings().clone() };
    store.apply_at(Command::Write(Box::new(next.clone())), 0.0);
    store.set_together(vec![b.clone(), c.clone()]);
    next.levels.red.black = 0.2;
    next.vignette.stops = 0.5;
    store.apply_at(Command::Write(Box::new(next)), 5.0);
    let live: Vec<f32> = store.together_live().iter().map(|(_, s)| s.levels.red.black).collect();
    assert_eq!(read(&b).levels.red.black, 0.0, "another roll keeps its own windows");
    assert_eq!(read(&b).vignette.stops, 0.5, "and takes the look");
    assert_eq!(live[0], 0.0, "shown as written");
    // Twin: a frame of the same roll takes the window moved.
    assert_eq!(read(&c).levels.red.black, 0.2);
    assert_eq!(live[1], 0.2);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The face turned reaches every frame together, as a view and no step: the undos reach past it, and
/// the one of a step taken on Pro turns the view back there, every frame together with it.
#[test]
fn the_face_turned_reaches_every_frame_together_and_is_no_step() {
    use oni_settings::Face::{Pro, Scanner};
    let roll = |id| oni_settings::frame::RollReference { id, stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None };
    let (r1, r2) = (Id::new(), Id::new());
    let on = |id| PipelineSettings { roll_reference: Some(roll(id)), ..PipelineSettings::default() };
    let (mut store, dir, [_, b, c]) = three("together-face", on(r2), on(r1));
    let next = PipelineSettings { roll_reference: Some(roll(r1)), ..store.settings().clone() };
    store.apply_at(Command::Write(Box::new(next)), 0.0);
    store.set_together(vec![b.clone(), c.clone()]);
    store.apply_at(Command::Face(Scanner), 5.0);
    let live: Vec<oni_settings::Face> = store.together_live().iter().map(|(_, s)| s.face).collect();
    assert_eq!([read(&b).face, read(&c).face], [Scanner; 2], "another roll's frame takes the face too");
    assert_eq!(live, [Scanner; 2], "and shows it");
    // A roll field moved on the Scanner face reaches the roll's frame alone.
    let mut next = store.settings().clone();
    next.levels.red.black = 0.2;
    store.apply_at(Command::Write(Box::new(next)), 10.0);
    assert_eq!((read(&b).levels.red.black, read(&c).levels.red.black), (0.0, 0.2));
    store.undo();
    assert_eq!((read(&c).levels.red.black, store.face()), (0.0, Scanner), "the step taken on the Scanner face undone there");
    store.undo();
    assert_eq!(store.settings().roll_reference, None, "the second undo reaches the write made before the turn");
    assert_eq!([store.face(), read(&b).face, read(&c).face], [Pro; 3], "on Pro, where it was made, the frames together with it");
    // Twin: the turn as a step would be the second undo's, the write before it still to take.
    assert!(!store.can_undo());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A group formed with frames left on the other face takes the open frame's face, as a view and no
/// step: the frames together are edited on one face, never each on its own.
#[test]
fn frames_together_take_the_open_frames_face_when_the_group_forms() {
    use oni_settings::Face::{Pro, Scanner};
    let scanner = PipelineSettings { face: Scanner, ..PipelineSettings::default() };
    let (mut store, dir, [_, b, c]) = three("together-one-face", scanner.clone(), PipelineSettings::default());
    assert_eq!((read(&b).face, store.face()), (Scanner, Pro), "b was left on the Scanner face");
    store.set_together(vec![b.clone(), c.clone()]);
    let live: Vec<oni_settings::Face> = store.together_live().iter().map(|(_, s)| s.face).collect();
    assert_eq!([read(&b).face, read(&c).face], [Pro; 2], "every frame together on the open frame's face");
    assert_eq!(live, [Pro; 2], "and shown on it");
    assert!(!store.can_undo(), "a view, no step");
    // Twin: the open frame turned, the group follows it whole.
    store.apply_at(Command::Face(Scanner), 1.0);
    assert_eq!([read(&b).face, read(&c).face], [Scanner; 2]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Frame settings at a temperature and a tint, as a balance measured on that frame leaves them.
fn balanced(temperature: f32, tint: f32) -> PipelineSettings {
    Knob::Tint.set(&Knob::Temperature.set(&PipelineSettings::default(), temperature), tint)
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn a_temperature_drag_moves_each_frame_by_its_distance_and_leaves_its_tint() {
    let (mut store, dir, [_, b, c]) = three("together-drag", balanced(0.6, -0.3), balanced(-0.4, 0.2));
    store.set_together(vec![b.clone(), c.clone()]);
    store.hold();
    store.apply_at(Command::Step { knob: Knob::Temperature, by: 0.25 }, 0.0);
    store.apply_at(Command::Step { knob: Knob::Temperature, by: 0.25 }, 0.1);
    let live = store.together_live();
    assert!(near(Knob::Temperature.get(&live[0].1), 1.1), "shown under the hand: {}", Knob::Temperature.get(&live[0].1));
    store.release_at(0.2);
    let (rb, rc) = (read(&b), read(&c));
    assert!(near(Knob::Temperature.get(&rb), 1.1) && near(Knob::Temperature.get(&rc), 0.1), "each moved 0.5 from its own");
    assert!(near(Knob::Tint.get(&rb), -0.3) && near(Knob::Tint.get(&rc), 0.2), "the tint no hand moved stays each frame's");
    // Twins: the open frame's value laid whole, or its print carried whole, would land elsewhere.
    assert!(!near(Knob::Temperature.get(&rb), Knob::Temperature.get(store.settings())));
    let mut whole = balanced(0.6, -0.3);
    Field::Print.carry(store.settings(), &mut whole);
    assert!(!near(Knob::Tint.get(&whole), -0.3));
    store.undo();
    assert!(near(Knob::Temperature.get(&read(&b)), 0.6), "one undo takes every frame back");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_frame_held_at_a_knob_s_end_finds_its_offset_again_within_the_drag() {
    let (mut store, dir, [_, b, _]) = three("together-end", balanced(1.6, 0.0), balanced(0.0, 0.0));
    store.set_together(vec![b.clone()]);
    store.hold();
    store.apply_at(Command::Step { knob: Knob::Temperature, by: 0.6 }, 0.0);
    assert!(near(Knob::Temperature.get(&store.together_live()[0].1), Print::TEMPERATURE_RANGE.1), "held at the end");
    store.apply_at(Command::Step { knob: Knob::Temperature, by: -0.4 }, 0.1);
    store.release_at(0.2);
    assert!(near(Knob::Temperature.get(&read(&b)), 1.8), "0.2 past its own");
    // Twin: each notch laid on the last, the end ate the frame's offset.
    let notched = Knob::Temperature.set(&balanced(Print::TEMPERATURE_RANGE.1, 0.0), Print::TEMPERATURE_RANGE.1 - 0.4);
    assert!(!near(Knob::Temperature.get(&notched), 1.8));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_value_set_lands_on_every_frame_and_moves_nothing_else() {
    let (mut store, dir, [_, b, c]) = three("together-set", balanced(0.6, -0.3), balanced(0.6, -0.3));
    store.apply_at(Command::Step { knob: Knob::Temperature, by: 0.4 }, 0.0);
    store.set_together(vec![b.clone()]);
    store.apply_at(Command::Set { knob: Knob::Temperature, value: 0.0 }, 5.0);
    assert!(near(Knob::Temperature.get(&read(&b)), 0.0) && near(Knob::Tint.get(&read(&b)), -0.3));
    // Twin: the same distance as a move leaves a frame at its own value, never at the one set.
    store.set_together(vec![c.clone()]);
    store.apply_at(Command::Step { knob: Knob::Temperature, by: 0.4 }, 10.0);
    store.apply_at(Command::Step { knob: Knob::Temperature, by: -0.4 }, 15.0);
    assert!(near(Knob::Temperature.get(&read(&c)), 0.6));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_written_edit_lays_the_values_it_moved_and_no_other() {
    let own = PipelineSettings { print: oni_settings::tone::Print { contrast: 0.3, ..Default::default() }, ..PipelineSettings::default() };
    let (mut store, dir, [_, b, _]) = three("together-written", own.clone(), own);
    store.set_together(vec![b.clone()]);
    let mut next = store.settings().clone();
    next.print.exposure = 0.5;
    store.apply_at(Command::Write(Box::new(next)), 0.0);
    assert_eq!((read(&b).print.exposure, read(&b).print.contrast), (0.5, 0.3));
    // Twin: the open frame's contrast is another, which a whole print would have laid.
    assert_ne!(store.settings().print.contrast, 0.3);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_scanner_key_presses_each_frame_s_own_counts() {
    let mut own = PipelineSettings::default();
    own.scanner.c = 3;
    own.scanner.y = -2;
    let (mut store, dir, [_, b, _]) = three("together-scan", own.clone(), own);
    store.set_together(vec![b.clone()]);
    store.apply_at(Command::Scan(Scan::Step { letter: Letter::C, by: 1 }), 0.0);
    let s = read(&b).scanner;
    assert_eq!((s.c, s.y), (4, -2), "one click on its own three, its yellow untouched");
    // Twin: the open frame's own count is another.
    assert_ne!(store.settings().scanner.c, s.c);
    let _ = std::fs::remove_dir_all(&dir);
}
