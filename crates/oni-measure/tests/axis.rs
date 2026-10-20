//! `FilmProbe.axisChecks`: the density a reading names for a value is the one the kernel renders,
//! `−log10(max(v + pedestal, guard))`, at both ends of the ceiling's range.

use oni_graph::{Graph, Rect};
use oni_measure::truncate::{Stage, truncated};
use oni_pipeline::{Inputs, Pipeline, Request};
use oni_settings::axis::density_axis;
use oni_settings::levels::{Levels, LevelsSet};
use oni_settings::PipelineSettings;

/// Real signal, the neighbourhood of zero the pedestal compresses, and both sides of `−pedestal`.
const SWEEP: [f32; 10] = [0.99, 0.5, 0.18, 0.05, 0.01, 0.002, 0.0, -0.004, -0.008, -0.02];
/// The table's third decimal; the range's two ends stand 0.503 apart on the guards.
const AGREEMENT: f32 = 1e-3;

fn density(value: f32, ceiling: f32) -> f32 {
    -(value + density_axis::pedestal(ceiling)).max(density_axis::log_guard(ceiling)).log10()
}

/// The sweep through stages 4 to 7 with every level at the identity and the clip skipped.
fn kernel(g: &mut Graph, ceiling: f32) -> Vec<f32> {
    let mut s = PipelineSettings { density_ceiling: ceiling, ..PipelineSettings::default() };
    let identity = Levels::default();
    s.levels = LevelsSet { red: identity, green: identity, blue: identity };
    s.print.bypassed = true;
    let row: Vec<[f32; 4]> = SWEEP.iter().map(|&v| [v, v, v, 1.0]).collect();
    let source = g.upload(SWEEP.len() as u32, 1, &row).expect("the sweep uploads");
    let request = Request { region: Some(Rect { x: 0, y: 0, w: SWEEP.len() as u32, h: 1 }), measuring: true, ..Request::default() };
    let s = truncated(&s, Stage::Curves);
    let (_, px) = Pipeline::new().render(g, &source, &s, &request, &Inputs::default(), None).expect("the sweep renders");
    px.iter().map(|p| p[0]).collect()
}

#[test]
fn the_density_a_reading_names_is_the_kernel_s_own_axis_at_both_ends_of_the_range() {
    let Ok(mut g) = Graph::new() else {
        println!("SKIPPED: no GPU");
        return;
    };
    let range = density_axis::range();
    let ends = [*range.start(), *range.end()];
    let rendered: Vec<Vec<f32>> = ends.iter().map(|&c| kernel(&mut g, c)).collect();
    let apart = |table: usize, kernel: usize| {
        SWEEP.iter().enumerate().map(|(i, &v)| (density(v, ends[table]) - rendered[kernel][i]).abs()).fold(0.0, f32::max)
    };
    let worst = apart(0, 0).max(apart(1, 1));
    assert!(worst < AGREEMENT, "the column stands {worst:.2e} from the kernel's axis");
    // Twin: the column drawn at one end and rendered at the other refuses.
    let crossed = apart(0, 1);
    assert!(crossed > AGREEMENT, "drawn at {} and rendered at {}, the column stands only {crossed:.4} away", ends[0], ends[1]);
}
