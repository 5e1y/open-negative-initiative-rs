//! The TIFF writer: RGBA in 8 or 16 bits, ZIP (Deflate) or none, its ICC profile and the stated
//! metadata, and no other tag.

use std::borrow::Cow;
use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use oni_colour::Quantised;

use crate::ifd::{Directory, Value};
use crate::metadata::tag;

const PIXEL_X_DIMENSION: u16 = 40962;
const PIXEL_Y_DIMENSION: u16 = 40963;

pub mod tags {
    pub const IMAGE_WIDTH: u16 = 256;
    pub const IMAGE_LENGTH: u16 = 257;
    pub const BITS_PER_SAMPLE: u16 = 258;
    pub const COMPRESSION: u16 = 259;
    pub const PHOTOMETRIC: u16 = 262;
    pub const STRIP_OFFSETS: u16 = 273;
    pub const SAMPLES_PER_PIXEL: u16 = 277;
    pub const ROWS_PER_STRIP: u16 = 278;
    pub const STRIP_BYTE_COUNTS: u16 = 279;
    pub const PLANAR_CONFIGURATION: u16 = 284;
    pub const FILL_ORDER: u16 = 266;
    pub const ORIENTATION: u16 = 274;
    pub const RESOLUTION_UNIT: u16 = 296;
    pub const SAMPLE_FORMAT: u16 = 339;
    pub const EXTRA_SAMPLES: u16 = 338;
    pub const ICC_PROFILE: u16 = 34675;
}

/// Deflate, in TIFF's numbering; LZW (5) is never written.
pub const DEFLATE: u16 = 8;
pub const NONE: u16 = 1;

/// Uncompressed bytes per strip: large enough for Deflate's window, small enough to share the
/// strips among every core.
const STRIP_BYTES: usize = 1 << 20;

/// A TIFF of `width` × `height` RGBA samples, rows top first, alpha associated.
pub fn encoded(
    width: usize,
    height: usize,
    pixels: &Quantised,
    compress: bool,
    icc: &[u8],
    primary: &Directory,
    exif: &Directory,
) -> Option<Vec<u8>> {
    // Each strip's bytes made where it is compressed, so the frame is never held a second time.
    match pixels {
        Quantised::Eight(v) => laid_out(width, height, (8, UNSIGNED), (v.len(), &|a, b| Cow::Borrowed(&v[a..b])), compress, icc, primary, exif),
        Quantised::Sixteen(v) => {
            let big_endian = |a: usize, b: usize| Cow::Owned(v[a / 2..b / 2].iter().flat_map(|s| s.to_be_bytes()).collect());
            laid_out(width, height, (16, UNSIGNED), (v.len() * 2, &big_endian), compress, icc, primary, exif)
        }
    }
}

