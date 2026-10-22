//! The saturation curve (`chroma.metal`).

use super::*;

/// `saturationCurve(s, knots, lows, high, bends)`, as `SaturationCurve.arguments` packs them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SaturationCurve {
    pub knots: V3,
    pub lows: V4,
    pub high: f32,
    pub bends: V3,
}

impl SaturationCurve {
    pub fn rows(&self) -> Vec<V4> {
        vec![v3(self.knots, self.high), self.lows, v3(self.bends, 0.0)]
    }
}

pub fn saturation_curve(s: V4, k: &SaturationCurve) -> V4 {
    let y = dot(rgb(s), LUMA);
    let x = clamp(y.max(0.0).powf(1.0 / 2.2), 0.0, 1.0);
    let xs = [0.0, k.knots[0], k.knots[1], k.knots[2], 1.0];
    let ys = [k.lows[0], k.lows[1], k.lows[2], k.lows[3], k.high];
    let ms = [0.0, k.bends[0], k.bends[1], k.bends[2], 0.0];
    let i = if x < xs[1] { 0 } else if x < xs[2] { 1 } else if x < xs[3] { 2 } else { 3 };
    let h = (xs[i + 1] - xs[i]).max(1e-6);
    let t = x - xs[i];
    let b = (ys[i + 1] - ys[i]) / h - h * (2.0 * ms[i] + ms[i + 1]) / 6.0;
    let value = ys[i] + b * t + ms[i] / 2.0 * t * t + (ms[i + 1] - ms[i]) / (6.0 * h) * t * t * t;
    let factor = (2.0 * value).max(0.0);
    let chroma = map(rgb(s), |_, c| (c - y) * factor);
    let lost = dot(chroma, LUMA);
    with_alpha(map(chroma, |_, c| y + (c - lost)), s[3])
}
