//! Core Image's `CIColorClamp`.

use super::*;

/// `CIColorClamp`: every component held inside its pair.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorClamp {
    pub min_components: V4,
    pub max_components: V4,
}

impl Default for ColorClamp {
    fn default() -> Self {
        Self { min_components: [0.0; 4], max_components: [1.0; 4] }
    }
}

pub fn color_clamp(s: V4, k: &ColorClamp) -> V4 {
    std::array::from_fn(|c| clamp(s[c], k.min_components[c], k.max_components[c]))
}

const COLOR_CLAMP: Shader = shader!("color_clamp");

impl Gpu {
    pub fn color_clamp(&self, image: &Image, k: &ColorClamp) -> Image {
        self.per_pixel(COLOR_CLAMP, |_| vec![k.min_components, k.max_components], &[image], &[], 0)
    }
}
