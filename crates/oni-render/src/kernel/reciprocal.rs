//! A positive read as its own negative.

use super::*;

/// Each channel's light inverted about `k`, so a density reads flipped; under `floor` it holds there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reciprocal {
    pub k: f32,
    pub floor: f32,
}

pub fn reciprocal(s: V4, p: &Reciprocal) -> V4 {
    with_alpha(map(rgb(s), |_, v| p.k / v.max(p.floor)), s[3])
}
