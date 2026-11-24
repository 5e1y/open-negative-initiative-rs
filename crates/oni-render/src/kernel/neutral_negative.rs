//! The neutral negative: a film of no character, each layer's density the screen's code of its own channel's light, so
//! a window from its bare film to its white, read as that code, gives the scene's light back.

use super::*;

/// The neutral film's exposure and development: no spectrum, no coupler, pure dyes on a clear bare film.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NeutralNegative {
    /// One over the scene's light the code's 1 stands on, the film's exposure laid.
    pub per_white: f32,
    /// The density from the bare film to the code's 1.
    pub range: f32,
    /// The pedestal its reading adds to a scan, taken off this one: the density read is `range` times the code exactly.
    pub veil: f32,
}

impl NeutralNegative {
    pub fn rows(&self) -> Vec<V4> {
        vec![[self.per_white, self.range, self.veil, 0.0]]
    }
}

/// The screen's code of light `x`, sRGB's, carried straight on under zero and by its own power past 1.
pub fn screen_code(x: f32) -> f32 {
    if x <= 0.003_130_8 { 12.92 * x } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 }
}

/// Each layer's density above the bare film one scene pixel develops.
pub fn neutral_dyes(s: V4, k: &NeutralNegative) -> V3 {
    map(rgb(s), |_, v| k.range * screen_code(v * k.per_white))
}

/// One scene pixel to the light its negative's scan holds, each layer's density on its own channel; alpha kept.
pub fn neutral_negative(s: V4, k: &NeutralNegative) -> V4 {
    with_alpha(map(neutral_dyes(s, k), |_, d| (1.0 + k.veil) * (-std::f32::consts::LOG2_10 * d).exp2() - k.veil), s[3])
}
