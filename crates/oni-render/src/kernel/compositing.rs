//! Core Image's `CIBlendWithMask` and `CISourceOverCompositing`.

use super::*;

pub fn blend_with_mask(image: V4, background: V4, mask: V4) -> V4 {
    std::array::from_fn(|c| mix(background[c], image[c], mask[1]))
}

pub fn source_over(image: V4, background: V4) -> V4 {
    std::array::from_fn(|c| image[c] + background[c] * (1.0 - image[3]))
}

const BLEND_WITH_MASK: Shader = shader!("blend_with_mask");

const SOURCE_OVER: Shader = shader!("source_over");

impl Gpu {
    /// The three on one extent: the caller has placed the patch and its mask on the background's.
    pub fn blend_with_mask(&self, image: &Image, background: &Image, mask: &Image) -> Image {
        self.per_pixel(BLEND_WITH_MASK, |_| Vec::new(), &[image, background, mask], &[], 0)
    }

    pub fn source_over(&self, image: &Image, background: &Image) -> Image {
        self.per_pixel(SOURCE_OVER, |_| Vec::new(), &[image, background], &[], 0)
    }
}
