//! Halation: the light a film's base sends back into its layers, each layer's exposure joined by a dose of its light
//! spread with a long tail, every light feeding it, before the film's curve.

use super::*;

/// The share of the dose each colour layer takes: the red one alone, the deepest, on the base; the light the base sends
/// back is red, blue stopped by the yellow filter and green absorbed by its own layer.
pub const HALATION_COLOUR: [f32; 3] = [1.0, 0.0, 0.0];

/// The glow's spread with distance, decaying as `exp(-r / λ)` on the film (owner's call): three Gaussians, each its
/// deviation in λ and its weight, within 15 % of the exponential out to nine λ.
pub const HALATION_TAIL: [[f32; 2]; 3] = [[0.5, 0.141_47], [1.5, 0.673_79], [2.75, 0.184_74]];

/// The halation's dose on each layer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Halation {
    pub dose: [f32; 3],
}

impl Halation {
    /// `dose` on a colour film's layers, or on a silver film's one layer the same on its three channels.
    pub fn on(dose: f32, silver: bool) -> Halation {
        Halation { dose: if silver { [dose; 3] } else { HALATION_COLOUR.map(|c| dose * c) } }
    }

    pub fn rows(&self) -> Vec<V4> {
        vec![v3(self.dose, 0.0)]
    }
}

/// The glow of one pixel: its light spread by each of `HALATION_TAIL`'s Gaussians, `near`, `mid` and `far`, weighed and
/// summed; alpha zero.
pub fn halation_tail(near: V4, mid: V4, far: V4) -> V4 {
    let [a, b, c] = HALATION_TAIL.map(|t| t[1]);
    with_alpha(map(rgb(near), |i, v| a * v + b * mid[i] + c * far[i]), 0.0)
}

/// One pixel's exposures joined by `k`'s dose of their glow; alpha kept.
pub fn halation(e: V4, glow: V4, k: &Halation) -> V4 {
    with_alpha(map(rgb(e), |c, v| v + k.dose[c] * glow[c]), e[3])
}

/// One pixel of `exposed` at `at`, in the graph's pixels, joined by `k`'s dose of the glow read bilinearly off `plate`
/// where `map` takes the pixel's centre; alpha kept.
pub fn halation_laid(exposed: &Grid, at: V2, plate: &Grid, map: [[f32; 3]; 2], k: &Halation) -> V4 {
    let e = exposed.load([at[0].floor() as i32, at[1].floor() as i32]);
    let [x, y] = map;
    let q = [x[0] * at[0] + x[1] * at[1] + x[2] - 0.5, y[0] * at[0] + y[1] * at[1] + y[2] - 0.5];
    let f = q.map(f32::floor);
    let t = [q[0] - f[0], q[1] - f[1]];
    let (i, j) = (f[0] as i32, f[1] as i32);
    let mix = |a: V4, b: V4, t: f32| std::array::from_fn::<f32, 4, _>(|c| a[c] + (b[c] - a[c]) * t);
    let top = mix(plate.load([i, j]), plate.load([i + 1, j]), t[0]);
    let bottom = mix(plate.load([i, j + 1]), plate.load([i + 1, j + 1]), t[0]);
    halation(e, mix(top, bottom, t[1]), k)
}
