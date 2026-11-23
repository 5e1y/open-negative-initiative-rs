//! The Frontier SP-3000 as a print on the paper kernel: its measured table refitted by
//! `docs/research/frontier/frontier_print_fit.py`, each of the kernel's sums run over bands of its own, not wavelengths.

use super::{Sheet, V3, V4};
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

/// The print, each layer reading the scanner's row of the three densities: the fit's matrix, black and white's alike.
pub(super) fn sheet() -> Sheet {
    let step = (CURVE_RANGE[1] - CURVE_RANGE[0]) / (PAPER_CURVE_NODES - 1) as f32;
    let at = |i: usize| f64::from(CURVE_RANGE[0] + step * i as f32);
    let curves = std::array::from_fn(|i| std::array::from_fn(|c| density(c, at(i)) as f32));
    Sheet::new(curves, CURVE_RANGE[0], step, FIT.dyes.clone(), FIT.view.clone(), FIT.matrix)
}
