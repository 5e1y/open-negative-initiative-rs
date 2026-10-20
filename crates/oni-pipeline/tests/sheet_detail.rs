//! `Pipeline.measureWindowChecks`, its last line: a contact sheet renders sharpening and chroma
//! denoise off whatever they hold, and the same doses move a photograph.

use oni_graph::{Graph, Rect};
use oni_pipeline::{Inputs, Pipeline, Request};
use oni_settings::blocks::ChromaDenoise;
use oni_settings::cg::Size;
use oni_settings::frame::ContactSheetGrid;
use oni_settings::PipelineSettings;

const SIDE: u32 = 32;

/// A hard-edged checkerboard of five-pixel squares, read whole through the measuring chain.
fn rendered(g: &mut Graph, s: &PipelineSettings) -> Vec<[f32; 4]> {
    let pixels: Vec<[f32; 4]> = (0..SIDE * SIDE)
        .map(|i| if ((i % SIDE) / 5 + (i / SIDE) / 5) % 2 == 0 { [0.6, 0.2, 0.1, 1.0] } else { [0.1, 0.3, 0.5, 1.0] })
        .collect();
    let source = g.upload(SIDE, SIDE, &pixels).expect("the board uploads");
    let request = Request { region: Some(Rect { x: 0, y: 0, w: SIDE, h: SIDE }), measuring: true, ..Request::default() };
    Pipeline::new().render(g, &source, s, &request, &Inputs::default(), None).expect("the board renders").1
}

fn gap(a: &[[f32; 4]], b: &[[f32; 4]]) -> f32 {
    a.iter().zip(b).flat_map(|(p, q)| (0..3).map(move |c| (p[c] - q[c]).abs())).fold(0.0, f32::max)
}

#[test]
fn a_sheet_renders_sharpening_and_chroma_denoise_off_and_a_photograph_does_not() {
    let Ok(mut g) = Graph::new() else {
        println!("SKIPPED: no GPU");
        return;
    };
    let mut dosed = PipelineSettings::default();
    dosed.sharpen.amount = 3.0;
    dosed.chroma = ChromaDenoise { radius: 6.0, force: 1.0 };
    let mut bare = dosed.clone();
    bare.sharpen.amount = 0.0;
    bare.chroma = ChromaDenoise::OFF;
    let photo = gap(&rendered(&mut g, &dosed), &rendered(&mut g, &bare));
    let grid = ContactSheetGrid { columns: 1, rows: 1, frames: 1, tile: Size { width: 32.0, height: 32.0 }, margin: 0.0, windows: None };
    dosed.sheet = Some(grid.clone());
    bare.sheet = Some(grid);
    let sheet = gap(&rendered(&mut g, &dosed), &rendered(&mut g, &bare));
    assert_eq!(sheet, 0.0, "a sheet carries its doses into the render");
    // Twin: on a photograph the same doses move a pixel.
    assert!(photo > 1e-2, "the doses move a photograph only {photo:.4}");
}
