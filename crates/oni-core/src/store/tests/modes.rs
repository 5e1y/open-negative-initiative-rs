//! A film type changed by any gesture moves each value still on the old mode's rest onto the new one's,
//! the hand's own kept, one step; a frame the change does not reach keeps every value, its film included.

use super::*;
use oni_settings::frame::RollReference;

/// RAWs named `names` in a fresh folder, `edit` laid on each sidecar by its name.
fn frames(label: &str, names: &[&str], edit: impl Fn(&str, &mut PipelineSettings)) -> (EditorStore, std::path::PathBuf, Vec<String>) {
    let dir = std::env::temp_dir().join(format!("oni-core-store-modes-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let paths: Vec<String> = names.iter().map(|n| dir.join(n).to_string_lossy().into_owned()).collect();
    for (p, n) in paths.iter().zip(names) {
        std::fs::write(p, b"").unwrap();
        let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
        edit(n, &mut s);
        sidecar::write(&s, Path::new(p), WriteOptions::default()).unwrap();
    }
    let mut store = EditorStore::new(Archive::new(paths.clone(), |_: &Path| true));
    store.open(&paths[0]).unwrap();
    (store, dir, paths)
}

fn read(p: &str) -> PipelineSettings {
    sidecar::read(Path::new(p), true).unwrap().settings
}

/// Every value a mode rests on, at that mode's rest.
fn on_rest(s: &PipelineSettings) -> bool {
    *s == PipelineSettings::rest(s.mode, s.decoded_from_mosaic)
}

#[test]
fn every_rest_follows_the_mode_and_a_placed_value_stays() {
    for from in ConversionMode::ALL {
        for to in ConversionMode::ALL {
            for mosaic in [true, false] {
                let carried = PipelineSettings::rest(from, mosaic).carried(to);
                assert!(on_rest(&carried), "{from:?} at rest carried to {to:?} (mosaic {mosaic}) stands off its rest");
            }
        }
    }
    // The hand's values are its own: an exposure, a ceiling and a black placed stay where they were put.
    let mut placed = PipelineSettings::rest(ConversionMode::Negative, true);
    (placed.print.exposure, placed.density_ceiling) = (0.7, placed.density_ceiling + 0.1);
    placed.levels.red.set_black(0.05);
    let carried = placed.carried(ConversionMode::Digital);
    assert_eq!((carried.print.exposure, carried.density_ceiling, carried.levels.red.black), (0.7, placed.density_ceiling, 0.05));
    // Twin: a ceiling kept whatever it rested on leaves a slide off its rest.
    let negative = PipelineSettings::rest(ConversionMode::Negative, true);
    let kept = PipelineSettings { density_ceiling: negative.density_ceiling, ..negative.carried(ConversionMode::Positive) };
    assert!(!on_rest(&kept) && kept.density_ceiling != PipelineSettings::rest(ConversionMode::Positive, true).density_ceiling);
}

#[test]
fn a_roll_turned_digital_from_its_open_frame_rests_every_frame_on_digital_in_one_step() {
    let roll = Id::new();
    let (mut store, dir, paths) = frames("roll", &["a.NEF", "b.NEF", "c.NEF"], |name, s| {
        s.roll_reference = Some(RollReference { id: roll, stops: [0.0; 3], black: [0.0; 3], white: [3.0; 3], base: None, median: None });
        if name == "c.NEF" {
            s.print.exposure = 0.7;
        }
    });
    let [a, b, c] = [&paths[0], &paths[1], &paths[2]];
    let measured = |s: &PipelineSettings| PipelineSettings { roll_reference: None, ..s.clone() };
    store.apply_mode(ConversionMode::Digital, std::slice::from_ref(a));
    store.flush();
    assert!(on_rest(&measured(store.settings())), "the open frame on Digital's rest: {:?}", store.settings().print);
    assert!(on_rest(&measured(&read(b))), "its roll's frame too: {:?}", read(b).print);
    assert_eq!((read(c).mode, read(c).print.exposure), (ConversionMode::Digital, 0.7), "an exposure placed by hand stays");
    store.undo();
    store.flush();
    for p in [a, b] {
        assert_eq!((read(p).mode, read(p).print.exposure), (ConversionMode::Negative, 0.0), "one undo");
    }
    assert_eq!(read(c).print.exposure, 0.7);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_digital_frame_beside_keeps_its_own_film_as_the_open_frame_turns_digital() {
    let (mut store, dir, paths) = frames("together", &["a.NEF", "b.NEF", "c.NEF"], |name, s| {
        if name == "b.NEF" {
            *s = s.carried(ConversionMode::Digital);
            s.virtual_film = s.film().map(|f| oni_settings::VirtualFilm { exposure: 1.5, ..f });
        }
    });
    let [a, b, c] = [&paths[0], &paths[1], &paths[2]];
    store.set_together(vec![b.clone(), c.clone()]);
    let before = store.settings().clone();
    store.apply(Command::Mode(ConversionMode::Digital));
    store.flush();
    let exposure = |s: &PipelineSettings| s.film().map(|f| f.exposure);
    assert_eq!((store.settings().mode, exposure(store.settings())), (ConversionMode::Digital, Some(0.0)), "the open frame on its film at rest");
    assert_eq!((read(b).mode, exposure(&read(b))), (ConversionMode::Digital, Some(1.5)), "the frame beside keeps its film");
    assert_eq!(read(c), PipelineSettings::rest(ConversionMode::Negative, true), "a negative beside, on no roll, takes nothing of it");
    // Twin: the open frame's write laid leaf by leaf gives b the open frame's film.
    let mut patched = read(b);
    Field::VirtualFilm.patch(&before, store.settings(), &mut patched);
    assert_eq!(exposure(&patched), Some(0.0));
    store.undo();
    store.flush();
    assert_eq!((read(a).mode, exposure(&read(b))), (ConversionMode::Negative, Some(1.5)));
    let _ = std::fs::remove_dir_all(&dir);
}
