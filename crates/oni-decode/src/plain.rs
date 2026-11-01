//! A file neither LibRaw nor the linear container reads, as ImageIO hands it to Core Image: its
//! encoded values on [0, 1], upright, its embedded profile, and whether it holds one grey channel.

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits, codecs::tiff::TiffDecoder, imageops::FilterType};
use std::path::Path;

pub struct Encoded {
    pub width: usize,
    pub height: usize,
    /// The upright file's own size, before any reduction.
    pub full: (usize, usize),
    /// Row-major RGBA, the file's own code values and its alpha, straight; a file with none reads 1.
    pub pixels: Vec<[f32; 4]>,
    /// The ICC profile the file embeds; `None` names nothing, read as the chosen encoding or sRGB.
    pub icc: Option<Vec<u8>>,
    pub grey: bool,
}

/// `plainLinearReduced`'s thumbnail when `longest_side` is given: never past the file's own size,
/// sampled on its code values as ImageIO reduces before Core Image linearises; Catmull-Rom is the
/// filter nearest ImageIO's, 0.002 to 0.005 off on three scans, where a triangle is 0.002 to 0.006.
pub fn read(path: &Path, longest_side: Option<f64>) -> Option<Encoded> {
    read_within(path, longest_side, ALLOWED)
}

/// What one read may allocate: the reader's own 512 MB refuse a 45 Mpx 16-bit TIFF in one strip,
/// where ImageIO reads it; 4 GB is the export's budget.
const ALLOWED: u64 = 4 << 30;

fn read_within(path: &Path, longest_side: Option<f64>, allowed: u64) -> Option<Encoded> {
    let mut reader = ImageReader::open(path).ok()?.with_guessed_format().ok()?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(allowed);
    reader.limits(limits);
    let tiff = reader.format() == Some(ImageFormat::Tiff);
    let mut decoder = reader.into_decoder().ok()?;
    let mut icc = decoder.icc_profile().ok().flatten();
    if icc.is_none() && tiff {
        icc = tiff_profile(path);
    }
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
            image = image.resize_exact(nw, nh, FilterType::CatmullRom);
        }
    }
    let rgba = image.into_rgba32f();
    let (width, height) = (rgba.width() as usize, rgba.height() as usize);
    let pixels = rgba.pixels().map(|p| [p[0], p[1], p[2], p[3]]).collect();
    Some(Encoded { width, height, full, pixels, icc, grey })
}

/// A file no reader of ONI takes, named by its extension in capitals (`HEIC`); `None` when one takes
/// it, or when the file cannot be opened, which its absence tells instead.
pub fn unsupported(path: &Path) -> Option<String> {
    let reader = ImageReader::open(path).ok()?.with_guessed_format().ok()?;
    if reader.format().is_some_and(|f| f.reading_enabled()) {
        return None;
    }
    let named = path.extension().map(|e| e.to_string_lossy().to_uppercase()).filter(|e| !e.is_empty());
    Some(named.or_else(|| reader.format().map(|f| format!("{f:?}").to_uppercase())).unwrap_or_default())
}

/// The reader's limits cap a TIFF tag at the pixels' own byte count, so a profile larger than a small
/// image's pixels (598 bytes over a 4 × 1 frame) is dropped; a decoder with the default limits reads it.
fn tiff_profile(path: &Path) -> Option<Vec<u8>> {
    let file = std::io::BufReader::new(std::fs::File::open(path).ok()?);
    TiffDecoder::new(file).ok()?.icc_profile().ok().flatten()
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

    /// An uncompressed 16-bit RGB TIFF in one strip, as an editor may write a large scan.
    fn one_strip(path: &Path, w: u32, h: u32) {
        let data_len = w * h * 6;
        let (bits_at, ifd_at) = (8 + data_len, 8 + data_len + 6);
        let mut out = b"II*\0".to_vec();
        out.extend(ifd_at.to_le_bytes());
        out.extend((0..w * h * 3).flat_map(|i| ((i % 60_000) as u16).to_le_bytes()));
        out.extend([16, 0, 16, 0, 16, 0]);
        let entries: [(u16, u16, u32, u32); 8] = [(256, 4, 1, w), (257, 4, 1, h), (258, 3, 3, bits_at), (259, 3, 1, 1), (262, 3, 1, 2), (273, 4, 1, 8), (277, 3, 1, 3), (279, 4, 1, data_len)];
        out.extend((entries.len() as u16).to_le_bytes());
        for (tag, kind, count, value) in entries {
            out.extend(tag.to_le_bytes());
            out.extend(kind.to_le_bytes());
            out.extend(count.to_le_bytes());
            out.extend(if kind == 3 && count == 1 { u32::from(value as u16) } else { value }.to_le_bytes());
        }
        out.extend(0u32.to_le_bytes());
        std::fs::write(path, out).unwrap();
    }

    #[test]
    fn a_16_bit_tiff_in_one_strip_reads_past_the_readers_own_limit() {
        let dir = std::env::temp_dir().join(format!("oni-plain-strip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Scan.tif");
        one_strip(&path, 600, 400);
        let read = read(&path, Some(300.0)).expect("the scan reads");
        assert_eq!((read.width, read.height, read.full), (300, 200, (600, 400)));
        // Twin: the same file under a limit as near its size as 512 MB is to a 45 Mpx scan is refused.
        assert!(read_within(&path, Some(300.0), 2_000_000).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_heic_is_named_unsupported_and_a_broken_tiff_is_left_to_its_decode() {
        let dir = std::env::temp_dir().join(format!("oni-plain-formats-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let heic = dir.join("IMG_0001.HEIC");
        std::fs::write(&heic, b"\0\0\0\x18ftypheic\0\0\0\0mif1heic").unwrap();
        assert_eq!(unsupported(&heic).as_deref(), Some("HEIC"));
        // Twin: a TIFF whose bytes are broken is a format ONI reads, so its failure is the decode's.
        let bad = dir.join("Bad.tif");
        std::fs::write(&bad, b"II*\0not a picture").unwrap();
        assert_eq!(unsupported(&bad), None);
        assert!(read(&bad, None).is_none(), "and it does fail to decode");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
