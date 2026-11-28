//! The chart a digital render is judged on against a film's reference: the ColorChecker's chromatic patches, Oklch rings and
//! a grey ramp, the render a CPU chain of the pipeline's own stages, compared at matched grey in Oklab and CIEDE2000.

use oni_pipeline::conversion;
use oni_pipeline::film;
use oni_render::{ColorMatrix, Stage47, cpu};
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};

pub type V3 = [f64; 3];
pub type M3 = [[f64; 3]; 3];

pub const SRGB_TO_XYZ: M3 = [[0.4124564, 0.3575761, 0.1804375], [0.2126729, 0.7151522, 0.0721750], [0.0193339, 0.1191920, 0.9503041]];
pub const M2020: M3 = [[0.6369580, 0.1446169, 0.1688810], [0.2627002, 0.6779981, 0.0593017], [0.0, 0.0280727, 1.0609851]];

pub fn mul(m: &M3, v: V3) -> V3 {
    std::array::from_fn(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}
pub fn matmul(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}
pub fn inv(m: &M3) -> M3 {
    let d = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let c = |a: usize, b: usize, e: usize, f: usize| m[a][b] * m[e][f] - m[a][f] * m[e][b];
    [
        [c(1, 1, 2, 2) / d, -c(0, 1, 2, 2) / d, c(0, 1, 1, 2) / d],
        [-c(1, 0, 2, 2) / d, c(0, 0, 2, 2) / d, -c(0, 0, 1, 2) / d],
        [c(1, 0, 2, 1) / d, -c(0, 0, 2, 1) / d, c(0, 0, 1, 1) / d],
    ]
}
pub fn dec(v: f64) -> f64 {
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}
pub fn enc(v: f64) -> f64 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.0031308 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}
/// sRGB code to Oklab.
pub fn oklab(code: V3) -> V3 {
    let [r, g, b] = code.map(dec);
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    [0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s, 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s, 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s]
}
/// A digital photo's render on `stock` baked through `couplers`, at rest on no paper, as a CPU chain;
/// a slide projected.
pub struct Render {
    stock: film::Stock,
    projected: Option<&'static film::projection::Projected>,
    white: ColorMatrix,
    k47: Stage47,
    to_2020: M3,
    from_2020: M3,
}
impl Render {
    pub fn new(key: &'static str, couplers: M3) -> Render {
        Render::of(key, film::bake_through(&VirtualStock::of(key).entry(), couplers).unwrap())
    }
    /// The render of `key` on `stock`, read on that stock's own reference.
    pub fn of(key: &'static str, stock: film::Stock) -> Render {
        let v = VirtualStock::of(key);
        let mut s = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(v);
        s.gains.stops = stock.reference.stops;
        s.levels = stock.reference.levels();
        let to_2020 = matmul(&inv(&M2020), &SRGB_TO_XYZ);
        let projected = v.entry().reversal.then(|| film::projection::projected(v));
        Render { white: conversion::scene_white(&s).unwrap(), k47: conversion::stage47(&s), to_2020, from_2020: inv(&to_2020), stock, projected }
    }
    /// Scene light in linear Rec. 2020 to the print's display code.
    pub fn scene(&self, p: V3) -> V3 {
        let lit = cpu::color_matrix([p[0] as f32, p[1] as f32, p[2] as f32, 1.0], &self.white);
        let neg = self.stock.shot(lit, 0.0);
        let out = match self.projected {
            Some(p) => cpu::projection(neg, &p.projection, &p.table),
            None => cpu::stage47(neg, &self.k47),
        };
        mul(&self.from_2020, [out[0] as f64, out[1] as f64, out[2] as f64]).map(enc)
    }
    /// A display code read as the scene it encodes.
    pub fn code(&self, c: V3) -> V3 {
        self.scene(mul(&self.to_2020, c.map(dec)))
    }
}

/// The grey ramp's patches, display codes, the mid one the grey a render is matched at.
pub const RAMP: [f64; 9] = [0.1, 0.2, 0.3, 0.4, 0.4663, 0.55, 0.65, 0.75, 0.85];
/// Oklab to CIELAB on D65, through linear sRGB.
pub fn cielab(ok: V3) -> V3 {
    let [l, a, b] = ok;
    let (l_, m_, s_) = ((l + 0.3963377774 * a + 0.2158037573 * b).powi(3), (l - 0.1055613458 * a - 0.0638541728 * b).powi(3), (l - 0.0894841775 * a - 1.2914855480 * b).powi(3));
    let rgb = [4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_, -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_, -0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_];
    let xyz = mul(&SRGB_TO_XYZ, rgb);
    let f = |t: f64| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
    let [x, y, z] = [f(xyz[0] / 0.95047), f(xyz[1]), f(xyz[2] / 1.08883)];
    [116.0 * y - 16.0, 500.0 * (x - y), 200.0 * (y - z)]
}
