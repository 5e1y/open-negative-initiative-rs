//! A hand's Frontier look reads ONI's inversion: started from ONI's own look it renders as the ONI
//! machine, and an edit of the file moves the render.

use oni_graph::{Graph, Rect};
use oni_pipeline::{Inputs, Pipeline, Request, frontier_table};
use oni_settings::modes::{Face, ScannerMachine};
use oni_settings::{ConversionMode, PipelineSettings};

/// A strip of negative tones rendered on `machine` and read back whole.
fn render(g: &mut Graph, machine: ScannerMachine) -> Vec<[f32; 4]> {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true).per_pixel_only();
    s.face = Face::Scanner;
    s.scanner_machine = machine;
    let pixels: Vec<[f32; 4]> = (0..64).map(|i| [0.15 + 0.01 * i as f32, 0.2 + 0.006 * i as f32, 0.12 + 0.004 * i as f32, 1.0]).collect();
    let source = g.upload(64, 1, &pixels).expect("the strip uploads");
    let request = Request { region: Some(Rect { x: 0, y: 0, w: 64, h: 1 }), measuring: true, ..Request::default() };
    Pipeline::new().render(g, &source, &s, &request, &Inputs::default(), None).expect("the strip renders").1
}

fn gap(a: &[[f32; 4]], b: &[[f32; 4]]) -> f32 {
    a.iter().zip(b).flat_map(|(p, q)| (0..3).map(move |c| (p[c] - q[c]).abs())).fold(0.0, f32::max)
}

#[test]
fn a_hands_look_reads_onis_inversion() {
    let Ok(mut g) = Graph::new() else { return println!("SKIPPED: no GPU") };
    let measured = render(&mut g, ScannerMachine::Frontier);
    let oni = render(&mut g, ScannerMachine::Oni);
    frontier_table::set(Some(frontier_table::starting()));
    let started = render(&mut g, ScannerMachine::Frontier);
    assert!(gap(&started, &oni) < 0.01, "the starting look is ONI's own: {}", gap(&started, &oni));
    // Twin: the measured table renders its own way, so the check can tell the paths apart.
    assert!(gap(&measured, &oni) > 0.02, "{}", gap(&measured, &oni));
    // An edit of the file reaches the render: every output halved.
    let mut halved = frontier_table::starting();
    halved.cube_data.iter_mut().for_each(|n| n[..3].iter_mut().for_each(|v| *v *= 0.5));
    frontier_table::set(Some(halved));
    assert!(gap(&render(&mut g, ScannerMachine::Frontier), &started) > 0.05);
    frontier_table::set(None);
    assert!(gap(&render(&mut g, ScannerMachine::Frontier), &measured) < 1e-6, "back on the measured table");
}
