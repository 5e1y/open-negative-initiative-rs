//! Every input of frames edited together takes one path: each frame keeps its own offset, what a roll
//! shares reaches the whole roll and no other, and a step the open frame stood still for still lands.

use super::*;
use crate::command::Each;
use crate::knob::{Channel, Handle};
use oni_settings::blocks::SeparationFamily;
use oni_settings::frame::RollReference;

fn roll(id: Id) -> RollReference {
    RollReference { id, stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None }
}

fn on(id: Id) -> PipelineSettings {
    PipelineSettings { roll_reference: Some(roll(id)), ..PipelineSettings::default() }
}

/// Frames written as `laid` in one folder, every one in the archive, a NEF read through its mosaic,
/// and the store open on the first.
fn frames(label: &str, laid: Vec<(&str, PipelineSettings)>) -> (EditorStore, std::path::PathBuf, Vec<String>) {
    let dir = std::env::temp_dir().join(format!("oni-core-store-one-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut paths = Vec::new();
    for (name, settings) in laid {
        let path = dir.join(name).to_string_lossy().into_owned();
        std::fs::create_dir_all(Path::new(&path).parent().unwrap()).unwrap();
        std::fs::write(&path, b"").unwrap();
        sidecar::write(&settings, Path::new(&path), WriteOptions::default()).unwrap();
        paths.push(path);
    }
    let mut store = EditorStore::new(Archive::new(paths.clone(), |p: &Path| p.extension().is_some_and(|e| e == "NEF")));
    store.open(&paths[0]).unwrap();
    (store, dir, paths)
}

fn read(p: &str) -> PipelineSettings {
    let mosaic = p.ends_with(".NEF");
    sidecar::read(Path::new(p), mosaic, true).unwrap().settings
}

fn ceiling(id: Id, at: f32) -> PipelineSettings {
    PipelineSettings { density_ceiling: at, ..on(id) }
}

#[test]
fn a_roll_block_reset_with_part_of_the_roll_together_reaches_the_whole_roll_and_no_other() {
    let (r, other) = (Id::new(), Id::new());
    let laid = vec![("a.tif", ceiling(r, 2.5)), ("b.tif", ceiling(r, 2.5)), ("c.tif", ceiling(r, 2.5)), ("d.tif", ceiling(other, 2.2))];
    let (mut store, dir, p) = frames("roll-reset", laid);
    store.set_together(vec![p[1].clone(), p[3].clone()]);
    let rest = Each::new(|s| PipelineSettings { density_ceiling: 2.3, ..s.clone() });
    store.apply_at(Command::Each(rest.clone()), 0.0);
    let at = |p: &str| read(p).density_ceiling;
    assert_eq!((at(&p[1]), at(&p[2])), (2.3, 2.3), "the frame together and the roll's frame left unselected, as a slider move");
    assert_eq!(at(&p[3]), 2.2, "another roll keeps its own");
    store.undo();
    assert_eq!([at(&p[1]), at(&p[2]), at(&p[3])], [2.5, 2.5, 2.2], "one undo takes every frame back");
    // Twin: the gesture laid on the selection alone splits the roll, the frame left out on the old value.
    store.reset_each(&[p[0].clone(), p[1].clone(), p[3].clone()], |s| rest.on(s));
    assert_eq!((at(&p[2]), at(&p[3])), (2.5, 2.3));
    let _ = std::fs::remove_dir_all(&dir);
}

fn dyed(id: Id, family: SeparationFamily) -> PipelineSettings {
    PipelineSettings { separation: 1.1, separation_family: Some(family), separation_by_hand: true, ..on(id) }
}

#[test]
fn a_dye_reset_lays_the_open_roll_s_stock_on_its_roll_alone() {
    let (portra, fuji) = (Id::new(), Id::new());
    let laid = vec![("a.tif", dyed(portra, SeparationFamily::KodakC41)), ("b.tif", dyed(fuji, SeparationFamily::FujiC41)), ("c.tif", dyed(portra, SeparationFamily::KodakC41))];
    let (mut store, dir, p) = frames("dyes", laid);
    store.set_together(vec![p[1].clone()]);
    let rest = |s: &PipelineSettings| PipelineSettings { separation: 1.0, separation_family: Some(SeparationFamily::KodakC41), separation_by_hand: false, ..s.clone() };
    store.apply_at(Command::Each(Each::new(rest)), 0.0);
    let dyes = |s: PipelineSettings| (s.separation, s.separation_family, s.separation_by_hand);
    assert_eq!(dyes(read(&p[2])), (1.0, Some(SeparationFamily::KodakC41), false), "the Portra roll's frame left out takes its stock's dyes");
    assert_eq!(dyes(read(&p[1])), (1.1, Some(SeparationFamily::FujiC41), true), "the Fuji frame keeps its own stock's");
    // Twin: one family laid on every frame shown gives the Fuji frame Kodak's dyes.
    store.reset_each(&[p[1].clone()], rest);
    assert_eq!(read(&p[1]).separation_family, Some(SeparationFamily::KodakC41));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_action_the_open_frame_stands_still_for_still_reaches_the_frames_together() {
    let mut theirs = PipelineSettings { vignette: oni_settings::blocks::Vignette { stops: -0.7, ..Default::default() }, ..PipelineSettings::default() };
    theirs.scanner.c = 3;
    theirs.scanner.y = 10;
    theirs.manual_curves.red.add(oni_settings::cg::Point::new(0.5, 0.7));
    let mut open = PipelineSettings::default();
    open.scanner.y = 48;
    let (mut store, dir, p) = frames("still", vec![("a.tif", open), ("b.tif", theirs.clone()), ("c.tif", theirs)]);
    let (own, left) = (read(&p[1]), read(&p[2]));
    store.set_together(vec![p[1].clone()]);
    let set = store.apply_at(Command::Set { knob: Knob::Vignette, value: 0.0 }, 0.0);
    let reset = store.apply_at(Command::Scan(Scan::Reset { letter: Letter::C }), 5.0);
    let step = store.apply_at(Command::Scan(Scan::Step { letter: Letter::Y, by: 1 }), 10.0);
    let curves = store.apply_at(Command::Each(Each::new(|s| PipelineSettings { manual_curves: Default::default(), ..s.clone() })), 15.0);
    // Twin: the open frame stood each time, so a step read off its own change alone would land nowhere.
    assert!([set, reset, step, curves].iter().all(|a| a.changed.is_empty()));
    let b = read(&p[1]);
    assert_eq!((b.vignette.stops, b.scanner.c, b.scanner.y), (0.0, 0, 11), "the double click, the C reset, the key past the open frame's end");
    assert!(b.manual_curves.red.is_neutral(), "the curves reset on a frame whose own were moved");
    assert_eq!(read(&p[2]), left, "a frame left out is written nothing");
    for _ in 0..4 {
        store.undo();
    }
    assert_eq!(read(&p[1]), own, "the undos lay its own settings back, exactly");
    let _ = std::fs::remove_dir_all(&dir);
}

fn slide(white: f32) -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Positive, false);
    s.levels.red.white = white;
    s
}

#[test]
fn a_levels_handle_moves_each_slide_together_from_its_own_window() {
    let laid = vec![("Roll/a.tif", slide(2.4)), ("Roll/b.tif", slide(2.2)), ("Roll/c.tif", slide(2.0))];
    let (mut store, dir, p) = frames("slides", laid);
    store.set_together(vec![p[1].clone()]);
    store.apply_at(Command::Step { knob: Knob::Level(Channel::Red, Handle::White), by: -0.1 }, 0.0);
    let white = |p: &str| read(p).levels.red.white;
    assert!((white(&p[1]) - 2.1).abs() < 1e-5, "0.1 down from its own Auto levels: {}", white(&p[1]));
    assert_eq!(white(&p[2]), 2.0, "a slide not together keeps its own, as the roll never shares a slide's levels");
    // Twin: the window written as a value lays the open slide's white on the other.
    let mut next = store.settings().clone();
    next.levels.red.white = 2.2;
    store.apply_at(Command::Write(Box::new(next)), 5.0);
    assert!((white(&p[1]) - 2.2).abs() < 1e-5);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn frames_on_no_roll_share_no_roll_setting_and_a_contact_sheet_takes_no_flat_field() {
    let unprepared = PipelineSettings { density_ceiling: 2.4, ..PipelineSettings::default() };
    let (mut store, dir, p) = frames("no-roll", vec![("a.tif", unprepared.clone()), ("b.tif", unprepared)]);
    store.set_together(vec![p[1].clone()]);
    store.apply_at(Command::Step { knob: Knob::DensityCeiling, by: 0.1 }, 0.0);
    store.apply_at(Command::Step { knob: Knob::Vignette, by: 0.3 }, 5.0);
    let b = read(&p[1]);
    assert!((store.settings().density_ceiling - 2.5).abs() < 1e-5, "the open frame's moved");
    assert_eq!(b.density_ceiling, 2.4, "a frame on no roll keeps its own ceiling");
    // Twin: the look, which no roll owns, reaches it.
    assert!((b.vignette.stops - 0.3).abs() < 1e-6);
    let r = Id::new();
    let grid = oni_settings::frame::ContactSheetGrid { columns: 1, rows: 1, frames: 1, tile: Default::default(), margin: 0.0, windows: None };
    let sheet = PipelineSettings { sheet: Some(grid), ..on(r) };
    let (mut store, dir2, p) = frames("sheet", vec![("a.tif", on(r)), ("b.tif", sheet), ("c.tif", on(r))]);
    store.set_together(vec![p[1].clone(), p[2].clone()]);
    store.apply_at(Command::Step { knob: Knob::FlatFieldTemperature, by: 0.2 }, 0.0);
    assert_eq!(read(&p[1]).flat_field.temperature, 0.0, "the roll's contact sheet takes no flat-field");
    assert!((read(&p[2]).flat_field.temperature - 0.2).abs() < 1e-6, "twin: a frame of the roll does");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}

#[test]
fn a_frame_of_the_roll_on_another_decode_takes_its_roll_settings() {
    let (r, other) = (Id::new(), Id::new());
    let raw = PipelineSettings { decoded_from_mosaic: true, ..ceiling(r, 2.4) };
    let tiff = PipelineSettings { decoded_from_mosaic: false, ..ceiling(r, 2.4) };
    let (mut store, dir, p) = frames("decode", vec![("a.NEF", raw), ("b.tif", tiff), ("c.tif", ceiling(other, 2.4))]);
    store.set_together(vec![p[1].clone(), p[2].clone()]);
    store.apply_at(Command::Step { knob: Knob::DensityCeiling, by: 0.1 }, 0.0);
    assert!((read(&p[1]).density_ceiling - 2.5).abs() < 1e-5, "{}", read(&p[1]).density_ceiling);
    // Twin: a frame of another roll keeps its own.
    assert_eq!(read(&p[2]).density_ceiling, 2.4);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_move_lands_on_the_face_it_was_made_on() {
    let mut theirs = PipelineSettings::default();
    theirs.print.contrast = 0.1;
    theirs.scanner_finish.contrast = -0.1;
    let (mut store, dir, p) = frames("faces", vec![("a.tif", PipelineSettings::default()), ("b.tif", theirs)]);
    store.set_together(vec![p[1].clone()]);
    store.hold();
    store.apply_at(Command::Step { knob: Knob::Contrast, by: 0.2 }, 0.0);
    store.apply_at(Command::Face(oni_settings::Face::Scanner), 0.1);
    store.apply_at(Command::Step { knob: Knob::Contrast, by: 0.05 }, 0.2);
    store.release_at(0.3);
    let b = read(&p[1]);
    assert!((b.print.contrast - 0.3).abs() < 1e-6, "Pro's move on Pro's contrast: {}", b.print.contrast);
    // Twin: both moves laid on the face of the release would leave Pro's and take 0.25 on the Scanner's.
    assert!((b.scanner_finish.contrast - -0.05).abs() < 1e-6, "{}", b.scanner_finish.contrast);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_selection_changed_under_the_hand_keeps_the_drag_one_held_step() {
    let (mut store, dir, p) = frames("regroup", vec![("a.tif", PipelineSettings::default()), ("b.tif", PipelineSettings::default()), ("c.tif", PipelineSettings::default())]);
    store.set_together(vec![p[1].clone()]);
    store.hold();
    store.apply(Command::Step { knob: Knob::Vignette, by: 0.1 });
    store.set_together(vec![p[1].clone(), p[2].clone()]);
    store.apply(Command::Step { knob: Knob::Vignette, by: 0.1 });
    assert_eq!(read(&p[2]).vignette.stops, PipelineSettings::default().vignette.stops, "still under the hand, nothing written per notch");
    store.release();
    // Twin: once the hand lifts, the frame joined mid-drag takes the notch it was together for.
    assert!((read(&p[2]).vignette.stops - 0.1).abs() < 1e-6 && (read(&p[1]).vignette.stops - 0.2).abs() < 1e-6);
    let _ = std::fs::remove_dir_all(&dir);
}
