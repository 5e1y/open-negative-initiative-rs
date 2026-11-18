//! A digital photo's levels lie in the curves' tables, so every reading in front of the curves renders
//! the frame as if they rested: the levels editor's histogram never follows its own handles. Its
//! Lighting acts after them, so no Lighting slider moves that reading either. On the Scanner face they
//! read the keyed picture in front of the machine's print.

use oni_graph::{Graph, Rect};
use oni_measure::reader::{Copy, Laying, Reader};
use oni_measure::truncate::{Stage, truncated};
use oni_pipeline::{Inputs, Pipeline, Request};
use oni_settings::cg::Point;
use oni_settings::levels::Levels;
use oni_settings::modes::Face;
use oni_settings::scanner::ScannerClicks;
use oni_settings::{ConversionMode, PipelineSettings};

const SWEEP: [f32; 8] = [0.0, 0.02, 0.1, 0.25, 0.5, 0.75, 0.95, 1.3];

fn read(g: &mut Graph, s: &PipelineSettings, before: Stage) -> Vec<[f32; 4]> {
    let row: Vec<[f32; 4]> = SWEEP.iter().map(|&v| [v, v * 0.8, v * 0.6, 1.0]).collect();
    let source = g.upload(SWEEP.len() as u32, 1, &row).expect("the sweep uploads");
    let request = Request { region: Some(Rect { x: 0, y: 0, w: SWEEP.len() as u32, h: 1 }), measuring: true, ..Request::default() };
    let (_, px) = Pipeline::new().render(g, &source, &truncated(s, before), &request, &Inputs::default(), None).expect("the sweep renders");
    px
}

#[test]
fn a_reading_in_front_of_the_curves_renders_no_picture_level() {
    let Ok(mut g) = Graph::new() else {
        println!("SKIPPED: no GPU");
        return;
    };
    let rest = PipelineSettings::rest(ConversionMode::Digital, true);
    let mut moved = rest.clone();
    moved.picture_levels.linked = Levels { black: 0.1, white: 0.8, mid: 0.35, ..Levels::default() };
    moved.picture_levels.blue.set_white(0.6);
    for stage in [Stage::Levels, Stage::ManualCurves] {
        assert_eq!(read(&mut g, &moved, stage), read(&mut g, &rest, stage), "{stage:?}");
    }
    // Twin: past the curves the reading is of the levels laid.
    assert_ne!(read(&mut g, &moved, Stage::Curves), read(&mut g, &rest, Stage::Curves));
}

#[test]
fn a_reading_in_front_of_a_digital_photos_curves_holds_while_lighting_moves() {
    let Ok(mut g) = Graph::new() else {
        println!("SKIPPED: no GPU");
        return;
    };
    let exposed = |mode: ConversionMode| {
        let mut s = PipelineSettings::rest(mode, true);
        s.print.exposure += 0.7;
        s.print.white_point = 0.05;
        (PipelineSettings::rest(mode, true), s)
    };
    let (rest, moved) = exposed(ConversionMode::Digital);
    assert_eq!(read(&mut g, &moved, Stage::ManualCurves), read(&mut g, &rest, Stage::ManualCurves));
    // Twin: a negative's Lighting comes before its curves, so their reading follows it.
    let (rest, moved) = exposed(ConversionMode::Negative);
    assert_ne!(read(&mut g, &moved, Stage::ManualCurves), read(&mut g, &rest, Stage::ManualCurves));
}

/// Through the reader's door, as the levels' histogram takes it.
fn entering(g: &mut Graph, s: &PipelineSettings, before: Stage) -> Vec<[f32; 4]> {
    let row: Vec<[f32; 4]> = SWEEP.iter().map(|&v| [v, v * 0.8, v * 0.6, 1.0]).collect();
    let source = g.upload(SWEEP.len() as u32, 1, &row).expect("the sweep uploads");
    let mut pipeline = Pipeline::new();
    let mut r = Reader { graph: g, pipeline: &mut pipeline, laying: Laying::Nothing };
    let image = r.image(Copy { source: &source, full_width: None }, s, before).expect("the recipe");
    g.render(&image, image.extent(), None).expect("the sweep renders")
}

#[test]
fn on_the_scanner_face_a_digital_photos_levels_read_the_keys_and_neither_the_print_nor_the_curves() {
    let Ok(mut g) = Graph::new() else {
        println!("SKIPPED: no GPU");
        return;
    };
    let rest = PipelineSettings { face: Face::Scanner, ..PipelineSettings::rest(ConversionMode::Digital, true) };
    let mut finished = rest.clone();
    finished.scanner_finish.contrast = 0.3;
    finished.scanner_finish.manual_curves.linked.add(Point::new(0.4, 0.7));
    finished.scanner.w = 3;
    assert_eq!(entering(&mut g, &finished, Stage::PictureLevels), entering(&mut g, &rest, Stage::PictureLevels));
    // The keys act before the levels, so their reading follows a D.
    let keyed = PipelineSettings { scanner: ScannerClicks::new(4, 0, 0, 0, 0), ..rest.clone() };
    assert_ne!(entering(&mut g, &keyed, Stage::PictureLevels), entering(&mut g, &rest, Stage::PictureLevels));
    // Twin: the machine's print reaches the reading in front of its curves.
    assert_ne!(entering(&mut g, &finished, Stage::ManualCurves), entering(&mut g, &rest, Stage::ManualCurves));
    // On Pro the keys render nothing, and the levels read what the curves read.
    let pro = |s: &PipelineSettings| PipelineSettings { face: Face::Pro, ..s.clone() };
    assert_eq!(entering(&mut g, &pro(&keyed), Stage::PictureLevels), entering(&mut g, &pro(&rest), Stage::ManualCurves));
}
