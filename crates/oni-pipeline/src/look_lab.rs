//! The Frontier Look Lab: the measured table as a short chain of named values, a dye matrix then a
//! paper curve, baked on the CPU into a cube the `color_cube` kernel reads like the measured one.

pub mod measure;
pub mod paper;
pub mod params;
#[cfg(test)]
mod tests;

pub use params::*;

use crate::scanner::MACHINE;
use oni_render::ColorCube;

/// The density the paper's channel gains turn around: the machine's neutral green.
pub fn pivot() -> f64 {
    f64::from(MACHINE.neutral()[1])
}

/// Node `i` of `n` over the machine's density domain, the measured cube's own lattice at 27.
pub fn node(i: usize, n: usize) -> f64 {
    f64::from(MACHINE.low()) + f64::from(MACHINE.span()) * i as f64 / (n - 1) as f64
}

/// A density as the cube kernel reads it, 0 to 1 over the domain.
pub fn normalised(x: f64) -> f64 {
    ((x - f64::from(MACHINE.low())) / f64::from(MACHINE.span())).clamp(0.0, 1.0)
}

/// One set of values ready to evaluate, its curve's tangents laid once.
pub struct Chain {
    rows: [[f64; 3]; 3],
    curve: paper::Curve,
    gain: [f64; 3],
    offset: [f64; 3],
    pivot: f64,
}

impl Chain {
    pub fn new(p: &LabParams) -> Self {
        Self::with_pivot(p, pivot())
    }

    pub fn with_pivot(p: &LabParams, pivot: f64) -> Self {
        Chain {
            rows: p.rows(),
            curve: paper::Curve::new(&p.paper),
            gain: [p.paper_gain[0], 1.0, p.paper_gain[1]],
            offset: [p.paper_offset[0], 0.0, p.paper_offset[1]],
            pivot,
        }
    }

    /// The matrix then the paper: cube-input density to display sRGB code.
    pub fn paper(&self, x: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|c| {
            let mixed = self.rows[c][0] * x[0] + self.rows[c][1] * x[1] + self.rows[c][2] * x[2];
            self.curve.at(self.gain[c] * (mixed - self.pivot) + self.pivot + self.offset[c])
        })
    }

    /// The chain's code, clipped as the display reads it.
    pub fn code(&self, x: [f64; 3]) -> [f64; 3] {
        self.paper(x).map(|v| v.clamp(0.0, 1.0))
    }
}

/// The chain on an `n`³ lattice of the machine's domain, red fastest, as `CIColorCube` reads it.
pub fn bake(p: &LabParams, n: usize) -> ColorCube {
    let chain = Chain::new(p);
    let cube_data = (0..n * n * n)
        .map(|i| {
            let x = [i % n, (i / n) % n, i / (n * n)].map(|k| node(k, n));
            let c = chain.code(x);
            [c[0] as f32, c[1] as f32, c[2] as f32, 1.0]
        })
        .collect();
    ColorCube { cube_dimension: n, cube_data }
}
