//! A frame made digital lands on its virtual film, its negative's gains, windows and dyes placed from the
//! film's reference, whatever gesture made it digital.

use super::*;
use oni_settings::VirtualStock;

/// Two RAWs and a plain file in a fresh folder; `edit` laid on every sidecar.
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
    store.open(&paths[0]).unwrap();
    (store, dir, paths)
}

fn read(p: &str) -> PipelineSettings {
    sidecar::read(Path::new(p), p.ends_with("NEF")).unwrap().settings
}

/// What a frame on the stock's reference holds.
fn on_film(s: &PipelineSettings) -> bool {
    let r = VirtualStock::KodakPortra800.reference();
    s.virtual_film.is_some() && s.gains.stops == r.stops && s.levels == r.levels() && s.dye_set == VirtualStock::KodakPortra800.dye_set()
}

#[test]
fn frames_made_digital_together_each_land_on_their_film_in_one_step() {
    let (mut store, dir, [a, b, c]) = frames("together", |_| {});
    store.apply_mode(ConversionMode::Digital, &[a.clone(), b.clone(), c.clone()]);
    store.flush();
    assert!(on_film(store.settings()));
    for p in [&b, &c] {
        assert!(read(p).mode == ConversionMode::Digital && on_film(&read(p)), "{p}");
    }
    store.undo();
    store.flush();
    assert_eq!(store.settings().mode, ConversionMode::Negative, "one step back");
    // Twin: back on the negative, its own gains and windows stand, no film's.
    assert!(!on_film(&read(&b)) && read(&b).mode == ConversionMode::Negative);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_roll_prepared_as_digital_lands_on_its_film_and_leaves_its_reference_in_one_step() {
    let roll = Id::new();
    let reference = oni_settings::frame::RollReference { id: roll, stops: [0.0; 3], black: [0.0; 3], white: [3.0; 3], base: None, median: None };
    let (mut store, dir, [a, b, c]) = frames("prepared", |s| s.roll_reference = Some(reference.clone()));
    let all = [a.clone(), b.clone(), c.clone()];
    assert_eq!(store.make_digital(&all), 3);
    assert_eq!(store.history().undo_label(), Some("Prepare the roll of 3 photos"));
    store.flush();
    for p in &all {
        let s = read(p);
        assert!(s.mode == ConversionMode::Digital && s.roll_reference.is_none() && on_film(&s), "{p}");
    }
    // One undo takes the three back.
    store.undo();
    store.flush();
    for p in &all {
        assert_eq!((read(p).mode, read(p).roll_reference.map(|r| r.id)), (ConversionMode::Negative, Some(roll)), "{p}");
    }
    // Twin: a frame left on its roll keeps the roll's windows, never the film's.
    assert!(!on_film(&read(&b)));
    let _ = std::fs::remove_dir_all(&dir);
}
