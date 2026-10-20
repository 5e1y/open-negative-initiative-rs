//! What the Lab is judged by: Oklab of display code, ΔE as Oklab distance × 100, and the measured
//! cube's nodes that a reading constrains.

use super::node;
use crate::scanner::MACHINE;

/// The cube's lattice side.
pub const SIDE: usize = 27;
/// Past this spread between channels a node is mostly smoothing, not measurement.
pub const SPREAD: f64 = 0.6;

pub fn linear(code: f64) -> f64 {
    let c = code.abs();
    let l = if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
    l.copysign(code)
}

pub fn encoded(l: f64) -> f64 {
    let c = l.abs();
    let v = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    v.copysign(l)
}

pub fn oklab(rgb: [f64; 3]) -> [f64; 3] {
    let l = (0.412_221_470_8 * rgb[0] + 0.536_332_536_3 * rgb[1] + 0.051_445_992_9 * rgb[2]).cbrt();
    let m = (0.211_903_498_2 * rgb[0] + 0.680_699_545_1 * rgb[1] + 0.107_396_956_6 * rgb[2]).cbrt();
    let s = (0.088_302_461_9 * rgb[0] + 0.281_718_837_6 * rgb[1] + 0.629_978_700_5 * rgb[2]).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
    ]
}

pub fn linear_rgb(lab: [f64; 3]) -> [f64; 3] {
    let l = (lab[0] + 0.396_337_777_4 * lab[1] + 0.215_803_757_3 * lab[2]).powi(3);
    let m = (lab[0] - 0.105_561_345_8 * lab[1] - 0.063_854_172_8 * lab[2]).powi(3);
    let s = (lab[0] - 0.089_484_177_5 * lab[1] - 1.291_485_548_0 * lab[2]).powi(3);
    [
        4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
        -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
        -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701_0 * s,
    ]
}

/// Oklab of a display code, clipped as the display reads it.
pub fn lab_of(code: [f64; 3]) -> [f64; 3] {
    oklab(code.map(|v| linear(v.clamp(0.0, 1.0))))
}

pub fn delta_e(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (p, q) = (lab_of(a), lab_of(b));
    100.0 * ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
}

/// Lightness, chroma and hue in degrees from 0 to 360.
pub fn lch(lab: [f64; 3]) -> [f64; 3] {
    let h = lab[2].atan2(lab[1]).to_degrees();
    [lab[0], lab[1].hypot(lab[2]), if h < 0.0 { h + 360.0 } else { h }]
}

/// A hue difference wrapped to −180 … 180.
pub fn turn(to: f64, from: f64) -> f64 {
    (to - from + 540.0).rem_euclid(360.0) - 180.0
}

/// The value under which a share `q` of `v` falls.
pub fn centile(v: &[f64], q: f64) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    s[((s.len() - 1) as f64 * q).round() as usize]
}

pub fn median(v: &[f64]) -> f64 {
    centile(v, 0.5)
}

/// The measured cube's code at lattice node (r, g, b).
pub fn cube_at(r: usize, g: usize, b: usize) -> [f64; 3] {
    let v = MACHINE.render.cube_data[(b * SIDE + g) * SIDE + r];
    [v[0], v[1], v[2]].map(f64::from)
}

/// Every lattice node inside the reach of the measured negatives, as (density, the cube's code).
pub fn reached() -> Vec<([f64; 3], [f64; 3])> {
    let (low, high) = MACHINE.reached();
    let mut out = Vec::new();
    for b in 0..SIDE {
        for g in 0..SIDE {
            for r in 0..SIDE {
                let x = [r, g, b].map(|k| node(k, SIDE));
                if (0..3).all(|c| x[c] >= f64::from(low[c]) - 1e-9 && x[c] <= f64::from(high[c]) + 1e-9) {
                    out.push((x, cube_at(r, g, b)));
                }
            }
        }
    }
    out
}

/// The reached nodes whose channels sit within `SPREAD` of each other: every one is read by a measurement.
pub fn constrained() -> Vec<([f64; 3], [f64; 3])> {
    reached().into_iter().filter(|(x, _)| x.iter().cloned().fold(f64::MIN, f64::max) - x.iter().cloned().fold(f64::MAX, f64::min) < SPREAD).collect()
}
