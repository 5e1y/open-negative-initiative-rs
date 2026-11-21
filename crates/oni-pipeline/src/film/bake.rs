//! A profile baked into the virtual negative's tables: the light per chromaticity on the film's band-pass
//! window, its input gamut compressed; the curves before and after the couplers; the scan's dyes and weights.

use super::cie::{self, WAVELENGTHS};
use super::profile::{Profile, Spectra};

pub type M3 = [[f64; 3]; 3];

pub fn mul(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|r| std::array::from_fn(|c| (0..3).map(|k| a[r][k] * b[k][c]).sum()))
}

pub fn apply(m: &M3, v: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|r| (0..3).map(|k| m[r][k] * v[k]).sum())
}

pub fn inverse(m: &M3) -> M3 {
    let [[a, b, c], [d, e, f], [g, h, i]] = *m;
    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    [
        [(e * i - f * h) / det, (c * h - b * i) / det, (b * f - c * e) / det],
        [(f * g - d * i) / det, (a * i - c * g) / det, (c * d - a * f) / det],
        [(d * h - e * g) / det, (b * g - a * h) / det, (a * e - b * d) / det],
    ]
}

fn diagonal(v: [f64; 3]) -> M3 {
    std::array::from_fn(|r| std::array::from_fn(|c| if r == c { v[r] } else { 0.0 }))
}

/// A light's XYZ on the 1931 observer, its Y at 1.
pub fn white_of(spd: &[f64; WAVELENGTHS]) -> [f64; 3] {
    let sum = |cmf: &[f64; WAVELENGTHS]| (0..WAVELENGTHS).map(|l| spd[l] * cmf[l]).sum::<f64>();
    let y = sum(&cie::Y);
    [sum(&cie::X) / y, 1.0, sum(&cie::Z) / y]
}

pub fn xy_of(xyz: [f64; 3]) -> [f64; 2] {
    let s = xyz[0] + xyz[1] + xyz[2];
    [xyz[0] / s, xyz[1] / s]
}

/// A full von Kries adaptation from white `from` to white `to` in the cone space `m`.
fn adaptation(m: &M3, from: [f64; 3], to: [f64; 3]) -> M3 {
    let (a, b) = (apply(m, from), apply(m, to));
    mul(&inverse(m), &mul(&diagonal(std::array::from_fn(|k| b[k] / a[k])), m))
}

/// CAT16's cone space (Li et al. 2017), the adaptation spektrafilm exposes its films through.
const CAT16: M3 = [[0.401288, 0.650173, -0.051461], [-0.250268, 1.204414, 0.045854], [-0.002079, 0.048952, 0.953127]];
const BRADFORD: M3 = [[0.8951, 0.2664, -0.1614], [-0.7502, 1.7135, 0.0367], [0.0389, -0.0685, 1.0296]];

/// The D65 white the working space's encoding stands on.
pub const D65_XY: [f64; 2] = [0.3127, 0.3290];

fn xyz_of_xy(xy: [f64; 2]) -> [f64; 3] {
    [xy[0] / xy[1], 1.0, (1.0 - xy[0] - xy[1]) / xy[1]]
}

/// Linear Rec. 2020 on D65 to XYZ.
pub fn rec2020_to_xyz() -> M3 {
    let primaries = [[0.708, 0.292], [0.170, 0.797], [0.131, 0.046]].map(xyz_of_xy);
    let m: M3 = std::array::from_fn(|r| std::array::from_fn(|c| primaries[c][r]));
    let s = apply(&inverse(&m), xyz_of_xy(D65_XY));
    std::array::from_fn(|r| std::array::from_fn(|c| m[r][c] * s[c]))
}

/// The working space to XYZ adapted by CAT16 to the film's reference white: a working grey lands on it.
pub fn to_film(reference: &[f64; WAVELENGTHS]) -> M3 {
    mul(&adaptation(&CAT16, xyz_of_xy(D65_XY), white_of(reference)), &rec2020_to_xyz())
}

/// The scan's weights: the light table's light through each wavelength, read on the 1931 observer, the
/// table's white Bradford-adapted to the working D65 and its Y at 1, in Rec. 2020.
pub fn scan_weights(light: &[f64; WAVELENGTHS]) -> Vec<[f64; 3]> {
    let to_rgb = mul(&inverse(&rec2020_to_xyz()), &adaptation(&BRADFORD, white_of(light), xyz_of_xy(D65_XY)));
    let y: f64 = (0..WAVELENGTHS).map(|l| light[l] * cie::Y[l]).sum();
    (0..WAVELENGTHS).map(|l| apply(&to_rgb, [cie::X[l], cie::Y[l], cie::Z[l]].map(|v| v * light[l] / y))).collect()
}

