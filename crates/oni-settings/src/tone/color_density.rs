//! One travel raising every channel to a power and undoing it on the luminance alone.

use super::V3;
use crate::axis::pipeline;

pub const RANGE: (f32, f32) = (-1.0, 1.0);
pub const BASE: f32 = 0.0;
pub const FULL_POWER: f32 = 2.0;

pub fn factor(travel: f32) -> f32 {
    libm::powf(FULL_POWER, travel)
}

pub fn rendered(rgb: V3, travel: f32) -> V3 {
    let m = factor(travel);
    if m == 1.0 {
        return rgb;
    }
    let p = rgb.map(|c| libm::powf(c.max(0.0), m));
    let y = pipeline::luma(p);
    if y <= 1e-12 {
        return rgb;
    }
    let k = libm::powf(y, (1.0 / m) - 1.0);
    p.map(|c| c * k)
}
