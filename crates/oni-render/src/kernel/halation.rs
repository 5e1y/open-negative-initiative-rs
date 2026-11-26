//! Halation: the light a film's base sends back into its layers, each layer's exposure joined by a dose of the blur of
//! its light weighted by how near the sensor's saturation it stands, before the film's curve.

use super::*;

/// The share of the dose each colour layer takes: the red one alone, the deepest, on the base; the light the base sends
/// back is red, blue stopped by the yellow filter and green absorbed by its own layer.
pub const HALATION_COLOUR: [f32; 3] = [1.0, 0.0, 0.0];

/// The halation's dose on each layer and each layer's exposure at the sensor's saturation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Halation {
    pub dose: [f32; 3],
    pub saturation: [f32; 3],
}

impl Halation {
    /// `dose` on a colour film's layers, or on a silver film's one layer the same on its three channels.
    pub fn on(dose: f32, silver: bool, saturation: [f32; 3]) -> Halation {
        Halation { dose: if silver { [dose; 3] } else { HALATION_COLOUR.map(|c| dose * c) }, saturation }
    }

    pub fn rows(&self) -> Vec<V4> {
        vec![v3(self.dose, 0.0), v3(self.saturation, 0.0)]
    }
}

/// The light one pixel sends into its glow: its exposure weighted by the square of its share of the saturation, so a
/// mid tone sends about a sixtieth of what a clipped lamp does; alpha zero.
pub fn halation_source(e: V4, k: &Halation) -> V4 {
    with_alpha(map(rgb(e), |c, v| v * clamp(v / k.saturation[c], 0.0, 1.0).powi(2)), 0.0)
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
