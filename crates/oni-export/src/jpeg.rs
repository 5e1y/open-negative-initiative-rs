//! The JPEG writer: 8-bit RGB at a quality, its ICC profile and the stated metadata in Exif.

use jpeg_encoder::{ColorType, Encoder};
use oni_colour::Quantised;

use crate::ifd::{Directory, Value};
use crate::metadata::tag;

/// The encoder's 1–100 scale from the export's 0–1 quality.
pub fn quality(q: f64) -> u8 {
    (q * 100.0).round().clamp(1.0, 100.0) as u8
}

/// The Exif segment's TIFF body, big-endian: the primary directory, then the Exif one.
pub fn exif_body(primary: &Directory, exif: &Directory) -> Option<Vec<u8>> {
    if primary.is_empty() && exif.is_empty() {
        return None;
    }
    let mut ifd = primary.clone();
    if !exif.is_empty() {
        ifd.set(tag::EXIF_IFD, Value::Long(vec![0]));
        let exif_at = 8 + ifd.len();
        ifd.set(tag::EXIF_IFD, Value::Long(vec![exif_at as u32]));
    }
    let mut out = b"MM\0*".to_vec();
    out.extend(8u32.to_be_bytes());
    out.extend(ifd.laid(8, 0));
    if !exif.is_empty() {
        let at = out.len() as u32;
        out.extend(exif.laid(at, 0));
    }
    Some(out)
}

/// An RGBA frame (alpha dropped) at `quality` 0–1, its profile embedded.
pub fn encoded(
    width: usize,
    height: usize,
    pixels: &Quantised,
    quality_: f64,
    icc: &[u8],
    primary: &Directory,
    exif: &Directory,
) -> Option<Vec<u8>> {
    let w = u16::try_from(width).ok()?;
    let h = u16::try_from(height).ok()?;
    let eight: std::borrow::Cow<[u8]> = match pixels {
        Quantised::Eight(v) => v.into(),
        Quantised::Sixteen(v) => v.iter().map(|s| ((*s as u32 + 128) / 257) as u8).collect::<Vec<_>>().into(),
    };
    if width == 0 || height == 0 || eight.len() != width * height * 4 {
        return None;
    }
    let mut out = Vec::new();
    let mut encoder = Encoder::new(&mut out, quality(quality_));
    if let Some(body) = exif_body(primary, exif) {
        encoder.add_exif_metadata(&body).ok()?;
    }
    encoder.add_icc_profile(icc).ok()?;
    encoder.encode(&eight, w, h, ColorType::Rgba).ok()?;
    Some(out)
}

/// A grey frame, every sample `value`: the gain map's own image.
pub fn grey(width: usize, height: usize, value: u8, quality_: f64) -> Option<Vec<u8>> {
    let pixels = vec![value; width * height];
    let mut out = Vec::new();
    let encoder = Encoder::new(&mut out, quality(quality_));
    encoder.encode(&pixels, u16::try_from(width).ok()?, u16::try_from(height).ok()?, ColorType::Luma).ok()?;
    Some(out)
}
