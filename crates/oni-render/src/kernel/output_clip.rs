//! The output clip (`pipeline.metal`).

use super::*;

pub fn output_clip(s: V4) -> V4 {
    with_alpha(map(rgb(s), |_, x| clamp(x, 0.0, 1.0)), s[3])
}