/// The bytes from one offset to another of a frame laid out row after row.
type Bytes<'a> = (usize, &'a (dyn Fn(usize, usize) -> Cow<'a, [u8]> + Sync));

/// A TIFF of half-float RGBA samples, rows top first, as Core Image writes `RGBAh`; no metadata.
pub fn encoded_half(width: usize, height: usize, pixels: &[[f32; 4]], icc: &[u8]) -> Option<Vec<u8>> {
    let bytes: Vec<u8> = pixels.iter().flatten().flat_map(|v| half::f16::from_f32(*v).to_be_bytes()).collect();
    laid_out(width, height, (16, FLOAT), (bytes.len(), &|a, b| Cow::Borrowed(&bytes[a..b])), false, icc, &Directory::default(), &Directory::default())
}

/// TIFF's SampleFormat: unsigned integers, IEEE floats.
const UNSIGNED: u16 = 1;
const FLOAT: u16 = 3;

#[allow(clippy::too_many_arguments)]
fn laid_out(
    width: usize,
    height: usize,
    (bits, format): (u16, u16),
    (length, bytes): Bytes<'_>,
    compress: bool,
    icc: &[u8],
    primary: &Directory,
    exif: &Directory,
) -> Option<Vec<u8>> {
    let row_bytes = width * 4 * bits as usize / 8;
    if width == 0 || height == 0 || length != row_bytes * height {
        return None;
    }
    let rows_per_strip = (STRIP_BYTES / row_bytes).clamp(1, height);
    let strips: Vec<(usize, usize)> = (0..length).step_by(rows_per_strip * row_bytes).map(|a| (a, (a + rows_per_strip * row_bytes).min(length))).collect();
    let laid: Vec<Cow<[u8]>> = if compress { deflated(&strips, bytes).into_iter().map(Cow::Owned).collect() } else { strips.iter().map(|&(a, b)| bytes(a, b)).collect() };

    let mut offsets = Vec::with_capacity(laid.len());
    let mut at = 8usize;
    for strip in &laid {
        offsets.push(u32::try_from(at).ok()?);
        at += strip.len();
    }
    at += at % 2;

    let mut ifd = primary.clone();
    ifd.set(tags::IMAGE_WIDTH, Value::Long(vec![width as u32]));
    ifd.set(tags::IMAGE_LENGTH, Value::Long(vec![height as u32]));
    ifd.set(tags::BITS_PER_SAMPLE, Value::Short(vec![bits; 4]));
    ifd.set(tags::COMPRESSION, Value::Short(vec![if compress { DEFLATE } else { NONE }]));
    ifd.set(tags::PHOTOMETRIC, Value::Short(vec![2]));
    ifd.set(tags::STRIP_OFFSETS, Value::Long(offsets));
    ifd.set(tags::SAMPLES_PER_PIXEL, Value::Short(vec![4]));
    ifd.set(tags::ROWS_PER_STRIP, Value::Long(vec![rows_per_strip as u32]));
    ifd.set(tags::STRIP_BYTE_COUNTS, Value::Long(laid.iter().map(|s| s.len() as u32).collect()));
    ifd.set(tags::PLANAR_CONFIGURATION, Value::Short(vec![1]));
    // What ImageIO states of every TIFF it writes: bits high first, upright, inches.
    ifd.set(tags::FILL_ORDER, Value::Short(vec![1]));
    ifd.set(tags::ORIENTATION, Value::Short(vec![1]));
    ifd.set(tags::RESOLUTION_UNIT, Value::Short(vec![2]));
    ifd.set(tags::SAMPLE_FORMAT, Value::Short(vec![format; 4]));
    // Associated alpha: the colours are premultiplied, as ImageIO writes Core Image's render.
    ifd.set(tags::EXTRA_SAMPLES, Value::Short(vec![1]));
    ifd.set(tags::ICC_PROFILE, Value::Undefined(icc.to_vec()));
    // ImageIO states the picture's size in every Exif directory it writes.
    let mut exif = exif.clone();
    if !exif.is_empty() {
        exif.set(PIXEL_X_DIMENSION, Value::Long(vec![width as u32]));
        exif.set(PIXEL_Y_DIMENSION, Value::Long(vec![height as u32]));
    }
    let exif = &exif;
    let exif_at = at + placeholder_len(&ifd, exif);
    if !exif.is_empty() {
        ifd.set(tag::EXIF_IFD, Value::Long(vec![u32::try_from(exif_at).ok()?]));
    }
    let ifd_at = u32::try_from(at).ok()?;
    let ifd_bytes = ifd.laid(ifd_at, 0);
    // Every offset is a LONG: past 4 GB the file cannot be laid.
    u32::try_from(at + ifd_bytes.len() + exif.len()).ok()?;

    let mut out = Vec::with_capacity(at + ifd_bytes.len() + exif.len());
    out.extend(b"MM\0*");
    out.extend(ifd_at.to_be_bytes());
    for strip in &laid {
        out.extend_from_slice(strip);
    }
    out.resize(at, 0);
    out.extend(&ifd_bytes);
    if !exif.is_empty() {
        out.extend(exif.laid(exif_at as u32, 0));
    }
    Some(out)
}

/// The primary directory's length once the Exif pointer is in it, which a LONG keeps inline.
fn placeholder_len(ifd: &Directory, exif: &Directory) -> usize {
    let mut with = ifd.clone();
    if !exif.is_empty() {
        with.set(tag::EXIF_IFD, Value::Long(vec![0]));
    }
    with.len()
}

/// Each strip as its own zlib stream, the strips shared among every core.
fn deflated<'a>(strips: &[(usize, usize)], bytes: &(dyn Fn(usize, usize) -> Cow<'a, [u8]> + Sync)) -> Vec<Vec<u8>> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(strips.len().max(1));
    let per = strips.len().div_ceil(threads.max(1)).max(1);
    let mut out: Vec<Vec<u8>> = vec![Vec::new(); strips.len()];
    std::thread::scope(|scope| {
        for (group, slots) in strips.chunks(per).zip(out.chunks_mut(per)) {
            scope.spawn(move || {
                for (&(a, b), slot) in group.iter().zip(slots) {
                    let strip = bytes(a, b);
                    let mut z = ZlibEncoder::new(Vec::with_capacity(strip.len() / 2), Compression::default());
                    z.write_all(&strip).expect("writing into memory");
                    *slot = z.finish().expect("writing into memory");
                }
            });
        }
    });
    out
}
