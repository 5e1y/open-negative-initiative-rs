//! A negative edited while a camera matrix shaped its decode, its gains compensated once the matrix
//! left: its film's base converts as before, so its points need no move, and its greys stay.

use oni_pipeline::conversion;
use oni_render::cpu;
use oni_settings::axis::density_axis;
use oni_settings::migrate::matrices::{M3, compensated, on_greys};
use oni_settings::{ConversionMode, PipelineSettings};

const Z_F: M3 = [[1.1242, -0.1665, -0.0092], [0.0277, 0.9493, -0.0627], [0.0104, -0.0133, 0.8966]];

fn apply(m: &M3, v: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|r| (0..3).map(|c| m[r][c] * v[c]).sum())
}

fn converted(x: [f64; 3], s: &PipelineSettings) -> [f32; 3] {
    let out = cpu::stage47([x[0] as f32, x[1] as f32, x[2] as f32, 1.0], &conversion::stage47(s));
    [out[0], out[1], out[2]]
}

/// The largest gap of any channel between the former render and this one over `scene`.
fn gap(scene: &[[f64; 3]], s: &PipelineSettings, now: &PipelineSettings, unmixed: &dyn Fn([f64; 3]) -> [f64; 3]) -> f32 {
    scene.iter().map(|&x| {
        let (before, after) = (converted(x, s), converted(unmixed(x), now));
        (0..3).map(|c| (before[c] - after[c]).abs()).fold(0.0, f32::max)
    }).fold(0.0, f32::max)
}

#[test]
fn a_negative_edited_under_a_matrix_keeps_its_greys_through_its_gains() {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.print.exposure = 0.3;
    s.gains.stops = [0.1, 0.0, -0.2];
    (s.levels.red.black, s.levels.green.black, s.levels.blue.black) = (0.35, 0.62, 0.9);
    (s.levels.red.white, s.levels.green.white, s.levels.blue.white) = (1.9, 2.2, 2.5);
    // The scan under the matrix: the orange base each black point stands on, then the dyes' density over it.
    let p = f64::from(density_axis::pedestal(s.density_ceiling));
    let base: [f64; 3] = std::array::from_fn(|c| {
        let black = [s.levels.red.black, s.levels.green.black, s.levels.blue.black][c];
        10f64.powf(-f64::from(black)) / f64::from(s.gains.stops[c]).exp2() - p
    });
    // The scene's greys: one density of dye over the base in each channel.
    let greys: Vec<[f64; 3]> = [1.0, 0.6, 0.3, 0.12, 0.05].iter().map(|&d| base.map(|b| b * d)).collect();
    let inverse = invert(&Z_F);
    let unmixed = |x: [f64; 3]| apply(&inverse, x);
    let now = compensated(&s, &Z_F);
    let over_greys = gap(&greys, &s, &now, &unmixed);
    assert!(over_greys < 0.01, "the scene's greys stay: {over_greys}");
    // Twins: without compensation the greys move, and they move too under the matrix's move on greys.
    let bare = gap(&greys, &s, &s, &unmixed);
    let mut by_greys = s.clone();
    let g = on_greys(&Z_F).unwrap();
    by_greys.gains.stops = std::array::from_fn(|c| s.gains.stops[c] + g[c]);
    let on_greys_base = gap(&greys, &s, &by_greys, &unmixed);
    assert!(bare > 0.02 && on_greys_base > 0.01, "uncompensated {bare}, on greys {on_greys_base}");
}

fn invert(m: &M3) -> M3 {
    let cof = |r: usize, c: usize| {
        let (r0, r1, c0, c1) = ((r + 1) % 3, (r + 2) % 3, (c + 1) % 3, (c + 2) % 3);
        m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0]
    };
    let det: f64 = (0..3).map(|c| m[0][c] * cof(0, c)).sum();
    std::array::from_fn(|r| std::array::from_fn(|c| cof(c, r) / det))
}
