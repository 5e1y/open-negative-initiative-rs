//! A virtual negative: a scene exposed on a film's spectral sensitivities, developed on its curves with
//! its DIR couplers, and read on a light table by a colorimetric camera, as a scan of that negative.

use super::*;

/// Most wavelengths the scan sums over: the uniform holds their dyes and weights.
pub const VIRTUAL_NEGATIVE_BANDS: usize = 64;

/// The film's exposure, development and scan. The table beside the input is `tc_nodes` square, the
/// light per chromaticity, then two rows of `curve_nodes`: each channel's density before the couplers act, then after.
#[derive(Clone, Debug, PartialEq)]
pub struct VirtualNegative {
    /// Working Rec. 2020 to XYZ adapted to the film's reference white, by rows.
    pub to_film: [V3; 3],
    /// The film's exposure as a factor of the scene's light.
    pub exposure: f32,
    /// The DIR couplers: the row's density lowers the column's log exposure.
    pub couplers: [V3; 3],
    /// The curves' first log exposure and the span to their last.
    pub log_low: f32,
    pub log_span: f32,
    pub tc_nodes: usize,
    pub curve_nodes: usize,
    /// Per scanned wavelength: the three dyes' density at unit amount and the base's, then the camera's
    /// Rec. 2020 weight of the light passed.
    pub dyes: Vec<V4>,
    pub weights: Vec<V3>,
}

impl VirtualNegative {
    pub fn head(&self) -> [u32; 4] {
        [self.tc_nodes as u32, self.curve_nodes as u32, self.dyes.len() as u32, 0]
    }

    pub fn rows(&self) -> Vec<V4> {
        let t = &self.to_film;
        let c = &self.couplers;
        let mut rows = vec![v3(t[0], self.exposure), v3(t[1], self.log_low), v3(t[2], self.log_span), v3(c[0], 0.0), v3(c[1], 0.0), v3(c[2], 0.0)];
        let pad = |n: usize| std::iter::repeat_n([0.0; 4], VIRTUAL_NEGATIVE_BANDS - n);
        rows.extend(self.dyes.iter().copied().chain(pad(self.dyes.len())));
        rows.extend(self.weights.iter().map(|w| v3(*w, 0.0)).chain(pad(self.weights.len())));
        rows
    }

    /// The table's width and height as one image.
    pub fn table_size(&self) -> (usize, usize) {
        (self.tc_nodes.max(self.curve_nodes), self.tc_nodes + 2)
    }
}

/// CIE xy to the table's square coordinates, each held on 0…1.
fn quad(x: f32, y: f32) -> [f32; 2] {
    let tx = (1.0 - x) * (1.0 - x);
    let ty = y / (1.0 - x).max(1e-10);
    [clamp(tx, 0.0, 1.0), clamp(ty, 0.0, 1.0)]
}

/// The film's light for chromaticity square `tc`, read bilinearly off the table.
fn light(tc: [f32; 2], k: &VirtualNegative, table: &[V4]) -> V3 {
    let (w, _) = k.table_size();
    let n = k.tc_nodes;
    let at = |i: usize, j: usize| rgb(table[j * w + i]);
    let f = tc.map(|v| v * (n - 1) as f32);
    let lo = f.map(|v| (v.floor() as usize).min(n - 2));
    let t = [f[0] - lo[0] as f32, f[1] - lo[1] as f32];
    let lerp = |a: V3, b: V3, t: f32| map(a, |c, v| mix(v, b[c], t));
    let low = lerp(at(lo[0], lo[1]), at(lo[0] + 1, lo[1]), t[0]);
    let high = lerp(at(lo[0], lo[1] + 1), at(lo[0] + 1, lo[1] + 1), t[0]);
    lerp(low, high, t[1])
}

/// Channel `c`'s density on curve row `row` at log exposure `e`, held at the curve's ends.
fn curve(e: f32, c: usize, row: usize, k: &VirtualNegative, table: &[V4]) -> f32 {
    let (w, _) = k.table_size();
    let m = k.curve_nodes;
    let f = clamp((e - k.log_low) / k.log_span, 0.0, 1.0) * (m - 1) as f32;
    let lo = (f.floor() as usize).min(m - 2);
    let base = (k.tc_nodes + row) * w;
    mix(table[base + lo][c], table[base + lo + 1][c], f - lo as f32)
}

/// One scene pixel to the light the camera reads through its negative; alpha kept.
pub fn virtual_negative(s: V4, k: &VirtualNegative, table: &[V4]) -> V4 {
    let xyz = k.to_film.map(|row| dot(row, rgb(s)));
    let b = xyz[0] + xyz[1] + xyz[2];
    let safe = b.max(1e-10);
    let raw = map(light(quad(xyz[0] / safe, xyz[1] / safe), k, table), |_, v| v * b * k.exposure);
    let log = map(raw, |_, v| (v.max(0.0) + 1e-10).log10());
    let first = map(log, |c, e| curve(e, c, 0, k, table));
    let inhibited = map(log, |m, e| e - (0..3).map(|d| first[d] * k.couplers[d][m]).sum::<f32>());
    let density = map(inhibited, |c, e| curve(e, c, 1, k, table));
    let mut out = [0.0f32; 3];
    for (dye, weight) in k.dyes.iter().zip(&k.weights) {
        let t = 10f32.powf(-(density[0] * dye[0] + density[1] * dye[1] + density[2] * dye[2] + dye[3]));
        out = map(out, |c, v| v + t * weight[c]);
    }
    with_alpha(out, s[3])
}
