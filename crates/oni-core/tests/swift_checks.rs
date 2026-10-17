//! The Swift app's self-checks on the editing model, each on its own adversarial values and with its
//! twin, a variant built to fail when the property it guards is broken.

use oni_core::history::History;
use oni_core::parameters::{self, EXCLUDED};
use oni_core::presets::{self, Preset, Tint};
use oni_settings::axis::density_axis;
use oni_settings::cg::{Point, Rect};
use oni_settings::curves::SaturationCurve;
use oni_settings::frame::Correction;
use oni_settings::levels::LevelsSet;
use oni_settings::settings::look_settings;
use oni_settings::{ConversionMode, Id, PipelineSettings};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("oni-core-swift-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn rows(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|n| n.to_string()).collect()
}

// MARK: - ConversionMode.selfCheck

#[test]
fn a_mode_change_carries_the_handles_at_rest_and_leaves_a_placed_black() {
    let base = density_axis::base();
    let mut placed = PipelineSettings::rest(ConversionMode::Negative, false);
    placed.levels.red.set_black(0.42);
    let moved = parameters::carried(&placed, ConversionMode::Positive);
    assert_eq!(moved.levels.red.black, 0.42, "a placed black stays where the hand put it");
    assert_eq!(moved.levels.green.black, oni_settings::levels::Levels::resting_black(ConversionMode::Positive, base));
    // Twin: the round trip from a positive's rest lands on the negative's rest.
    let positive = PipelineSettings::rest(ConversionMode::Positive, false);
    let back = parameters::carried(&positive, ConversionMode::Negative);
    assert_eq!(back.levels, LevelsSet::neutral(ConversionMode::Negative, base));
    assert_ne!(positive.levels, back.levels);
}

// MARK: - Correction.selfCheck

#[test]
fn a_stroke_is_one_step_and_never_travels_with_a_look() {
    assert!(EXCLUDED.iter().any(|(field, _)| *field == "corrections"));
    let id = Id::parse("11111111-2222-3333-4444-555555555555").unwrap();
    let stroke = [Point::new(0.12, 0.34), Point::new(0.56, 0.78)];
    let mut history = History::default();
    for count in 1..=3 {
        let mut next = history.current().clone();
        let points = stroke.iter().cycle().take(count).copied().collect();
        next.corrections = vec![Correction { id, stroke: points, radius: 9.5 }];
        history.record(next, 1000.0 + count as f64 * 0.05);
    }
    assert_eq!(history.len(), 2, "a stroke of several points is one step");
    assert!(history.undo().is_some_and(|s| s.corrections.is_empty()));
    // Twin: the same points drawn past the coalescing window are three steps.
    let mut slow = History::default();
    for count in 1..=3 {
        let mut next = slow.current().clone();
        next.corrections = vec![Correction { id, stroke: stroke.iter().cycle().take(count).copied().collect(), radius: 9.5 }];
        slow.record(next, 1000.0 + count as f64);
    }
    assert_eq!(slow.len(), 4);
}

// MARK: - PresetTint.selfCheck

#[test]
fn an_unknown_tint_is_refused_and_the_default_is_neutral() {
    assert_eq!(Tint::from_raw("chartreuse"), None);
    assert_eq!(Tint::default(), Tint::Slate);
    // Twin: a known tint reads as itself.
    assert_eq!(Tint::from_raw("amber"), Some(Tint::Amber));
}

// MARK: - Propagation.selfCheck

/// Source and destination differ on every field the paste and the presets read.
fn source() -> PipelineSettings {
    let mut s = PipelineSettings::default();
    s.gains.stops = [1.5, -0.5, 0.25];
    s.print.white_point = 0.2;
    s.print.black_point = -0.15;
    s.print.red.exposure = 0.4;
    s.saturation = SaturationCurve::converted(0.3, 0.0);
    s.print.shadows = 0.11;
    s.print.highlights = -0.07;
    s.levels.red.white = 0.9;
    s.levels.green.black = 0.04;
    s.levels.blue.mid = 0.41;
    s.levels.red.shadows = 0.17;
    s.levels.green.highlights = 0.83;
    s.manual_curves.linked.points = vec![Point::new(0.0, 0.0), Point::new(0.4, 0.6), Point::new(1.0, 1.0)];
    s.chroma.force = 0.5;
    s.chroma.radius = 2.0;
    s.sharpen.amount = 0.8;
    s.geometry.quarter_turns = 1;
    s.geometry.angle = 3.5;
    s.geometry.measure_window = Some(Rect::new(0.2, 0.1, 0.5, 0.6));
    s
}

