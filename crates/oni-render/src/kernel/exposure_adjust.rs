//! Core Image's `CIExposureAdjust`.

use super::*;

/// `CIExposureAdjust`: the colour times `2^ev`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExposureAdjust {
    pub ev: f32,
}

pub fn exposure_adjust(s: V4, k: &ExposureAdjust) -> V4 {
    let g = k.ev.exp2();
    with_alpha(map(rgb(s), |_, v| v * g), s[3])
}
