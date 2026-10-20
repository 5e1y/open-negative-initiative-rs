//! Roll #8's measurements against what the Swift app wrote, read off its decode dumps; with
//! `ONI_SWIFT_MEASURE`, against what it measures on its own measuring copies (`FORMAT`).

use oni_graph::{Graph, Source};
use oni_measure::reader::{Copy, Reader};
use oni_measure::truncate::Stage;
use oni_measure::{auto_levels, key, pipette, roll};
use oni_settings::cg::Point;
use oni_settings::levels::{Levels, LevelsChannel};
use oni_pipeline::Pipeline;
use oni_settings::codec;
use oni_settings::{ConversionMode, PipelineSettings};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The 1000 px dumps against Swift's 1400 px copies part the filters by 0.0053 stop (DSC_6446, Pro)
/// and 0.0122 (DSC_6464, Scanner): a bound on the copies' gap, the harness judging the code.
const FILTERS_GAP: f32 = 0.02;
/// The roll's gains part by 0.0096 stop, its whites by 0.0070, its medians by 0.0019 of window.
const ROLL_STOPS_GAP: f32 = 0.03;
const ROLL_ENDS_GAP: f32 = 0.015;

/// On the same copy as Swift, only arithmetic and the Lanczos taps part the two.
const HARNESS_STOPS: f32 = 2e-3;
const HARNESS_ENDS: f32 = 2.0 / 1024.0;
const HARNESS_FILTERS: f32 = 2e-3;

struct Dump {
    width: u32,
    height: u32,
    pixels: Vec<[f32; 4]>,
}

/// `RenderDump`'s format: width and height as `Int32`, then RGBA `Float`, top row first.
fn dump(path: &Path) -> Dump {
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let int = |i: usize| i32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as u32;
    let (width, height) = (int(0), int(4));
    let floats: Vec<f32> = b[8..].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
    assert_eq!(floats.len(), (width * height * 4) as usize, "{}: truncated", path.display());
    Dump { width, height, pixels: floats.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect() }
}

fn names(dir: &Path) -> Vec<String> {
    let list = std::fs::read_to_string(dir.join("list.txt")).expect("list.txt");
    list.lines().filter(|l| !l.is_empty()).map(|l| l.rsplit('/').next().unwrap().to_owned()).collect()
}

fn swapped(d: &Dump) -> Vec<[f32; 4]> {
    d.pixels.iter().map(|p| [p[2], p[1], p[0], p[3]]).collect()
}

fn sidecar(roll: &Path, name: &str) -> oni_settings::sidecar::Loaded {
    oni_settings::sidecar::read(&roll.join(name), true, false).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn worst(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f32::max)
}

fn setup(check: &str) -> Option<(PathBuf, PathBuf, Graph)> {
    let dir = oni_fixtures::reference_dir(check)?;
    let roll = oni_fixtures::raw_dir(check)?;
    if !oni_fixtures::heavy(check) {
        return None;
    }
    let graph = Graph::new().map_err(|e| println!("SKIPPED {check}: {e}")).ok()?;
    Some((dir, roll, graph))
}

/// The frames Auto White Balance was pressed on, Pro and Scanner, with the full correction each wrote.
const PRESSED: [&str; 2] = ["DSC_6446.NEF", "DSC_6464.NEF"];

