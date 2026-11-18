//! Halation: the light of the highlights crossing the film's base and coming back to its red layer,
//! a little to its green, as a wide glow hugging their edges.

use super::*;

/// The scene light past which a highlight halates at rest, in the decode's linear units.
pub const HALATION_THRESHOLD: f32 = 0.6;
/// How much of each channel's light past the threshold comes back: the red layer lies on the base.
pub const HALATION_COLOUR: [f32; 3] = [1.0, 0.3, 0.0];

/// `halation(s, blurred)`: `amount` of the light past `threshold` moved from where it fell to its glow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Halation {
    pub amount: f32,
    pub threshold: f32,
}

/// The light a pixel sends back through the base past `threshold`, each blur tap's step before it is summed.
pub fn halation_source(s: V4, threshold: f32) -> V4 {
    with_alpha(map(rgb(s), |c, v| (v - threshold).max(0.0) * HALATION_COLOUR[c]), 0.0)
}

/// The pixel with its glow, `blurred` the source's blur: what the source keeps it leaves where it fell.
pub fn halation(s: V4, blurred: V4, k: &Halation) -> V4 {
    let own = halation_source(s, k.threshold);
    with_alpha(map(rgb(s), |c, v| v + k.amount * (blurred[c] - own[c])), s[3])
}
