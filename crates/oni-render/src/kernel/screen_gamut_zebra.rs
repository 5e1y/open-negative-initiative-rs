//! The screen-gamut zebra (`gamut.metal`).

use super::*;

/// `screenGamutZebra(picture, shown, tint, period, duty, phase, tolerance)`; `tint` in the working space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenGamutZebra {
    pub tint: V4,
    pub period: f32,
    pub duty: f32,
    pub phase: f32,
    pub tolerance: f32,
}

impl ScreenGamutZebra {
    pub fn rows(&self) -> Vec<V4> {
        vec![self.tint, [self.period, self.duty, self.phase, self.tolerance]]
    }
}

pub fn screen_gamut_zebra(picture: &Image, shown: &Image, k: &ScreenGamutZebra) -> Image {
    image_map(picture, |x, y, p| {
        let seen = shown.at(x, y);
        let beyond = (0..3).any(|c| seen[c] < -k.tolerance || seen[c] > 1.0 + k.tolerance);
        if !beyond {
            return p;
        }
        let at = picture.ci_coord(x, y);
        if (at[0] + at[1] + k.phase) % k.period < k.duty * k.period { k.tint } else { p }
    })
}

const SCREEN_GAMUT_ZEBRA: Shader = shader!("screen_gamut_zebra");

impl Gpu {
    /// Per pixel on one extent, the stripes laid on Core Image coordinates from `picture.origin`.
    pub fn screen_gamut_zebra(&self, picture: &Image, shown: &Image, k: &ScreenGamutZebra) -> Image {
        let rows = |band: &Image| [vec![[band.origin[0], band.origin[1], 0.0, 0.0]], k.rows()].concat();
        self.per_pixel(SCREEN_GAMUT_ZEBRA, rows, &[picture, shown], &[], 0)
    }
}
