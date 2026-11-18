//! A film type changed by any gesture moves each value still on the old mode's rest onto the new one's,
//! the hand's own kept, one step; a frame the change does not reach keeps every value, its white included.

use super::*;
use oni_settings::frame::RollReference;
use oni_settings::tone::Print;
use oni_settings::white::White;

const A: White = White { kelvin: 5200.0, tint: 4.0 };
const B: White = White { kelvin: 3300.0, tint: -2.0 };

/// RAWs named `names` in a fresh folder, each As Shot its own, `edit` laid on each sidecar by its name.
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
    store.read_shots(|p: &Path| match p.file_name()?.to_str()? {
        "b.NEF" => Some(B),
        _ => Some(A),
    });
    store.open(&paths[0]).unwrap();
    (store, dir, paths)
}

fn read(p: &str) -> PipelineSettings {
    sidecar::read(Path::new(p), true).unwrap().settings
}

/// Every value a mode rests on, at that mode's rest: its white aside, which rests on the file's As Shot.
fn on_rest(s: &PipelineSettings) -> bool {
    PipelineSettings { white: None, shot: None, ..s.clone() } == PipelineSettings::rest(s.mode, s.decoded_from_mosaic)
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
    // Twin: the open frame's former rule, its exposure kept whatever it rested on, leaves +1.50 EV on a digital photo.
    let negative = PipelineSettings::rest(ConversionMode::Negative, true);
    let kept = PipelineSettings { print: negative.print, ..negative.carried(ConversionMode::Digital) };
    assert!(!on_rest(&kept) && kept.print.exposure == Print::RESTING_EXPOSURE);
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
        assert_eq!((read(p).mode, read(p).print.exposure), (ConversionMode::Negative, Print::RESTING_EXPOSURE), "one undo");
    }
    assert_eq!(read(c).print.exposure, 0.7);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_digital_frame_beside_keeps_its_own_white_as_the_open_frame_turns_digital() {
    let (mut store, dir, paths) = frames("together", &["a.NEF", "b.NEF", "c.NEF"], |name, s| {
        if name == "b.NEF" {
            *s = s.carried(ConversionMode::Digital);
            (s.shot, s.white) = (Some(B), Some(B));
        }
    });
    let [a, b, c] = [&paths[0], &paths[1], &paths[2]];
    store.set_together(vec![b.clone(), c.clone()]);
    let before = store.settings().clone();
    store.apply(Command::Mode(ConversionMode::Digital));
    store.flush();
    assert_eq!((store.settings().mode, store.settings().white), (ConversionMode::Digital, Some(A)), "the open frame on its own As Shot");
    assert_eq!((read(b).mode, read(b).shot, read(b).white), (ConversionMode::Digital, Some(B), Some(B)), "the frame beside keeps its light");
    assert_eq!(read(c), PipelineSettings::rest(ConversionMode::Negative, true), "a negative beside, on no roll, takes nothing of it");
    // Twin: the open frame's write laid leaf by leaf, as the frames beside took a film type, gives b a's light.
    let mut patched = read(b);
    Field::White.patch(&before, store.settings(), &mut patched);
    assert_eq!(patched.white, Some(A));
    store.undo();
    store.flush();
    assert_eq!((read(a).mode, read(b).white), (ConversionMode::Negative, Some(B)));
    let _ = std::fs::remove_dir_all(&dir);
}
