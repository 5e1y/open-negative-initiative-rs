//! The output clip (`pipeline.metal`).

use super::*;

pub fn output_clip(s: V4) -> V4 {
    with_alpha(map(rgb(s), |_, x| clamp(x, 0.0, 1.0)), s[3])
}

const OUTPUT_CLIP: Shader = shader!("output_clip");

impl Gpu {
    /// Every per-pixel kernel writes its input's extent and reads its own pixel alone.
    pub fn output_clip(&self, image: &Image) -> Image {
        self.per_pixel(OUTPUT_CLIP, |_| Vec::new(), &[image], &[], 0)
    }
}
