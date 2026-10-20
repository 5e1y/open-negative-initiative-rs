//! The Frontier Look Lab: the measured table as a short chain of named values (a dye matrix, a paper
//! curve, a colour stage, the table's own residual), baked on the CPU into a cube like the measured one.

pub mod colour;
pub mod fit;
pub mod measure;
#[cfg(test)]
mod model_tests;
pub mod paper;
pub mod params;
#[cfg(test)]
mod tests;

pub use params::*;

use crate::scanner::MACHINE;
use measure::SIDE;
use oni_render::ColorCube;
use std::sync::LazyLock;

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

/// What the shipped Frontier set misses of the measured cube at each of its nodes. The clip is the
/// model's alone: the table's nodes dip under zero where it extrapolates, and the character keeps them.
pub static CHARACTER: LazyLock<Vec<[f64; 3]>> = LazyLock::new(|| {
    let model = Chain::new(&FITTED.frontier.with_character(0.0));
    (0..SIDE * SIDE * SIDE)
        .map(|i| {
            let (r, g, b) = (i % SIDE, (i / SIDE) % SIDE, i / (SIDE * SIDE));
            let (cube, x) = (measure::cube_at(r, g, b), [r, g, b].map(|k| node(k, SIDE)));
            let m = model.code(x);
            std::array::from_fn(|c| cube[c] - m[c])
        })
        .collect()
});

/// The character at density `x`, trilinear between the cube's nodes as the kernel reads them.
pub fn character(x: [f64; 3]) -> [f64; 3] {
    let f = x.map(|v| normalised(v) * (SIDE - 1) as f64);
    let lo = f.map(|v| (v.floor() as usize).min(SIDE - 2));
    let t: [f64; 3] = std::array::from_fn(|c| f[c] - lo[c] as f64);
    let mut out = [0.0; 3];
    for corner in 0..8 {
        let at: [usize; 3] = std::array::from_fn(|c| lo[c] + (corner >> c & 1));
        let w: f64 = (0..3).map(|c| if corner >> c & 1 == 1 { t[c] } else { 1.0 - t[c] }).product();
        let r = CHARACTER[(at[2] * SIDE + at[1]) * SIDE + at[0]];
        (0..3).for_each(|c| out[c] += w * r[c]);
    }
    out
}

/// One set of values ready to evaluate, its curve's tangents laid once.
pub struct Chain {
    p: LabParams,
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
            p: p.clone(),
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

    /// The model's code, clipped as the display reads it, then the character's share added.
    pub fn code(&self, x: [f64; 3]) -> [f64; 3] {
        let model = colour::colour(&self.p, self.paper(x)).map(|v| v.clamp(0.0, 1.0));
        if self.p.character == 0.0 {
            return model;
        }
        let r = character(x);
        std::array::from_fn(|c| model[c] + self.p.character * r[c])
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
