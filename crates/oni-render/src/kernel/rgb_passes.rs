//! Three narrowband passes joined into one picture (`rgbPasses.metal`).

use super::*;

pub fn rgb_passes(red: V4, green: V4, blue: V4) -> V4 {
    [red[0], green[1], blue[2], 1.0]
}

const RGB_PASSES: Shader = shader!("rgb_passes");

impl Gpu {
    /// The three passes already laid on the green pass's extent, as `RGBPasses.composed` lays them.
    pub fn rgb_passes(&self, red: &Image, green: &Image, blue: &Image) -> Image {
        self.per_pixel(RGB_PASSES, |_| Vec::new(), &[red, green, blue], &[], 0)
    }
}