fn destination() -> PipelineSettings {
    let mut d = PipelineSettings::default();
    d.gains.stops = [-0.75, 1.25, -2.0];
    d.print.white_point = -0.1;
    d.print.blue.exposure = -0.3;
    d.saturation = SaturationCurve::converted(0.0, -0.4);
    d.print.shadows = -0.13;
    d.print.highlights = 0.09;
    d.levels.red.white = 0.72;
    d.levels.green.black = 0.11;
    d.levels.blue.mid = 0.58;
    d.levels.red.shadows = 0.31;
    d.levels.green.highlights = 0.69;
    d.manual_curves.linked.points = vec![Point::new(0.0, 0.05), Point::new(0.7, 0.3), Point::new(1.0, 1.0)];
    d.chroma.force = 0.17;
    d.chroma.radius = 5.0;
    d.sharpen.amount = 0.09;
    d.geometry.quarter_turns = 2;
    d.geometry.angle = -1.25;
    d.geometry.crop = Rect::new(0.1, 0.1, 0.5, 0.5);
    d
}

#[test]
fn a_paste_lays_the_look_and_leaves_the_framing_balance_and_film_type() {
    let (source, destination) = (source(), destination());
    let pasted = look_settings::copied(&source, &destination);
    assert_eq!(pasted.geometry, destination.geometry, "the framing and measuring window stay");
    assert!(pasted.manual_curves == source.manual_curves && pasted.saturation == source.saturation && pasted.print == source.print);
    assert!(pasted.chroma == source.chroma && pasted.sharpen == source.sharpen);
    assert!(pasted.gains == destination.gains && pasted.levels == destination.levels);
    // Twin: the three really differ, so a paste that overwrote one would be seen.
    assert!(pasted.geometry != source.geometry && source.gains != destination.gains && source.levels != destination.levels);

    let lender = PipelineSettings::rest(ConversionMode::Negative, false);
    let mut borrower = lender.clone();
    borrower.sharpen.amount = 2.0;
    assert_eq!(look_settings::copied(&lender, &borrower).sharpen, lender.sharpen, "the tabs travel whole");
    assert_ne!(lender.sharpen, borrower.sharpen);
    let mut positive = PipelineSettings::rest(ConversionMode::Positive, false);
    positive.sharpen.amount = 1.5;
    let crossed = look_settings::copied(&positive, &PipelineSettings::rest(ConversionMode::Negative, false));
    assert!(crossed.mode == ConversionMode::Negative && crossed.sharpen == positive.sharpen, "the film type never travels");
}

#[test]
fn each_former_group_carries_its_rows_and_none_falls_between_two() {
    let (source, destination) = (source(), destination());
    let preset = |names: &[&str]| Preset::new("p", rows(names), Tint::default(), source.clone());
    let convert = preset(&["gains", "levelsPerChannel"]).apply(&destination);
    assert!(convert.gains == source.gains && convert.levels == source.levels);
    assert!(convert.print == destination.print && convert.saturation == destination.saturation);
    assert!(convert.geometry == destination.geometry && convert.chroma == destination.chroma && convert.sharpen == destination.sharpen);

    let look = preset(&["levelsGlobal", "curves", "spectrogram", "saturation"]).apply(&destination);
    assert!(look.print == source.print && look.saturation == source.saturation);
    assert!(look.gains == destination.gains && look.levels == destination.levels);
    assert!(look.chroma == destination.chroma && look.sharpen == destination.sharpen);

    let detail = preset(&["chroma", "sharpen"]).apply(&destination);
    assert!(detail.chroma == source.chroma && detail.sharpen == source.sharpen);
    assert!(detail.gains == destination.gains && detail.levels == destination.levels && detail.print == destination.print);

    let all = ["gains", "levelsPerChannel", "levelsGlobal", "curves", "spectrogram", "saturation", "chroma", "sharpen"];
    let both = preset(&all).apply(&destination);
    let mut expected = destination.clone();
    expected.gains = source.gains;
    expected.levels = source.levels;
    expected.print = source.print;
    expected.saturation = source.saturation.clone();
    expected.chroma = source.chroma;
    expected.sharpen = source.sharpen;
    assert_eq!(both, expected, "the three tabs carry everything but geometry");
    assert_eq!(preset(&[]).apply(&destination), destination);
    // Twin: the test preset does change the destination.
    assert_ne!(both, destination);

    // No former group names these, and they are rows: leaving them out is a choice.
    let orphans = ["density", "densityCeiling", "separation", "texture", "flatField", "sourceEncoding"];
    assert!(orphans.iter().all(|o| !all.contains(o)));
    let ids: BTreeSet<&str> = parameters::all().iter().map(|r| r.id).collect();
    assert!(orphans.iter().all(|o| ids.contains(o)), "{orphans:?} against {ids:?}");
}

