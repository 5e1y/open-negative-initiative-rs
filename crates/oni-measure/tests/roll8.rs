//! Roll #8's keys on LibRaw's decode of two frames: a second press finds nothing, and the pipette
//! reads before the levels.

use oni_graph::{Graph, Source};
use oni_measure::reader::{Copy, Laying, Reader};
use oni_measure::truncate::Stage;
use oni_measure::{auto_levels, key, pipette};
use oni_pipeline::Pipeline;
use oni_settings::cg::Point;
use oni_settings::levels::{Levels, LevelsChannel};
use oni_settings::PipelineSettings;
use std::path::PathBuf;

fn worst(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f32::max)
}

fn setup(check: &str) -> Option<(PathBuf, Graph)> {
    let roll = oni_fixtures::raw_dir(check)?;
    if !oni_fixtures::heavy(check) {
        return None;
    }
    let graph = Graph::new().map_err(|e| println!("SKIPPED {check}: {e}")).ok()?;
    Some((roll, graph))
}

/// The frames the checks read.
const FRAMES: [&str; 2] = ["DSC_6432.NEF", "DSC_6448.NEF"];

/// LibRaw's decode of a frame at the measuring copy's side, uploaded.
fn uploaded(graph: &mut Graph, roll: &std::path::Path, name: &str) -> Source {
    let (d, _) = oni_decode::decode_for(&roll.join(name), 1.0, 2000).expect("LibRaw decodes the frame");
    graph.upload(d.width as u32, d.height as u32, &d.pixels).unwrap()
}

fn pressed_gap(a: &auto_levels::Balance, b: &auto_levels::Balance) -> (f32, f32) {
    let ends = (0..3).map(|c| (a.levels[c].black - b.levels[c].black).abs().max((a.levels[c].white - b.levels[c].white).abs()));
    (worst(a.stops, b.stops), ends.fold(0.0, f32::max))
}

/// A second press, from what the first wrote, finds nothing, and
/// a paste places the ends on the gains it holds; read after the levels, the press would chase itself.
#[test]
fn roll8_a_second_press_finds_nothing_and_a_paste_places_ends_on_held_gains() {
    let check = "roll8_a_second_press_finds_nothing_and_a_paste_places_ends_on_held_gains";
    let Some((roll, mut graph)) = setup(check) else { return };
    let mut pipeline = Pipeline::new();
    for name in FRAMES {
        let source = uploaded(&mut graph, &roll, name);
        let copy = Copy { source: &source, full_width: None };
        let mut r = Reader { graph: &mut graph, pipeline: &mut pipeline, laying: Laying::Nothing };
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

/// The disc reads before the levels, so moving all three windows leaves
/// the reading where it was; read before the curves, the same disc moves.
#[test]
fn roll8_the_pipette_reads_before_the_levels_so_a_second_pick_cannot_answer_the_first() {
    let check = "roll8_the_pipette_reads_before_the_levels_so_a_second_pick_cannot_answer_the_first";
    let Some((roll, mut graph)) = setup(check) else { return };
    let mut pipeline = Pipeline::new();
    for name in FRAMES {
        let source = uploaded(&mut graph, &roll, name);
        let copy = Copy { source: &source, full_width: None };
        let mut r = Reader { graph: &mut graph, pipeline: &mut pipeline, laying: Laying::Nothing };
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