#[test]
fn roll8_auto_white_balance_lands_on_the_filters_swift_wrote() {
    let check = "roll8_auto_white_balance_lands_on_the_filters_swift_wrote";
    let Some((dir, roll, mut graph)) = setup(check) else { return };
    let mut pipeline = Pipeline::new();
    let list = names(&dir);
    let wrote: Vec<[f32; 3]> = PRESSED.iter().map(|n| sidecar(&roll, n).settings.balance_filters.expect("pressed")).collect();
    let mut failures = Vec::new();
    for (i, name) in PRESSED.iter().enumerate() {
        let index = list.iter().position(|n| n == name).expect("listed");
        let d = dump(&dir.join("decode").join(format!("{index:05}.f32")));
        let s = sidecar(&roll, name).settings;
        let mut read = |pixels: &[[f32; 4]]| {
            let source: Source = graph.upload(d.width, d.height, pixels).unwrap();
            let mut r = Reader { graph: &mut graph, pipeline: &mut pipeline, resident: None };
            key::outcome(&mut r, Copy { source: &source, full_width: None }, name, &s).map(|o| o.filters)
        };
        let ours = read(&d.pixels).expect("the frame balances");
        let gap = worst(ours, wrote[i]);
        let other = worst(ours, wrote[1 - i]);
        println!("{name} ({:?}): {ours:?} for Swift's {:?}, gap {gap:.5}; {other:.5} from the other frame's", s.face, wrote[i]);
        if gap >= FILTERS_GAP {
            failures.push(format!("{name}: {gap:.5} from the filters Swift wrote"));
        }
        // Twins: the other frame's press, and the same frame with red and blue swapped.
        if other < FILTERS_GAP {
            failures.push(format!("{name}: within {other:.5} of the other frame's press, so the bound sees nothing"));
        }
        let crossed = read(&swapped(&d)).map_or(f32::INFINITY, |f| worst(f, wrote[i]));
        if crossed < FILTERS_GAP {
            failures.push(format!("{name}: red and blue swapped still land within {crossed:.5}"));
        }
        graph.purge();
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Every frame measured as Prepare the Roll measures it, then combined, against the reference the
/// roll's sidecars carry; its picked base is given back, unshifted, since the pick is not recorded.
#[test]
fn roll8_reference_lands_on_the_roll_swift_measured() {
    let check = "roll8_reference_lands_on_the_roll_swift_measured";
    let Some((dir, roll_dir, mut graph)) = setup(check) else { return };
    let mut pipeline = Pipeline::new();
    let list = names(&dir);
    let reference = sidecar(&roll_dir, &list[0]).settings.roll_reference.expect("the roll was prepared");
    let measure = |graph: &mut Graph, pipeline: &mut Pipeline, swap: bool| {
        let clock = std::time::Instant::now();
        let mut frames = Vec::new();
        for (index, name) in list.iter().enumerate() {
            let loaded = sidecar(&roll_dir, name);
            assert_eq!(loaded.settings.roll_reference.as_ref().map(|r| r.id), Some(reference.id), "{name} stands on another roll");
            let draft = loaded.roll_draft.expect("a draft");
            let before = loaded.settings;
            let start = roll::start(&before, draft.film, draft.window.expect("a window"), None, before.capture_stops);
            let d = dump(&dir.join("decode").join(format!("{index:05}.f32")));
            let pixels = if swap { swapped(&d) } else { d.pixels };
            let source = graph.upload(d.width, d.height, &pixels).unwrap();
            let mut r = Reader { graph, pipeline, resident: None };
            if let Some(m) = key::roll_measured(&mut r, Copy { source: &source, full_width: None }, name, &start) {
                frames.push(m);
            }
            drop(source);
            graph.purge();
        }
        println!("{} frames measured in {:.1} s", frames.len(), clock.elapsed().as_secs_f64());
        frames
    };
    let frames = measure(&mut graph, &mut pipeline, false);
    assert_eq!(frames.len(), list.len(), "every frame of the roll reads");
    let picked = reference.base.map(|b| [0, 1, 2].map(|c| b[c] - auto_levels::shift(reference.stops[c], ConversionMode::Negative)));
    let ours = roll::combined(&frames, ConversionMode::Negative, picked).expect("the roll combines");
    let stops = worst(ours.stops, reference.stops);
    let white = worst(ours.white, reference.white);
    let median = worst(ours.median.expect("every frame carries its median"), reference.median.expect("the roll carries one"));
    let black = worst(ours.black, reference.black);
    println!("stops {:?} for {:?} ({stops:.4})", ours.stops, reference.stops);
    println!("white {:?} for {:?} ({white:.4}); median {:?} for {:?} ({median:.4}); black {black:.6}", ours.white, reference.white, ours.median, reference.median);
    assert!(stops < ROLL_STOPS_GAP && white < ROLL_ENDS_GAP && median < ROLL_ENDS_GAP, "the roll departs from Swift's");
    // The base moves with the gains, so the black parts only by the gains' own gap.
    assert!(black < ROLL_ENDS_GAP, "the base given back lands {black} off the black");

    // Twin: red and blue swapped on every frame, the same bounds see a different roll.
    let crossed = roll::combined(&measure(&mut graph, &mut pipeline, true), ConversionMode::Negative, picked).unwrap();
    let (cs, cw) = (worst(crossed.stops, reference.stops), worst(crossed.white, reference.white));
    println!("swapped: stops {:?} ({cs:.4}), white {:?} ({cw:.4})", crossed.stops, crossed.white);
    assert!(cs >= ROLL_STOPS_GAP && cw >= ROLL_ENDS_GAP, "the bounds cannot tell red from blue");
}

/// The frames the per-frame checks read; `ONI_FULL_ROLL=1` takes every frame the dump lists.
const FRAMES: [&str; 2] = ["DSC_6432.NEF", "DSC_6448.NEF"];

fn chosen(dir: &Path) -> Vec<(usize, String)> {
    let list = names(dir);
    if std::env::var_os("ONI_FULL_ROLL").is_some_and(|v| v == "1") {
        return list.into_iter().enumerate().collect();
    }
    FRAMES.iter().map(|f| (list.iter().position(|n| n == f).expect("listed"), f.to_string())).collect()
}

fn pressed_gap(a: &auto_levels::Balance, b: &auto_levels::Balance) -> (f32, f32) {
    let ends = (0..3).map(|c| (a.levels[c].black - b.levels[c].black).abs().max((a.levels[c].white - b.levels[c].white).abs()));
    (worst(a.stops, b.stops), ends.fold(0.0, f32::max))
}

/// `AutoLevels.selfCheck(source:)`: a second press, from what the first wrote, finds nothing, and
/// a paste places the ends on the gains it holds; read after the levels, the press would chase itself.
#[test]
fn roll8_a_second_press_finds_nothing_and_a_paste_places_ends_on_held_gains() {
    let check = "roll8_a_second_press_finds_nothing_and_a_paste_places_ends_on_held_gains";
    let Some((dir, _, mut graph)) = setup(check) else { return };
    let mut pipeline = Pipeline::new();
    for (index, name) in chosen(&dir) {
        let d = dump(&dir.join("decode").join(format!("{index:05}.f32")));
        let source = graph.upload(d.width, d.height, &d.pixels).unwrap();
        let copy = Copy { source: &source, full_width: None };
        let mut r = Reader { graph: &mut graph, pipeline: &mut pipeline, resident: None };
        let base = PipelineSettings::default();
        let first = key::placed(&mut r, copy, &base).expect("the frame places");
        let pressed = auto_levels::applied(&first, &base, Some(auto_levels::CLASSIC));
        let twice = key::placed(&mut r, copy, &pressed).expect("the pressed frame places");
        let (stops, ends) = pressed_gap(&first, &twice);
        println!("{name}: second press moves the gains {stops:.5} and the ends {ends:.5}");
        assert!(stops < 0.005 && ends < 0.002, "{name}: a second press finds {stops:.5} st, {ends:.5}");
        assert!(first.levels.iter().all(|l| l.white > l.black && (Levels::MID_RANGE.0..=Levels::MID_RANGE.1).contains(&l.mid)));

        let held = key::ends(&mut r, copy, &pressed).unwrap();
        let mut pushed = pressed.clone();
        pushed.gains.stops[0] += 0.5;
        let moved = key::ends(&mut r, copy, &pushed).unwrap();
        let same = (0..3).map(|c| (held[c].black - first.levels[c].black).abs().max((held[c].white - first.levels[c].white).abs())).fold(0.0, f32::max);
        let shifted = (moved[0].black - first.levels[0].black).abs();
        println!("{name}: paste ends on the press's gains {same:.5}, red +0.5 st moves them {shifted:.5}");
        assert!(same < 1e-3 && shifted > 0.01, "{name}: paste ends {same:.5}, pushed {shifted:.5}");

        // Twin: the same press read after the levels sees the handles it just wrote.
        let downstream = |r: &mut Reader, s: &PipelineSettings| {
            let h = r.reading(copy, s, Stage::Curves).histogram;
            auto_levels::two_steps(&h, s.gains.stops, s.mode, 0.0, |stops| {
                let mut probe = s.clone();
                probe.gains.stops = stops;
                r.reading(copy, &probe, Stage::Curves).histogram
            })
        };
        let circular_first = downstream(&mut r, &base).unwrap();
        let circular = auto_levels::applied(&circular_first, &base, None);
        // A press that places nothing on its own handles departs as surely as one that moves them.
        let (cs, ce) = downstream(&mut r, &circular).map_or((f32::INFINITY, f32::INFINITY), |b| pressed_gap(&circular_first, &b));
        println!("{name}: read after the levels the second press moves {cs:.5} st, {ce:.5}");
        assert!(cs >= 0.005 || ce >= 0.002, "{name}: the bound cannot see a press read after its own handles");
        drop(source);
        graph.purge();
    }
}

/// `Pipette.selfCheck(source:)`: the disc reads before the levels, so moving all three windows leaves
/// the reading where it was; read before the curves, the same disc moves.
#[test]
fn roll8_the_pipette_reads_before_the_levels_so_a_second_pick_cannot_answer_the_first() {
    let check = "roll8_the_pipette_reads_before_the_levels_so_a_second_pick_cannot_answer_the_first";
    let Some((dir, _, mut graph)) = setup(check) else { return };
    let mut pipeline = Pipeline::new();
    for (index, name) in chosen(&dir) {
        let d = dump(&dir.join("decode").join(format!("{index:05}.f32")));
        let source = graph.upload(d.width, d.height, &d.pixels).unwrap();
        let copy = Copy { source: &source, full_width: None };
        let mut r = Reader { graph: &mut graph, pipeline: &mut pipeline, resident: None };
        let rest = PipelineSettings::default();
        let mut moved = rest.clone();
        for channel in LevelsChannel::PER_CHANNEL {
            if let Some(w) = moved.levels.get_mut(channel) {
                *w = Levels { black: 0.42, white: 2.31, mid: 0.61, ..Levels::default() };
            }
        }
        let mut read = |s: &PipelineSettings, before: Stage| {
            let plane = r.plane(copy, s, before).unwrap();
            let at = Point::new(0.5, 0.5);
            pipette::read(&plane, at, s.mode, pipette::DEFAULT_RADIUS, pipette::RETAINED_SPACE, pipette::RETAINED_STATISTIC).unwrap().fractions
        };
        let drift = worst(read(&rest, Stage::Levels), read(&moved, Stage::Levels));
        let circular = worst(read(&rest, Stage::Curves), read(&moved, Stage::Curves));
        println!("{name}: the windows moved, the pick drifts {drift:.7}; read before the curves {circular:.4}");
        assert!(drift < 1e-6, "{name}: {drift}");
        // Twin: the circular wiring, read after the levels.
        assert!(circular > 1e-3, "{name}: {circular}");
        drop(source);
        graph.purge();
    }
}

// MARK: - The harness on Swift's own measuring copies

fn floats(v: &Value) -> Option<[f32; 3]> {
    let a = v.as_array()?;
    (a.len() == 3).then(|| [0, 1, 2].map(|c| a[c].as_f64().unwrap_or(f64::NAN) as f32))
}

fn settings_of(v: &Value) -> PipelineSettings {
    let source = codec::Source { mosaic: &|| true, plain: &|| false };
    codec::decode_value(v.clone(), &source, false).expect("a sidecar document").settings
}

/// What the Mac writes, beside its `measure/` folder of `RenderDump`s of `Negative.measure`.
const FORMAT: &str = r#"measure.json: {
  "frames": [{
    "file": "DSC_6432.NEF", "dump": "measure/00000.f32" (Negative.measure, the 1400 px copy),
    "fullWidth": the frame's full width,
    "start": the sidecar document (SettingsCodec) Prepare the Roll measures the frame from,
    "settings": the frame's own sidecar document,
    "measured": RollReference.measured on start: {"stops": [3], "tails": [[low, high] x 3], "median": [3] or null},
    "placed": AutoLevels.placed on settings: {"stops": [3], "levels": [[black, white] x 3] as fractions},
    "filters": FilterBalance.filters on settings: [3]
  }],
  "roll": RollReference.combined of every measured frame, no base: {"stops", "black", "white", "median"}
}"#;

#[test]
fn harness_lands_on_swift_s_own_measurements() {
    let check = "harness_lands_on_swift_s_own_measurements";
    let Some(path) = std::env::var_os("ONI_SWIFT_MEASURE").map(PathBuf::from) else {
        println!("SKIPPED {check}: ONI_SWIFT_MEASURE names no measure.json, written as\n{FORMAT}");
        return;
    };
    if !oni_fixtures::heavy(check) {
        return;
    }
    let Ok(mut graph) = Graph::new().map_err(|e| println!("SKIPPED {check}: {e}")) else { return };
    let mut pipeline = Pipeline::new();
    let doc: Value = serde_json::from_slice(&std::fs::read(&path).expect("measure.json")).expect("JSON");
    let base = path.parent().unwrap_or(Path::new("."));
    let (mut failures, mut frames, mut compared) = (Vec::new(), Vec::new(), 0);
    for f in doc["frames"].as_array().expect("frames") {
        let (mut missed, mut absent) = (Vec::new(), Vec::new());
        let name = f["file"].as_str().unwrap_or("?").to_owned();
        let d = dump(&base.join(f["dump"].as_str().expect("dump")));
        let source = graph.upload(d.width, d.height, &d.pixels).unwrap();
        let copy = Copy { source: &source, full_width: f["fullWidth"].as_f64() };
        let mut r = Reader { graph: &mut graph, pipeline: &mut pipeline, resident: None };
        let mut differ = |what: &str, gap: f32, bound: f32| {
            compared += 1;
            if gap.is_nan() || gap >= bound {
                missed.push(format!("{name} {what}: {gap:.6} past {bound}"));
            }
        };
        if f["measured"].is_object() {
            let start = settings_of(&f["start"]);
            match key::roll_measured(&mut r, copy, &name, &start) {
                Some(m) => {
                    differ("measured stops", worst(m.balance.stops, floats(&f["measured"]["stops"]).unwrap()), HARNESS_STOPS);
                    for (ours, theirs) in m.tails.iter().zip(f["measured"]["tails"].as_array().unwrap()) {
                        let at = |i: usize| theirs[i].as_f64().unwrap_or(f64::NAN) as f32;
                        differ("tails", (ours[0] - at(0)).abs().max((ours[1] - at(1)).abs()), HARNESS_ENDS);
                    }
                    if let (Some(ours), Some(theirs)) = (m.median, floats(&f["measured"]["median"])) {
                        differ("median", worst(ours, theirs), HARNESS_ENDS);
                    }
                    frames.push(m);
                }
                None => absent.push(format!("{name}: measures nothing where Swift measured")),
            }
        }
        let s = settings_of(&f["settings"]);
        if let Some(placed) = f["placed"].as_object() {
            match key::placed(&mut r, copy, &s) {
                Some(b) => {
                    differ("placed stops", worst(b.stops, floats(&placed["stops"]).unwrap()), HARNESS_STOPS);
                    for (c, ends) in placed["levels"].as_array().unwrap().iter().enumerate() {
                        let (black, white) = (ends[0].as_f64().unwrap() as f32, ends[1].as_f64().unwrap() as f32);
                        differ("placed ends", (b.levels[c].black - black).abs().max((b.levels[c].white - white).abs()), HARNESS_ENDS);
                    }
                }
                None => absent.push(format!("{name}: places nothing where Swift placed")),
            }
        }
        if let Some(theirs) = floats(&f["filters"]) {
            match key::filters(&mut r, copy, &s, oni_measure::filter_balance::printable) {
                Some(ours) => differ("filters", worst(ours, theirs), HARNESS_FILTERS),
                None => absent.push(format!("{name}: balances nothing where Swift did")),
            }
        }
        drop(source);
        graph.purge();
        failures.extend(missed);
        failures.extend(absent);
    }
    if let Some(theirs) = doc["roll"].as_object() {
        match roll::combined(&frames, ConversionMode::Negative, None) {
            Some(ours) => {
                compared += 3;
                for (what, a, b, bound) in [
                    ("roll stops", ours.stops, floats(&theirs["stops"]), HARNESS_STOPS),
                    ("roll black", ours.black, floats(&theirs["black"]), HARNESS_ENDS),
                    ("roll white", ours.white, floats(&theirs["white"]), HARNESS_ENDS),
                ] {
                    let gap = b.map_or(f32::NAN, |b| worst(a, b));
                    if gap.is_nan() || gap >= bound {
                        failures.push(format!("{what}: {gap:.6} past {bound}"));
                    }
                }
            }
            None => failures.push("the roll combines to nothing".into()),
        }
    }
    println!("{compared} values compared");
    assert!(compared > 0, "measure.json held nothing to compare");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
