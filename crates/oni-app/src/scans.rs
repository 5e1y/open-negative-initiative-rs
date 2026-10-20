//! `Thumbnails.scan`: a frame not yet converted shows the scan as the file holds it, a RAW's own
//! preview turned upright, never the pipeline's render: about 10 ms against a decode's 400.

use crate::display::{Params, Screen};
use crate::render;
use crate::thumbnails::{SIDE, jpeg};
use std::path::Path;

/// `Thumbnails.isConverted`: the settings differ from the frame taken back to unconverted.
pub fn is_converted(settings: &oni_settings::PipelineSettings) -> bool {
    *settings != oni_core::parameters::unconverted(settings)
}

/// The tile's JPEG of the file as it holds itself; `None` for a RAW carrying no preview it reads.
pub fn scan(path: &Path) -> Option<Vec<u8>> {
    if oni_decode::decodes_via_mosaic(path) {
        let e = oni_decode::embedded::embedded(path)?;
        let (rgb, w, h) = match e.preview {
            oni_decode::embedded::Preview::Jpeg(bytes) => rgb_of(&bytes)?,
            oni_decode::embedded::Preview::Rgb { width, height, pixels } => (pixels, width, height),
        };
        let (rgb, w, h) = oni_decode::embedded::turned(&rgb, (w, h), e.flip);
        let (rgba, w, h) = reduced(&rgb, w, h, SIDE);
        return jpeg(&rgba, w, h);
    }
    let d = render::decode_alone(path, Some(SIDE as f64)).ok()?;
    let l = d.linear;
    let rgba = render::encoded(&l, &Params::new(Screen::Srgb, l.width as u32, l.height as u32, 0), 1);
    jpeg(&rgba, l.width, l.height)
}

/// A JPEG's pixels as RGB rows, grey widened, decoded no larger than the tile needs: a camera's
/// full-size preview read at an eighth costs a tenth of its whole decode.
fn rgb_of(bytes: &[u8]) -> Option<(Vec<u8>, usize, usize)> {
    let mut decoder = jpeg_decoder::Decoder::new(bytes);
    let side = u16::try_from(SIDE).ok()?;
    decoder.scale(side, side).ok()?;
    let pixels = decoder.decode().ok()?;
    let info = decoder.info()?;
    let (w, h) = (usize::from(info.width), usize::from(info.height));
    let rgb = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => pixels,
        jpeg_decoder::PixelFormat::L8 => pixels.iter().flat_map(|&v| [v, v, v]).collect(),
        _ => return None,
    };
    Some((rgb, w, h))
}

/// RGB rows averaged down to `side` on the long side, opaque; a smaller image kept at its size.
fn reduced(rgb: &[u8], w: usize, h: usize, side: usize) -> (Vec<u8>, usize, usize) {
    let scale = (side as f64 / w.max(h).max(1) as f64).min(1.0);
    let (ow, oh) = (((w as f64 * scale).round() as usize).max(1), ((h as f64 * scale).round() as usize).max(1));
    let mut out = Vec::with_capacity(ow * oh * 4);
    for oy in 0..oh {
        let (y0, y1) = (oy * h / oh, ((oy + 1) * h / oh).max(oy * h / oh + 1));
        for ox in 0..ow {
            let (x0, x1) = (ox * w / ow, ((ox + 1) * w / ow).max(ox * w / ow + 1));
            let mut sum = [0u32; 3];
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = (y * w + x) * 3;
                    for c in 0..3 {
                        sum[c] += u32::from(rgb[p + c]);
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            out.extend(sum.map(|v| ((v + n / 2) / n) as u8));
            out.push(255);
        }
    }
    (out, ow, oh)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4 by 2 image of two flat halves reduced to 2 by 1: each output pixel is its half's mean.
    #[test]
    fn a_preview_is_averaged_down_to_the_tile_side() {
        let rgb: Vec<u8> = (0..8).flat_map(|i| if i % 4 < 2 { [10, 20, 30] } else { [200, 100, 50] }).collect();
        let (out, w, h) = reduced(&rgb, 4, 2, 2);
        assert_eq!((w, h), (2, 1));
        assert_eq!(out, [10, 20, 30, 255, 200, 100, 50, 255]);
        // Twin: an image already under the side keeps its size.
        assert_eq!(reduced(&rgb, 4, 2, 320).1, 4);
    }

    /// Fresh settings read as unconverted; a frame inverted as a negative reads as converted.
    #[test]
    fn a_frame_is_converted_once_its_settings_leave_the_unconverted_rest() {
        let rest = oni_core::parameters::unconverted(&oni_settings::PipelineSettings::default());
        assert!(!is_converted(&rest));
        let mut moved = rest.clone();
        moved.print.exposure += 0.5;
        assert!(is_converted(&moved), "twin: an edit converts it");
    }
}
