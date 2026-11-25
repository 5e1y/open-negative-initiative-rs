//! Halation: the light a film's base sends back into its layers, each layer's exposure joined by a dose of its blur
//! before the film's curve, which alone shows it around the highlights.

use super::*;

/// The share of the dose each colour layer takes: the red one lies on the base, the blue one behind the yellow filter.
pub const HALATION_COLOUR: [f32; 3] = [1.0, 0.3, 0.0];

/// The halation's dose on each layer, a share of the exposure its base sends back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Halation {
    pub dose: [f32; 3],
}

impl Halation {
    /// `dose` on a colour film's layers, or on a silver film's one layer, the same on its three channels.
    pub fn on(dose: f32, silver: bool) -> Halation {
        Halation { dose: if silver { [dose; 3] } else { HALATION_COLOUR.map(|c| dose * c) } }
    }

    pub fn rows(&self) -> Vec<V4> {
        vec![v3(self.dose, 0.0)]
    }
}

/// One pixel's exposures joined by `k`'s dose of their blur, over one and the dose: an even exposure stays as the
/// film's sheet measured it, halation included; alpha kept.
pub fn halation(e: V4, blurred: V4, k: &Halation) -> V4 {
    with_alpha(map(rgb(e), |c, v| (v + k.dose[c] * blurred[c]) / (1.0 + k.dose[c])), e[3])
}