/// Abramowitz and Stegun's 7.1.26, within 1.5e-7 of the error function.
fn erf(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
    let poly = t * (0.254_829_592 + t * (-0.284_496_736 + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    (1.0 - poly * (-x * x).exp()).copysign(x)
}

pub fn wavelength(l: usize) -> f64 {
    cie::FIRST_NM + cie::STEP_NM * l as f64
}

/// The film's sensitivities through its band-pass window, each channel's response to its reference white kept.
pub fn windowed_sensitivity(p: &Profile, reference: &[f64; WAVELENGTHS]) -> Vec<[f64; 3]> {
    let [c_uv, s_uv, c_ir, s_ir] = p.window;
    let window: Vec<f64> = (0..WAVELENGTHS)
        .map(|l| {
            let w = wavelength(l);
            0.25 * (1.0 + erf((w - c_uv) / (s_uv * std::f64::consts::SQRT_2))) * (1.0 - erf((w - c_ir) / (s_ir * std::f64::consts::SQRT_2)))
        })
        .collect();
    let sensitivity: Vec<[f64; 3]> = p.log_sensitivity.iter().map(|s| s.map(|v| 10f64.powf(v))).collect();
    let kept: [f64; 3] = std::array::from_fn(|c| {
        let through: f64 = (0..WAVELENGTHS).map(|l| sensitivity[l][c] * reference[l] * window[l]).sum();
        let bare: f64 = (0..WAVELENGTHS).map(|l| sensitivity[l][c] * reference[l]).sum();
        through / bare
    });
    (0..WAVELENGTHS).map(|l| std::array::from_fn(|c| sensitivity[l][c] * window[l] / kept[c])).collect()
}

/// The table's square coordinates to CIE xy, and back, each held on 0…1.
pub fn xy_of_square(t: [f64; 2]) -> [f64; 2] {
    let r = t[0].max(0.0).sqrt();
    [1.0 - r, t[1] * r]
}

pub fn square_of_xy(xy: [f64; 2]) -> [f64; 2] {
    let tx = (1.0 - xy[0]) * (1.0 - xy[0]);
    let ty = xy[1] / (1.0 - xy[0]).max(1e-10);
    [tx.clamp(0.0, 1.0), ty.clamp(0.0, 1.0)]
}

/// The spectral locus, 380 to 700 nm, closed.
fn locus() -> Vec<[f64; 2]> {
    let mut out: Vec<[f64; 2]> = (0..=64).map(|l| xy_of([cie::X[l], cie::Y[l], cie::Z[l]])).collect();
    out.push(out[0]);
    out
}

/// How far along `direction` from `origin` the closed polygon is first met.
fn ray_reach(origin: [f64; 2], direction: [f64; 2], polygon: &[[f64; 2]]) -> f64 {
    let mut nearest = f64::INFINITY;
    for edge in polygon.windows(2) {
        let (a, b) = (edge[0], edge[1]);
        let e = [b[0] - a[0], b[1] - a[1]];
        let denom = direction[0] * e[1] - direction[1] * e[0];
        if denom.abs() <= 1e-12 {
            continue;
        }
        let o = [origin[0] - a[0], origin[1] - a[1]];
        let t = (-o[0] * e[1] + o[1] * e[0]) / denom;
        let s = (-o[0] * direction[1] + o[1] * direction[0]) / denom;
        if t > 1e-9 && (0.0..=1.0).contains(&s) && t < nearest {
            nearest = t;
        }
    }
    nearest
}

/// spektrafilm's input gamut compression: each chromaticity's distance from the white, as a share of the
/// locus' along its hue, through a Reinhard knee of power 6 that meets the locus at infinity.
pub fn compressed(xy: [f64; 2], white: [f64; 2], polygon: &[[f64; 2]]) -> [f64; 2] {
    let delta = [xy[0] - white[0], xy[1] - white[1]];
    let dist = delta[0].hypot(delta[1]);
    if dist < 1e-9 {
        return xy;
    }
    let direction = [delta[0] / dist, delta[1] / dist];
    let reach = ray_reach(white, direction, polygon).max(1e-12);
    let d = dist / reach;
    let knee = d / (1.0 + d.powi(6)).powf(1.0 / 6.0);
    [white[0] + direction[0] * knee * reach, white[1] + direction[1] * knee * reach]
}

fn bilinear(table: &[[f64; 3]], n: usize, at: [f64; 2]) -> [f64; 3] {
    let f = at.map(|v| (v * (n - 1) as f64).clamp(0.0, (n - 1) as f64));
    let lo = f.map(|v| (v.floor() as usize).min(n - 2));
    let t = [f[0] - lo[0] as f64, f[1] - lo[1] as f64];
    let node = |i: usize, j: usize| table[i * n + j];
    std::array::from_fn(|c| {
        let low = node(lo[0], lo[1])[c] * (1.0 - t[0]) + node(lo[0] + 1, lo[1])[c] * t[0];
        let high = node(lo[0], lo[1] + 1)[c] * (1.0 - t[0]) + node(lo[0] + 1, lo[1] + 1)[c] * t[0];
        low * (1.0 - t[1]) + high * t[1]
    })
}

/// The film's light per chromaticity node `(i, j)` at `i·n + j`, its gamut compression laid in, so a
/// lookup at a chromaticity reads what the uncompressed table holds at its compressed one.
pub fn light_table(spectra: &Spectra, sensitivity: &[[f64; 3]], reference: &[f64; WAVELENGTHS]) -> Vec<[f64; 3]> {
    let n = spectra.nodes;
    let plain: Vec<[f64; 3]> = (0..n * n)
        .map(|k| {
            let s = spectra.at(k / n, k % n);
            std::array::from_fn(|c| (0..WAVELENGTHS).map(|l| s[l] * sensitivity[l][c]).sum())
        })
        .collect();
    let white = xy_of(white_of(reference));
    let polygon = locus();
    (0..n * n)
        .map(|k| {
            let square = [(k / n) as f64 / (n - 1) as f64, (k % n) as f64 / (n - 1) as f64];
            bilinear(&plain, n, square_of_xy(compressed(xy_of_square(square), white, &polygon)))
        })
        .collect()
}

/// spektrafilm's DIR couplers on a colour negative: the row layer's density inhibits the column's.
pub const COUPLERS: M3 = [[0.336, 0.353, 0.302], [0.154, 0.319, 0.353], [0.168, 0.226, 0.273]];

/// The curves above their least density, then the curves before the couplers act, so a grey
/// developed through the couplers lands on the data sheet's curve.
pub fn curves(p: &Profile) -> (Vec<[f64; 3]>, Vec<[f64; 3]>) {
    let low: [f64; 3] = std::array::from_fn(|c| p.density_curves.iter().map(|d| d[c]).fold(f64::INFINITY, f64::min));
    let normal: Vec<[f64; 3]> = p.density_curves.iter().map(|d| std::array::from_fn(|c| d[c] - low[c])).collect();
    let le = &p.log_exposure;
    let before = (0..3)
        .map(|c| {
            let shifted: Vec<f64> = (0..le.len()).map(|i| le[i] - (0..3).map(|k| normal[i][k] * COUPLERS[k][c]).sum::<f64>()).collect();
            le.iter().map(|&e| interpolate(e, &shifted, &normal, c)).collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    (normal, (0..le.len()).map(|i| [before[0][i], before[1][i], before[2][i]]).collect())
}

/// `np.interp`: channel `c` of `ys` at `x` along increasing `xs`, held at the ends.
pub fn interpolate(x: f64, xs: &[f64], ys: &[[f64; 3]], c: usize) -> f64 {
    let n = xs.len();
    if x <= xs[0] {
        return ys[0][c];
    }
    if x >= xs[n - 1] {
        return ys[n - 1][c];
    }
    let hi = xs.partition_point(|v| *v <= x).min(n - 1);
    let lo = hi - 1;
    let t = (x - xs[lo]) / (xs[hi] - xs[lo]);
    ys[lo][c] + t * (ys[hi][c] - ys[lo][c])
}

/// The scanned band: the wavelengths the profile measures both the dyes and the base on, each with
/// the dyes' and base's densities; outside it the scan reads no light, as spektrafilm's does.
pub fn scanned(p: &Profile) -> Vec<(usize, [f64; 4])> {
    (0..WAVELENGTHS)
        .filter_map(|l| {
            let [c, m, y] = p.channel_density[l];
            Some((l, [c?, m?, y?, p.base_density[l]?]))
        })
        .collect()
}
