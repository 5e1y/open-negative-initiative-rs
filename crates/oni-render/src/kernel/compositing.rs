//! Core Image's `CIBlendWithMask`.

use super::*;

pub fn blend_with_mask(image: V4, background: V4, mask: V4) -> V4 {
    std::array::from_fn(|c| mix(background[c], image[c], mask[1]))
}
