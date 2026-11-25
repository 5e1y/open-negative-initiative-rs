//! The neutral negative: a film of no colour of its own, each layer's density a colour negative's mean tone curve of its
//! own channel's light, so its window reads as a film's, its bare film black and the sensor's white its white.

use super::*;

/// The nodes of the neutral film's tone curve, evenly spaced in log exposure.
pub const NEUTRAL_NODES: usize = 64;

/// The neutral film's exposure and development: no spectrum, no coupler, pure dyes on a clear bare film.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NeutralNegative {
    /// One over the scene's light the window's white stands on, the film's exposure laid.
    pub per_white: f32,
    /// The density from the bare film to that white.
    pub range: f32,
    /// The pedestal its reading adds to a scan, taken off this one: the density read is the curve's exactly.
    pub veil: f32,
    /// The curve's first node, in log of the light over the white, and the step between two.
    pub low: f32,
    pub step: f32,
    /// The density above the bare film at each node, one at the white.
    pub curve: [f32; NEUTRAL_NODES],
}

impl NeutralNegative {
    pub fn rows(&self) -> Vec<V4> {
        let mut rows = vec![[self.per_white, self.range, self.veil, self.low], [self.step, 0.0, 0.0, 0.0]];
        rows.extend(self.curve.chunks(4).map(|c| [c[0], c[1], c[2], c[3]]));
        rows
    }

    /// The curve at `x`, the light over the white: held at its ends, none under its foot.
    pub fn tone(&self, x: f32) -> f32 {
        let f = clamp((x.max(1e-30).log10() - self.low) / self.step, 0.0, (NEUTRAL_NODES - 1) as f32);
        let i = (f.floor() as usize).min(NEUTRAL_NODES - 2);
        mix(self.curve[i], self.curve[i + 1], f - i as f32)
    }
}

/// The screen's code of light `x`, sRGB's, carried straight on under zero and by its own power past 1.
pub fn screen_code(x: f32) -> f32 {
    if x <= 0.003_130_8 { 12.92 * x } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 }
}

/// Each layer's density above the bare film one scene pixel develops.
pub fn neutral_dyes(s: V4, k: &NeutralNegative) -> V3 {
    map(rgb(s), |_, v| k.range * k.tone(v * k.per_white))
}

/// One scene pixel to the light its negative's scan holds, each layer's density on its own channel; alpha kept.
pub fn neutral_negative(s: V4, k: &NeutralNegative) -> V4 {
    with_alpha(map(neutral_dyes(s, k), |_, d| (1.0 + k.veil) * (-std::f32::consts::LOG2_10 * d).exp2() - k.veil), s[3])
}
