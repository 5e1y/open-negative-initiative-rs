//! What the export's file checks share: a frame, the settings of a format and space, and readers.

use oni_colour::Space;
use oni_export::{Format, Image, Settings};
use std::io::Cursor;

/// A smooth working-space frame: two ramps and a diagonal, every value inside every space.
pub fn frame(w: usize, h: usize) -> Image {
    let mut pixels = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            pixels.extend([0.02 + 0.5 * u, 0.03 + 0.4 * v, 0.05 + 0.2 * (u + v), 1.0]);
        }
    }
    Image { width: w, height: h, pixels }
}

pub fn settings(format: Format, space: Space) -> Settings {
    let mut s = Settings::default();
    s.set_format(format);
    s.set_space(space);
    s
}

pub fn read_tiff(file: &[u8]) -> tiff::decoder::Decoder<Cursor<&[u8]>> {
    tiff::decoder::Decoder::new(Cursor::new(file)).expect("a TIFF reader opens the file")
}

pub fn jpeg_profile(file: &[u8]) -> Option<Vec<u8>> {
    let mut d = jpeg_decoder::Decoder::new(Cursor::new(file));
    d.decode().ok()?;
    d.icc_profile()
}
