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
    let r = VirtualStock::of("kodak-portra-800").reference();
    s.virtual_film.is_some() && s.gains.stops == r.stops && s.levels == r.levels() && s.dye_set == VirtualStock::of("kodak-portra-800").dye_set()
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

/// Digital photos in one folder, the open one on `open`'s film, the others on `others`', each on its own reference.
fn on_stocks(label: &str, open: VirtualStock, others: VirtualStock) -> (EditorStore, std::path::PathBuf, [String; 3]) {
    let (store, dir, paths) = frames(label, |_| {});
    for (i, p) in paths.iter().enumerate() {
        let s = PipelineSettings::rest(ConversionMode::Digital, p.ends_with("NEF")).on_stock(if i == 0 { open } else { others });
        sidecar::write(&s, Path::new(p), WriteOptions::default()).unwrap();
    }
    let mut store = store;
    store.open(&paths[0]).unwrap();
    (store, dir, paths)
}

/// Whether `s` reads its negative on its own film's gains, windows and dyes.
fn on_own_reference(s: &PipelineSettings) -> bool {
    let stock = s.film().expect("a digital photo").stock;
    let r = stock.reference();
    s.gains.stops == r.stops && s.levels == r.levels() && s.dye_set == stock.dye_set()
}

#[test]
fn a_film_picked_on_one_digital_photo_leaves_the_folders_others_on_their_own_film() {
    let (mut store, dir, [a, b, c]) = on_stocks("picked", VirtualStock::of("kodak-portra-800"), VirtualStock::of("kodak-gold-200"));
    let next = store.settings().on_stock(VirtualStock::of("kodak-vision3-200t"));
    store.apply_at(Command::Write(Box::new(next)), 0.0);
    store.flush();
    assert!(on_own_reference(&read(&a)) && read(&a).film().unwrap().stock == VirtualStock::of("kodak-vision3-200t"));
    for p in [&b, &c] {
        let s = read(p);
        assert_eq!(s.film().unwrap().stock, VirtualStock::of("kodak-gold-200"), "{p}");
        assert!(on_own_reference(&s), "{p}: {:?} {:?}", s.gains, s.dye_set);
    }
    // Twin: Gold 200 read on Vision3 200T's gains and dyes, what the folder's roll laid, is off its reference.
    let mut laid = read(&b);
    laid.gains.stops = VirtualStock::of("kodak-vision3-200t").reference().stops;
    laid.dye_set = VirtualStock::of("kodak-vision3-200t").dye_set();
    assert!(!on_own_reference(&laid));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_roll_dose_moved_on_one_digital_photo_still_reaches_its_folder_on_each_films_own() {
    let (mut store, dir, [_, b, _]) = on_stocks("dose", VirtualStock::of("kodak-portra-800"), VirtualStock::of("kodak-ultramax-400"));
    let mut next = store.settings().clone();
    next.chroma_decompression = 0.7;
    store.apply_at(Command::Write(Box::new(next)), 0.0);
    store.flush();
    let s = read(&b);
    assert_eq!(s.chroma_decompression, 0.7, "the folder's roll shares the dose");
    assert!(on_own_reference(&s), "{:?}", s.dye_set);
    // Twin: the open photo's own dyes are another stock's.
    assert_ne!(store.settings().dye_set, s.dye_set);
    let _ = std::fs::remove_dir_all(&dir);
}
