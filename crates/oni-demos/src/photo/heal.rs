//! De-dust on the demo's frame: the framed frame's size, and a stroke's patch searched and made as
//! the app's own heal does, written where this frame's engine alone reads it.

use std::path::Path;

use oni_pipeline::Request;
use oni_settings::PipelineSettings;
use oni_settings::frame::Correction;
use oni_app::render::{self, Reading};

use super::{Photo, Rgba};

impl Photo {
    fn full_source(&mut self, s: &PipelineSettings) -> Result<oni_graph::Source, String> {
        if self.full.is_none() {
            let decoded = render::decode_full(&self.path, Reading::of(s))?;
            self.full = Some(self.engine.full(decoded)?);
        }
        Ok(self.full.clone().expect("decoded"))
    }

    /// The full-resolution render's size, framed, in pixels: what a stroke's fractions are of.
    pub fn framed_size(&mut self, s: &PipelineSettings) -> Result<(u32, u32), String> {
        let source = self.full_source(s)?;
        let e = self.engine.unproofed(&source, s, &Request::default())?.extent();
        Ok((e.w, e.h))
    }

    /// `c`'s patch, as the app's heal makes it, kept in `caches` for this frame's renders; where it
    /// was copied from, in fractions of the framed frame, y down.
    pub fn heal(&mut self, s: &PipelineSettings, c: &Correction, caches: &Path) -> Result<(f64, f64), String> {
        let source = self.full_source(s)?;
        self.engine.caches = Some(caches.to_path_buf());
        let (region, size) = self.engine.search_region(&source, s, c)?;
        let patch = oni_app::heal::generate(&region, c, size.0, size.1);
        oni_app::patches::write(caches, c.id, patch.width as u32, patch.height as u32, &patch.pixels)?;
        self.engine.forget_patch(c.id);
        Ok(patch.offset)
    }
}

/// `picture` scaled `zoom` times to `width` × `height` from its top left, each pixel mixed from its
/// four neighbours, as a canvas zoomed past one to one shows it.
pub fn magnified(picture: &Rgba, zoom: u32, width: u32, height: u32) -> Rgba {
    let (w, h) = (picture.width as f32, picture.height as f32);
    let at = |x: usize, y: usize, c: usize| f32::from(picture.pixels[(y * picture.width as usize + x) * 4 + c]);
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        let fy = ((y as f32 + 0.5) / zoom as f32 - 0.5).clamp(0.0, h - 1.0);
        let (y0, ty) = (fy.floor() as usize, fy.fract());
        let y1 = (y0 + 1).min(picture.height as usize - 1);
        for x in 0..width {
            let fx = ((x as f32 + 0.5) / zoom as f32 - 0.5).clamp(0.0, w - 1.0);
            let (x0, tx) = (fx.floor() as usize, fx.fract());
            let x1 = (x0 + 1).min(picture.width as usize - 1);
            for c in 0..4 {
                let top = at(x0, y0, c) * (1.0 - tx) + at(x1, y0, c) * tx;
                let bottom = at(x0, y1, c) * (1.0 - tx) + at(x1, y1, c) * tx;
                pixels.push((top * (1.0 - ty) + bottom * ty).round() as u8);
            }
        }
    }
    Rgba { width, height, pixels }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_magnified_picture_keeps_each_pixel_at_its_centre_and_mixes_between() {
        let picture = Rgba { width: 2, height: 1, pixels: vec![0, 0, 0, 255, 200, 200, 200, 255] };
        let big = magnified(&picture, 4, 8, 4);
        let red = |x: usize| big.pixels[x * 4];
        assert_eq!((red(0), red(1), red(6), red(7)), (0, 0, 200, 200), "the ends hold their pixel");
        assert_eq!((red(3), red(4)), (75, 125), "between them, mixed by distance");
        // Twin: a nearest-pixel scale would jump at once from black to the grey.
        assert_ne!((red(3), red(4)), (0, 200));
    }
}
