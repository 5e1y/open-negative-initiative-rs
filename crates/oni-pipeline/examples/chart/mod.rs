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
/// Oklch to linear sRGB.
pub fn oklch(l: f64, c: f64, h: f64) -> V3 {
    let (a, b) = (c * h.cos(), c * h.sin());
    let l_ = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m_ = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s_ = (l - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    [4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_, -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_, -0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_]
}

pub struct Cube {
    n: usize,
    nodes: Vec<V3>,
}
impl Cube {
    pub fn read(path: &str) -> Cube {
        let (_, look) = oni_settings::stocks::cube::read(&std::fs::read(path).unwrap()).unwrap();
        Cube { n: look.size, nodes: look.nodes.iter().map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]).collect() }
    }
    pub fn at(&self, c: V3) -> V3 {
        let n = self.n;
        let p = c.map(|v| v.clamp(0.0, 1.0) * (n - 1) as f64);
        let i = p.map(|v| (v.floor() as usize).min(n - 2));
        let f: V3 = std::array::from_fn(|k| p[k] - i[k] as f64);
        let mut out = [0.0; 3];
        for (db, dg, dr) in (0..8).map(|q| (q >> 2 & 1, q >> 1 & 1, q & 1)) {
            let w = (if db == 1 { f[2] } else { 1.0 - f[2] }) * (if dg == 1 { f[1] } else { 1.0 - f[1] }) * (if dr == 1 { f[0] } else { 1.0 - f[0] });
            let node = self.nodes[(i[0] + dr) + n * (i[1] + dg) + n * n * (i[2] + db)];
            (0..3).for_each(|k| out[k] += w * node[k]);
        }
        out
    }
}

/// A digital photo's render on `stock` baked through `couplers`, at rest on the enlarger with no paper, as a CPU chain;
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
        s.print_paper.get_or_insert_with(Default::default);
        s.paperless = true;
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
/// The ColorChecker's chromatic patches, first on the chart.
pub const CHECKER: usize = 18;

/// The chart's colours: the ColorChecker's chromatic patches, Oklch rings held in sRGB and the grey ramp, as display
/// codes.
pub fn colours() -> Vec<V3> {
    let cc: [V3; 18] = [
        [37.54, 14.37, 14.92], [64.66, 19.27, 17.5], [49.32, -3.82, -22.54], [43.46, -12.74, 22.72], [54.94, 9.61, -24.79], [70.48, -32.26, -0.37],
        [62.73, 35.83, 56.5], [39.43, 10.75, -45.17], [50.57, 48.64, 16.67], [30.1, 22.54, -20.87], [71.77, -24.13, 58.19], [71.51, 18.24, 67.37],
        [28.37, 15.42, -49.8], [54.38, -39.72, 32.27], [42.43, 51.05, 28.62], [81.8, 2.67, 80.41], [50.63, 51.28, -14.12], [49.57, -29.71, -28.32],
    ];
    let brad: M3 = [[0.9555766, -0.0230393, 0.0631636], [-0.0282895, 1.0099416, 0.0210077], [0.0122982, -0.0204830, 1.3299098]];
    let to_srgb = inv(&SRGB_TO_XYZ);
    let mut out: Vec<V3> = cc
        .iter()
        .map(|&[l, a, b]| {
            let fy = (l + 16.0) / 116.0;
            let f = |t: f64| if t.powi(3) > 0.008856 { t.powi(3) } else { (t - 16.0 / 116.0) / 7.787 };
            let xyz = [0.96422 * f(fy + a / 500.0), f(fy), 0.82521 * f(fy - b / 200.0)];
            mul(&to_srgb, mul(&brad, xyz)).map(enc)
        })
        .collect();
    for l in [0.5, 0.65, 0.8] {
        for c in [0.05, 0.1] {
            for h in 0..12 {
                let lin = oklch(l, c, h as f64 * std::f64::consts::TAU / 12.0);
                if lin.iter().all(|v| (0.0..=1.0).contains(v)) {
                    out.push(lin.map(enc));
                }
            }
        }
    }
    out.extend(RAMP.map(|v| [v; 3]));
    out
}

/// A display code's linear light scaled by `k` channel by channel, as Oklab.
pub fn scaled(code: V3, k: V3) -> V3 {
    oklab(std::array::from_fn(|c| enc(dec(code[c]) * k[c])))
}