#[test]
fn a_new_preset_is_offered_the_lowest_free_number() {
    assert_eq!(presets::default_name(&[]), "Preset 01");
    assert_eq!(presets::default_name(&["Preset 01", "Preset 02"]), "Preset 03");
    // Twin: a highest-plus-one rule would answer 04.
    assert_eq!(presets::default_name(&["Preset 01", "Preset 03"]), "Preset 02");
    assert_eq!(presets::default_name(&["preset 01"]), "Preset 02");
    assert_eq!(presets::default_name(&["  PRESET 01  "]), "Preset 02");
    // `CharacterSet.whitespaces` folds spaces and tabs, never a line break.
    assert_eq!(presets::default_name(&["\tPreset 01"]), "Preset 02");
    assert_eq!(presets::default_name(&["Preset 01\n"]), "Preset 01");
    let ninety_nine: Vec<String> = (1..=99).map(|n| format!("Preset {n:02}")).collect();
    let refs: Vec<&str> = ninety_nine.iter().map(String::as_str).collect();
    assert_eq!(presets::default_name(&refs), "Preset 100");
    assert_eq!(presets::default_name(&["Portra 400", "Ektar"]), "Preset 01");
}

#[test]
fn presets_live_one_file_each_and_a_damaged_one_takes_none_down() {
    let folder = scratch("presets").join("Presets");
    let source = source();
    let first = Preset::new("Portra", rows(&["gains", "levelsPerChannel"]), Tint::default(), source.clone());
    let second = Preset::new("Portra", rows(&["levelsGlobal", "curves"]), Tint::default(), source.clone());
    let mut hostile = Preset::new("../../Kodak: 400/800", rows(&["gains", "sharpen"]), Tint::Plum, source.clone());
    hostile.note = "Light table, daylight".into();
    assert_ne!(presets::file_name(&first), presets::file_name(&second));
    let name = presets::file_name(&hostile);
    assert!(!name.contains('/') && !name.contains(':') && !name.starts_with('.'), "{name}");
    for p in [&first, &second, &hostile] {
        presets::save(p, &folder).unwrap();
    }
    assert_eq!(presets::files(&folder).len(), 3);
    // Twin: the raw name would have escaped the folder.
    assert_ne!(folder.join(&hostile.name).parent().map(Path::to_path_buf), Some(folder.clone()));
    assert_eq!(presets::load(&folder).into_iter().find(|p| p.id == hostile.id), Some(hostile.clone()));

    let mut renamed = hostile.clone();
    renamed.name = "Gold 200".into();
    presets::save(&renamed, &folder).unwrap();
    assert!(presets::load(&folder).contains(&renamed) && presets::files(&folder).len() == 3);
    let mut blanked = renamed.clone();
    blanked.name = "   ".into();
    presets::save(&blanked, &folder).unwrap();
    let after = presets::load(&folder);
    assert!(after.len() == 3 && after.iter().find(|p| p.id == hostile.id).map(|p| p.name.as_str()) == Some("Gold 200"));
    // Twin: the blank name written as it stands would not load.
    assert!(presets::decode(presets::encode(&blanked).unwrap().as_bytes()).is_none());

    std::fs::remove_file(&presets::files(&folder)[0]).unwrap();
    assert_eq!(presets::load(&folder).len(), 2, "a lost file takes no other down");
    let left = presets::load(&folder);
    presets::delete(&left[0], &folder, &mut |f: &Path| std::fs::remove_file(f));
    assert_eq!(presets::files(&folder).len(), 1);

    let target = &presets::files(&folder)[0];
    let mut object: Value = serde_json::from_slice(&std::fs::read(target).unwrap()).unwrap();
    object["settings"].as_object_mut().unwrap().remove("sharpen");
    std::fs::write(target, serde_json::to_vec(&object).unwrap()).unwrap();
    let holed = presets::decode(&std::fs::read(target).unwrap()).unwrap();
    assert!(holed.settings.sharpen == PipelineSettings::default().sharpen && holed.settings.gains == source.gains);

    let future = folder.join("future-00000000.json");
    std::fs::write(&future, br#"{"version": 99, "preset": {"name": "x", "groups": ["convert"]}}"#).unwrap();
    assert!(!presets::load(&folder).iter().any(|p| p.name == "x"), "a preset from a future version is ignored");
    std::fs::remove_file(&future).unwrap();

    // A file still naming the retired groups arrives carrying nothing, rather than guessed.
    let mut legacy = Preset::new("Ancien groupe", rows(&["gains"]), Tint::Rose, source.clone());
    legacy.note = "written before the tabs".into();
    let mut object: Value = serde_json::from_str(&presets::encode(&legacy).unwrap()).unwrap();
    object["preset"].as_object_mut().unwrap().remove("parameters");
    object["preset"]["groups"] = serde_json::json!(["colour", "tone"]);
    let reread = presets::decode(&serde_json::to_vec(&object).unwrap()).unwrap();
    assert!(reread.name == legacy.name && reread.tint == Tint::Rose && reread.note == legacy.note && reread.parameters.is_empty());
}

#[test]
fn a_preset_name_trims_spaces_and_tabs_and_keeps_a_line_break() {
    let folder = scratch("preset-names");
    let preset = Preset::new("Portra", rows(&["gains"]), Tint::default(), PipelineSettings::default());
    let mut object: Value = serde_json::from_str(&presets::encode(&preset).unwrap()).unwrap();
    object["preset"]["name"] = serde_json::json!("\tPortra\n");
    let read = presets::decode(&serde_json::to_vec(&object).unwrap()).unwrap();
    assert_eq!(read.name, "Portra\n", "the decode trims `CharacterSet.whitespaces` alone");
    // Twin: a save trims spaces and line breaks both.
    presets::save(&read, &folder).unwrap();
    assert_eq!(presets::load(&folder)[0].name, "Portra");
}

#[test]
fn an_import_never_overwrites_and_a_duplicate_says_so() {
    let folder = scratch("presets-share").join("Presets");
    let source = source();
    let held = Preset::new("Gold 200", rows(&["gains"]), Tint::Amber, source.clone());
    presets::save(&held, &folder).unwrap();
    assert!(presets::adopting(vec![held.clone()], &folder).is_empty() && presets::load(&folder).len() == 1);
    let mut retuned = held.clone();
    retuned.settings.gains.stops[0] += 0.5;
    let taken = presets::adopting(vec![retuned.clone()], &folder);
    let after = presets::load(&folder);
    assert!(taken.len() == 1 && after.len() == 2 && after.contains(&held));
    assert!(taken[0].id != held.id && taken[0].settings == retuned.settings);
    assert!(presets::decode(br#"{"hello": 1}"#).is_none(), "a JSON file that is not a preset is skipped");
    // Twin: a plain save of the clashing preset replaces the one held.
    let naive = folder.with_file_name("Naive");
    presets::save(&held, &naive).unwrap();
    presets::save(&retuned, &naive).unwrap();
    assert!(presets::load(&naive).len() == 1 && !presets::load(&naive).contains(&held));

    let copy = held.duplicated();
    assert!(copy.id != held.id && copy.name == "Gold 200 copy" && copy.settings == held.settings);
    assert!(copy.parameters == held.parameters && copy.tint == held.tint);
}
