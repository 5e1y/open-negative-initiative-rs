//! The whole model against the measured cube, and the character that gives the cube back.

use super::measure::*;
use super::*;
use crate::scanner;

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

#[test]
fn the_synthetic_roll_prints_its_grey_on_the_machine_neutral_and_pro_brighter() {
    let (frontier, pro) = pro::rolls();
    let grey = scanner::CHECK_GREY.map(|d| 10f32.powf(-d));
    let pixel = [grey[0], grey[1], grey[2], 1.0];
    let x = pro::cube_input(pixel, &frontier).unwrap();
    let neutral = MACHINE.neutral().map(f64::from);
    assert!((0..3).all(|c| (x[c] - neutral[c]).abs() < 1e-2), "{x:?} against {neutral:?}");
    let l = lab_of(pro::pro_code(pixel, &pro))[0];
    println!("Pro's grey at L {l:.3}; {} grid negatives", pro::grid().len());
    assert!(l > 0.65, "{l}");
    // Twin: the cube at the same input prints the grey darker.
    let cube = lab_of(Chain::new(&FITTED.frontier).code(x))[0];
    assert!(cube < 0.6, "{cube}");
}

fn against_pro(p: &LabParams, grid: &[([f64; 3], [f64; 3])]) -> f64 {
    let chain = Chain::new(p);
    median(&grid.iter().map(|(x, code)| delta_e(chain.code(*x), *code)).collect::<Vec<_>>())
}

#[test]
fn the_neutral_set_renders_pro_at_rest() {
    let grid = pro::grid();
    let neutral = against_pro(&FITTED.neutral, &grid);
    println!("neutral set against Pro at rest: median ΔE {neutral:.3} on {} negatives", grid.len());
    assert!(neutral <= 0.5, "{neutral}");
    // Twin: the Frontier set lands far from Pro.
    let frontier = against_pro(&FITTED.frontier.with_character(0.0), &grid);
    println!("  twin, Frontier set: {frontier:.3}");
    assert!(frontier > 5.0, "{frontier}");
}
