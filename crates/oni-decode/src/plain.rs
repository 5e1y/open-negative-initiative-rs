//! A file neither LibRaw nor the linear container reads, as ImageIO hands it to Core Image: its
//! encoded values on [0, 1], upright, its embedded profile, and whether it holds one grey channel.

use image::{DynamicImage, ImageDecoder, ImageReader, imageops::FilterType};
use std::path::Path;

pub struct Encoded {
    pub width: usize,
    pub height: usize,
    /// The upright file's own size, before any reduction.
    pub full: (usize, usize),
    /// Row-major RGBA, the file's own code values, alpha 1.
    pub pixels: Vec<[f32; 4]>,
    /// The ICC profile the file embeds; `None` names nothing, read as the chosen encoding or sRGB.
    pub icc: Option<Vec<u8>>,
    pub grey: bool,
}

/// `plainLinearReduced`'s thumbnail when `longest_side` is given: never past the file's own size,
/// sampled on its code values as ImageIO reduces before Core Image linearises.
pub fn read(path: &Path, longest_side: Option<f64>) -> Option<Encoded> {
    let mut decoder = ImageReader::open(path).ok()?.with_guessed_format().ok()?.into_decoder().ok()?;
    let icc = decoder.icc_profile().ok().flatten();
    let orientation = decoder.orientation().ok();
    let grey = !decoder.color_type().has_color();
    let mut image = DynamicImage::from_decoder(decoder).ok()?;
    if let Some(o) = orientation {
        image.apply_orientation(o);
    }
    // In floats before any resampling: an 8-bit reduction would quantise what Core Image reads in floats.
    let full = (image.width() as usize, image.height() as usize);
    let mut image = DynamicImage::ImageRgba32F(image.to_rgba32f());
    if let Some(side) = longest_side {
        let (w, h) = (image.width() as f64, image.height() as f64);
        let scale = side / w.max(h);
        if scale < 1.0 {
            let (nw, nh) = (((w * scale).round() as u32).max(1), ((h * scale).round() as u32).max(1));
            image = image.resize_exact(nw, nh, FilterType::Triangle);
        }
    }
    let rgba = image.into_rgba32f();
    let (width, height) = (rgba.width() as usize, rgba.height() as usize);
    let pixels = rgba.pixels().map(|p| [p[0], p[1], p[2], 1.0]).collect();
    Some(Encoded { width, height, full, pixels, icc, grey })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_png_reads_its_codes_and_a_reduction_keeps_the_side_asked() {
        let dir = std::env::temp_dir().join(format!("oni-plain-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ramp.png");
        let image = image::RgbImage::from_fn(40, 20, |x, _| image::Rgb([(x * 6) as u8, 128, 255 - (x * 6) as u8]));
        image.save(&path).unwrap();
        let full = read(&path, None).unwrap();
        assert_eq!((full.width, full.height), (40, 20));
        assert!((full.pixels[0][1] - 128.0 / 255.0).abs() < 1e-6 && full.pixels[0][2] == 1.0, "{:?}", full.pixels[0]);
        assert!(!full.grey && full.icc.is_none());
        let small = read(&path, Some(10.0)).unwrap();
        assert_eq!((small.width, small.height, small.full), (10, 5, (40, 20)));
        // Twin: a side past the file's own size leaves it whole, never enlarged.
        assert_eq!(read(&path, Some(400.0)).unwrap().width, 40);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
