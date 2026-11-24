//! A slide's black held under Lighting: the lit picture laid back onto its rest over the deep shadows, none of the move
//! where the rest shows the film's black, the whole from a reach of light above it, a smoothstep between.

use super::*;

/// Where the screen's black lies at rest, as a luma, and the light above it from which Lighting acts whole.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeldBlack {
    pub black: f32,
    pub reach: f32,
}

impl HeldBlack {
    pub fn rows(&self) -> Vec<V4> {
        vec![[self.black, self.reach, 0.0, 0.0]]
    }
}

/// One pixel lit by Lighting, `lit`, laid back onto its `rest` by its rest's light over the black; alpha the lit one's.
pub fn held_black(lit: V4, rest: V4, k: &HeldBlack) -> V4 {
    let t = clamp((dot(rgb(rest), LUMA) - k.black) / k.reach, 0.0, 1.0);
    let share = t * t * (3.0 - 2.0 * t);
    with_alpha(map(rgb(lit), |c, v| rest[c] + share * (v - rest[c])), lit[3])
}
