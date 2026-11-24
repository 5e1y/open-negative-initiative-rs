//! The Frontier SP-3000 as a paper: its measured table refitted into its paper's file by
//! `docs/research/frontier/frontier_print_fit.py` as the scanner's matrix and three curves, read through pure dyes.

use super::{Sheet, V3};
use crate::film::bake;
use oni_render::PAPER_CURVE_NODES;
use serde::Deserialize;

/// The log exposures the curves are laid over: past either end every layer sits on its own floor or Dmax.
pub const CURVE_RANGE: [f32; 2] = [-2.5, 2.5];

/// The fit as written: the table's neutral green density, the scanner's matrix (rows summing to one), and each layer's
/// normal integrals (amplitude, centre, spread).
#[derive(Deserialize)]
pub struct Fit {
    pub neutral: f32,
    pub matrix: [V3; 3],
    pub curves: [Vec<[f64; 3]>; 3],
}

/// The fit a paper's file holds, or why it does not read: every number finite, each layer a curve.
pub fn read(json: &str) -> Result<Fit, String> {
    let fit: Fit = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let finite = std::iter::once(fit.neutral).chain(fit.matrix.iter().flatten().copied()).all(f32::is_finite)
        && fit.curves.iter().flatten().flatten().all(|v| v.is_finite());
    if !finite || fit.curves.iter().any(Vec::is_empty) {
        return Err("a fit holding a number that is not finite, or a layer with no curve".into());
    }
    Ok(fit)
}

/// Layer `c`'s density at log exposure `e`.
pub fn density(fit: &Fit, c: usize, e: f64) -> f64 {
    let cdf = |z: f64| 0.5 * (1.0 + bake::erf(z / std::f64::consts::SQRT_2));
    fit.curves[c].iter().map(|[a, centre, spread]| a * cdf((e - centre) / spread)).sum()
}

/// The print, each layer reading the scanner's row of the three densities: the fit's matrix, black and white's alike.
pub(super) fn sheet(fit: &Fit) -> Sheet {
    let step = (CURVE_RANGE[1] - CURVE_RANGE[0]) / (PAPER_CURVE_NODES - 1) as f32;
    let at = |i: usize| f64::from(CURVE_RANGE[0] + step * i as f32);
    let curves = std::array::from_fn(|i| std::array::from_fn(|c| density(fit, c, at(i)) as f32));
    Sheet::new(curves, CURVE_RANGE[0], step, fit.matrix, super::PURE_DYES, None)
}