/// The chart as `render` prints it, each channel's light scaled so the ramp's mid grey matches the reference's, as Oklab: a
/// render is judged on its colours, its grey's exposure and balance set as the reference's.
pub fn printed(render: &Render, chart: &[V3], cube: &Cube) -> Vec<V3> {
    let grey = [RAMP[4]; 3];
    let (want, have) = (cube.at(grey), render.code(grey));
    let k: V3 = std::array::from_fn(|c| dec(want[c]) / dec(have[c]).max(1e-9));
    chart.iter().map(|c| scaled(render.code(*c), k)).collect()
}

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

/// CIEDE2000 between two CIELAB colours.
pub fn de2000(p: V3, q: V3) -> f64 {
    let (rad, deg) = (std::f64::consts::PI / 180.0, 180.0 / std::f64::consts::PI);
    let c_mean = (p[1].hypot(p[2]) + q[1].hypot(q[2])) / 2.0;
    let g = 0.5 * (1.0 - (c_mean.powi(7) / (c_mean.powi(7) + 25f64.powi(7))).sqrt());
    let (a1, a2) = ((1.0 + g) * p[1], (1.0 + g) * q[1]);
    let (c1, c2) = (a1.hypot(p[2]), a2.hypot(q[2]));
    let hue = |b: f64, a: f64| (b.atan2(a) * deg).rem_euclid(360.0);
    let (h1, h2) = (hue(p[2], a1), hue(q[2], a2));
    let dh = if c1 * c2 == 0.0 { 0.0 } else if (h2 - h1).abs() <= 180.0 { h2 - h1 } else if h2 - h1 > 180.0 { h2 - h1 - 360.0 } else { h2 - h1 + 360.0 };
    let big_dh = 2.0 * (c1 * c2).sqrt() * (dh / 2.0 * rad).sin();
    let (l_mean, cp_mean) = ((p[0] + q[0]) / 2.0, (c1 + c2) / 2.0);
    let h_mean = if c1 * c2 == 0.0 { h1 + h2 } else if (h1 - h2).abs() <= 180.0 { (h1 + h2) / 2.0 } else if h1 + h2 < 360.0 { (h1 + h2 + 360.0) / 2.0 } else { (h1 + h2 - 360.0) / 2.0 };
    let t = 1.0 - 0.17 * ((h_mean - 30.0) * rad).cos() + 0.24 * (2.0 * h_mean * rad).cos() + 0.32 * ((3.0 * h_mean + 6.0) * rad).cos() - 0.20 * ((4.0 * h_mean - 63.0) * rad).cos();
    let theta = 30.0 * (-((h_mean - 275.0) / 25.0).powi(2)).exp();
    let rc = 2.0 * (cp_mean.powi(7) / (cp_mean.powi(7) + 25f64.powi(7))).sqrt();
    let sl = 1.0 + 0.015 * (l_mean - 50.0).powi(2) / (20.0 + (l_mean - 50.0).powi(2)).sqrt();
    let (sc, sh) = (1.0 + 0.045 * cp_mean, 1.0 + 0.015 * cp_mean * t);
    let rt = -(2.0 * theta * rad).sin() * rc;
    let (dl, dc, dhh) = ((q[0] - p[0]) / sl, (c2 - c1) / sc, big_dh / sh);
    (dl * dl + dc * dc + dhh * dhh + rt * dc * dhh).sqrt()
}

/// A printed chart against the reference's: mean ΔE00 and mean |ΔL*| over the colours, the ColorChecker's chroma over the
/// reference's, the ramp's mean C* and the reference's own.
pub fn judged(printed: &[V3], targets: &[V3]) -> [f64; 5] {
    let (p, t): (Vec<V3>, Vec<V3>) = (printed.iter().map(|v| cielab(*v)).collect(), targets.iter().map(|v| cielab(*v)).collect());
    let colours = targets.len() - RAMP.len();
    let mean = |f: &dyn Fn(usize) -> f64| (0..colours).map(f).sum::<f64>() / colours as f64;
    let chroma = |v: &[V3]| v[..CHECKER].iter().map(|c| c[1].hypot(c[2])).sum::<f64>();
    let ramp = |v: &[V3]| v[colours..].iter().map(|c| c[1].hypot(c[2])).sum::<f64>() / RAMP.len() as f64;
    [mean(&|i| de2000(t[i], p[i])), mean(&|i| (t[i][0] - p[i][0]).abs()), chroma(&p) / chroma(&t), ramp(&p), ramp(&t)]
}
