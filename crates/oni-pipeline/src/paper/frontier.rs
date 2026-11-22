//! The Frontier SP-3000 as a print on the paper kernel: its measured table refitted by
//! `docs/research/frontier/frontier_print_fit.py`, each of the kernel's sums run over bands of its own, not wavelengths.

use super::{Enlarger, Sheet, V3, V4};
use crate::film::bake;
use oni_render::PAPER_CURVE_NODES;
use serde::Deserialize;
use std::sync::LazyLock;

/// The log exposures the curves are laid over: past either end every layer sits on its own floor or Dmax.
pub const CURVE_RANGE: [f32; 2] = [-2.5, 2.5];

/// The fit as written: the table's neutral green density, the scanner's matrix (rows summing to one), each layer's
/// normal integrals (amplitude, centre, spread), the print's dyes (density a layer, then the band's base) and lights.
#[derive(Deserialize)]
pub struct Fit {
    pub neutral: f32,
    pub matrix: [V3; 3],
    pub curves: [Vec<[f64; 3]>; 3],
    pub dyes: Vec<V4>,
    pub view: Vec<V3>,
}

pub static FIT: LazyLock<Fit> =
    LazyLock::new(|| serde_json::from_slice(include_bytes!("../../resources/scanner/frontier_print.json")).expect("the Frontier's print reads"));

/// Layer `c`'s density at log exposure `e`.
pub fn density(c: usize, e: f64) -> f64 {
    let cdf = |z: f64| 0.5 * (1.0 + bake::erf(z / std::f64::consts::SQRT_2));
    FIT.curves[c].iter().map(|[a, centre, spread]| a * cdf((e - centre) / spread)).sum()
}

pub(super) fn sheet() -> Sheet {
    let step = (CURVE_RANGE[1] - CURVE_RANGE[0]) / (PAPER_CURVE_NODES - 1) as f32;
    let at = |i: usize| f64::from(CURVE_RANGE[0] + step * i as f32);
    Sheet {
        curves: std::array::from_fn(|i| std::array::from_fn(|c| density(c, at(i)) as f32)),
        curve_low: CURVE_RANGE[0],
        curve_step: step,
        print: FIT.dyes.clone(),
        view: FIT.view.clone(),
    }
}

/// The scanner's read, one band a layer: a row of the matrix over the dye amounts, its base laying the table's neutral
/// at unit time; each layer's own filter alone dims it, a CC a hundredth of density there. Black and white reads the mean.
pub(super) fn enlarger(mono: bool) -> Enlarger {
    let row = |b: usize| if mono { [1.0 / 3.0; 3] } else { FIT.matrix[b] };
    Enlarger {
        negative: (0..3).map(|b| [row(b)[0], row(b)[1], row(b)[2], -FIT.neutral]).collect(),
        exposure: (0..3).map(|b| std::array::from_fn(|c| if c == b { 1.0 } else { 0.0 })).collect(),
        dichroic: (0..3).map(|b| std::array::from_fn(|c| if c == b { 0.0 } else { 1.0 })).collect(),
    }
}
