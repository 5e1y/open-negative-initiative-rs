//! The whole model against the measured cube, and the character that gives the cube back.

use super::measure::*;
use super::*;

fn against_cube(p: &LabParams) -> f64 {
    let chain = Chain::new(p);
    median(&constrained().iter().map(|(x, cube)| delta_e(chain.code(*x), *cube)).collect::<Vec<_>>())
}

#[test]
fn the_full_model_lands_nearer_the_cube_than_the_paper_alone() {
    let model = FITTED.frontier.with_character(0.0);
    let full = against_cube(&model);
    println!("full model at the fitted Frontier values: median ΔE {full:.3}");
    assert!(full <= 1.7, "{full}");
    // Twin: the same set with its colour stage at rest.
    let paper = against_cube(&model.paper_only());
    println!("  twin, colour stage at rest: {paper:.3}");
    assert!(paper >= full + 0.15, "{paper} against {full}");
}

#[test]
fn at_frontier_values_the_character_gives_the_measured_cube_back() {
    let baked = bake(&FITTED.frontier, SIDE);
    let worst = baked.cube_data.iter().zip(&MACHINE.render.cube_data).map(|(a, b)| (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f32::max)).fold(0.0, f32::max);
    println!("character 1 against frontier.cube: worst node {worst:.2e}");
    assert!(worst <= 1e-5, "{worst}");
    // Twin: without the character the bake is the model, a visible step away.
    let pure = against_cube(&FITTED.frontier.with_character(0.0));
    assert!(pure > 1.0, "{pure}");
    let half = bake(&FITTED.frontier.with_character(0.5), SIDE);
    let moved = half.cube_data.iter().zip(&MACHINE.render.cube_data).any(|(a, b)| (0..3).any(|c| (a[c] - b[c]).abs() > 1e-3));
    assert!(moved, "half the character is not the cube");
}
