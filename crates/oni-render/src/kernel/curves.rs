//! The tone curves (`curves.metal`): one table per channel and one on luminance.

use super::*;

/// The curves' table length, `Curve.lutSize`.
pub const LUT_SIZE: usize = 1024;

/// `curves(src, rgbLUT, lumaLUT, bent)`. `rgbLUT` packs linked, red, green, blue in R, G, B, A;
/// `lumaLUT` carries luma in R. `bent` flags the channels', the linked and the luma tables.
#[derive(Clone, Debug, PartialEq)]
pub struct Curves {
    pub rgb_lut: Vec<V4>,
    pub luma_lut: Vec<V4>,
    pub bent: V3,
}

const LUMA_SCALE_MAX: f32 = 8.0;

/// A table read linearly at `v * (size - 1)`: Core Image's sample at `mix(0.5, size - 0.5, v)`.
fn table(lut: &[V4], v: f32, c: usize) -> f32 {
    let n = lut.len();
    let f = clamp(v, 0.0, 1.0) * (n - 1) as f32;
    let k = (f.floor() as usize).min(n - 1);
    mix(lut[k][c], lut[(k + 1).min(n - 1)][c], f - k as f32)
}

fn apply_curve(lut: &[V4], v: f32, c: usize) -> f32 {
    if v <= 1.0 { table(lut, v, c) } else { table(lut, 1.0, c) + (v - 1.0) }
}

pub fn curves(s: V4, k: &Curves) -> V4 {
    let mut e = rgb(s);
    if k.bent[0] > 0.5 {
        e = map(e, |c, x| apply_curve(&k.rgb_lut, x, c + 1));
    }
    if k.bent[1] > 0.5 {
        e = map(e, |_, x| apply_curve(&k.rgb_lut, x, 0));
    }
    let y = dot(e, LUMA);
    if k.bent[2] > 0.5 && y > 1e-4 {
        let f = clamp(apply_curve(&k.luma_lut, y, 0) / y, 0.0, LUMA_SCALE_MAX);
        e = map(e, |_, x| x * f);
    }
    with_alpha(e, s[3])
}
