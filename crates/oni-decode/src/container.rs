//! `ContainerRead`: the TIFF container of a scan already demosaiced, read directly. It stays
//! container plumbing, strips and a 1-D table, never a RAW decoder.

use crate::Linear;
use std::cell::RefCell;
use std::fs::File;
use std::path::Path;
use window::{WINDOW, Window};

mod window;

/// What the bytes say the file is, read from the header, never from the extension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Mosaic,
    LinearRgb,
    /// A TIFF container this reader can see but would decode wrong.
    Unsupported(String),
}

#[derive(Clone, Debug, Default)]
struct Directory {
    width: usize,
    height: usize,
    bits_per_sample: usize,
    samples_per_pixel: usize,
    compression: i64,
    photometric: i64,
    planar_config: i64,
    sample_format: i64,
    subfile_type: i64,
    cfa_pattern: bool,
    strip_offsets: Vec<u64>,
    strip_byte_counts: Vec<usize>,
    rows_per_strip: usize,
    tile_offsets: Vec<u64>,
    tile_width: usize,
    tile_length: usize,
    linearization: Vec<u16>,
    black_level: Vec<f64>,
    white_level: Vec<f64>,
    sub_ifds: Vec<u64>,
    color_matrix: Vec<f64>,
    illuminant: i64,
}

/// The file's bytes, in memory for a decode, or read where asked for a header.
enum Bytes<'a> {
    Slice(&'a [u8]),
    File(RefCell<Window>),
}

struct Reader<'a> {
    bytes: Bytes<'a>,
    little: bool,
}

impl Reader<'_> {
    fn take<const N: usize>(&self, i: usize) -> Option<[u8; N]> {
        match &self.bytes {
            Bytes::Slice(b) => b.get(i..i.checked_add(N)?)?.try_into().ok(),
            Bytes::File(w) => w.borrow_mut().take(i as u64),
        }
    }

    fn u8(&self, i: usize) -> Option<u8> {
        self.take::<1>(i).map(|b| b[0])
    }

    fn u16(&self, i: usize) -> Option<u16> {
        let b = self.take::<2>(i)?;
        Some(if self.little { u16::from_le_bytes(b) } else { u16::from_be_bytes(b) })
    }

    fn u32(&self, i: usize) -> Option<u32> {
        let b = self.take::<4>(i)?;
        Some(if self.little { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) })
    }

    /// RATIONAL, or SRATIONAL read signed: a colour matrix's entries go negative.
    fn rational(&self, i: usize, signed: bool) -> Option<f64> {
        let (n, d) = (self.u32(i)?, self.u32(i + 4)?);
        if d == 0 {
            return None;
        }
        Some(if signed { f64::from(n as i32) / f64::from(d as i32) } else { f64::from(n) / f64::from(d) })
    }
}

fn type_size(t: u16) -> usize {
    match t {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 => 4,
        5 | 10 | 12 => 8,
        _ => 1,
    }
}

/// An offset read from the bytes, or `None` when it cannot be one.
fn offset(v: f64) -> Option<u64> {
    (v.is_finite() && v >= 0.0 && v <= f64::from(u32::MAX)).then_some(v as u64)
}

fn read_directory(r: &Reader, at: usize) -> Option<Directory> {
    let count = r.u16(at)? as usize;
    if count == 0 || count >= 4096 {
        return None;
    }
    let mut d = Directory { samples_per_pixel: 1, compression: 1, photometric: -1, planar_config: 1, sample_format: 1, ..Default::default() };
    let values = |entry: usize, t: u16, n: usize| -> Vec<f64> {
        let size = type_size(t) * n;
        let base = if size <= 4 { entry + 8 } else { r.u32(entry + 8).unwrap_or(0) as usize };
        if base == 0 && size > 4 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(n.min(1 << 16));
        for k in 0..n {
            let v = match t {
                1 | 2 | 6 | 7 => r.u8(base + k).map(f64::from),
                3 | 8 => r.u16(base + k * 2).map(f64::from),
                4 | 9 => r.u32(base + k * 4).map(f64::from),
                5 => r.rational(base + k * 8, false),
                10 => r.rational(base + k * 8, true),
                _ => return Vec::new(),
            };
            if let Some(v) = v {
                out.push(v);
            }
        }
        out
    };
    for i in 0..count {
        let entry = at + 2 + i * 12;
        let (Some(tag), Some(t), Some(n)) = (r.u16(entry), r.u16(entry + 2), r.u32(entry + 4)) else { continue };
        let n = n as usize;
        if n == 0 || n >= 1 << 24 {
            continue;
        }
        let v = || values(entry, t, n);
        let first = |d: f64| v().first().copied().unwrap_or(d);
        match tag {
            254 => d.subfile_type = first(0.0) as i64,
            256 => d.width = first(0.0) as usize,
            257 => d.height = first(0.0) as usize,
            258 => d.bits_per_sample = first(0.0) as usize,
            259 => d.compression = first(1.0) as i64,
            262 => d.photometric = first(-1.0) as i64,
            273 => d.strip_offsets = v().into_iter().filter_map(offset).collect(),
            277 => d.samples_per_pixel = first(1.0) as usize,
            278 => d.rows_per_strip = first(0.0) as usize,
            279 => d.strip_byte_counts = v().into_iter().map(|x| x as usize).collect(),
            284 => d.planar_config = first(1.0) as i64,
            322 => d.tile_width = first(0.0) as usize,
            323 => d.tile_length = first(0.0) as usize,
            324 => d.tile_offsets = v().into_iter().filter_map(offset).collect(),
            330 => d.sub_ifds = v().into_iter().filter_map(offset).collect(),
            339 => d.sample_format = first(1.0) as i64,
            33422 => d.cfa_pattern = true,
            50712 => d.linearization = v().into_iter().map(|x| x.clamp(0.0, 65535.0) as u16).collect(),
            50714 => d.black_level = v(),
            50717 => d.white_level = v(),
            50721 => d.color_matrix = v(),
            50778 => d.illuminant = first(0.0) as i64,
            _ => {}
        }
    }
    plausible(&d).then_some(d)
}

/// Each side under 2^20 and the product under 512 Mpx, before anything is allocated.
fn plausible(d: &Directory) -> bool {
    let side = 1 << 20;
    d.width > 0
        && d.height > 0
        && d.width <= side
        && d.height <= side
        && d.width * d.height <= 512 << 20
        && (1..=16).contains(&d.samples_per_pixel)
        && (1..=32).contains(&d.bits_per_sample)
}

/// IFD 0 of a DNG is usually a thumbnail: the largest full-resolution SubIFD holds the image.
fn main_directory(r: &Reader) -> Option<Directory> {
    let root = read_directory(r, r.u32(4)? as usize)?;
    let mut best = (root.subfile_type == 0).then(|| root.clone());
    for &sub in &root.sub_ifds {
        let Some(d) = read_directory(r, sub as usize) else { continue };
        let area = |b: &Option<Directory>| b.as_ref().map_or(0, |b| b.width * b.height);
        if d.subfile_type == 0 && d.width * d.height > area(&best) {
            best = Some(d);
        }
    }
    Some(best.unwrap_or(root))
}

/// A classic TIFF's reader; BigTIFF (43) lays its offsets out otherwise and is not one.
fn reader(bytes: Bytes<'_>) -> Option<Reader<'_>> {
    let mut r = Reader { bytes, little: true };
    r.little = match r.take::<2>(0)? {
        [b'I', b'I'] => true,
        [b'M', b'M'] => false,
        _ => return None,
    };
    (r.u16(2)? == 42).then_some(r)
}

fn of_file(path: &Path) -> Option<Reader<'static>> {
    let file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    reader(Bytes::File(RefCell::new(Window::new(file, len, WINDOW))))
}

fn classify(r: &Reader) -> Option<Kind> {
    let d = main_directory(r)?;
    if d.cfa_pattern || d.photometric == 32803 {
        return Some(Kind::Mosaic);
    }
    let refuse = |why: String| Some(Kind::Unsupported(why));
    if d.samples_per_pixel < 3 {
        return refuse(format!("{} channel per pixel", d.samples_per_pixel));
    }
    if d.compression != 1 {
        return refuse(format!("compression {}", d.compression));
    }
    if d.planar_config != 1 {
        return refuse("channels in separate planes".into());
    }
    if ![8, 16, 32].contains(&d.bits_per_sample) {
        return refuse(format!("{} bits per sample", d.bits_per_sample));
    }
    if d.strip_offsets.is_empty() && d.tile_offsets.is_empty() {
        return refuse("neither strips nor tiles".into());
    }
    // 34892 is LinearRaw, what a scanner writes; plain RGB (2) is left to the image reader.
    if d.photometric != 34892 {
        return refuse(format!("PhotometricInterpretation {}", d.photometric));
    }
    Some(Kind::LinearRgb)
}

/// The kind of the bytes, `None` when they are no classic TIFF container.
pub fn kind(bytes: &[u8]) -> Option<Kind> {
    classify(&reader(Bytes::Slice(bytes))?)
}

/// The header alone decides: only the directories are read, never the samples.
pub fn probe(path: &Path) -> Option<Kind> {
    classify(&of_file(path)?)
}

/// The main image's full size, without reading its samples.
pub fn pixel_size(path: &Path) -> Option<(usize, usize)> {
    let d = main_directory(&of_file(path)?)?;
    Some((d.width, d.height))
}

/// The main image's ColorMatrix1 (XYZ to its RGB, by rows) and CalibrationIlluminant1, 0 when unnamed;
/// an empty matrix when the file states none, read as the working space's own.
pub fn calibration(path: &Path) -> Option<(Vec<f64>, i64)> {
    let d = main_directory(&of_file(path)?)?;
    Some((d.color_matrix, d.illuminant))
}

/// The main image's bits per sample, without reading its samples.
pub fn bits_per_sample(path: &Path) -> Option<usize> {
    Some(main_directory(&of_file(path)?)?.bits_per_sample)
}

/// Where one output pixel's taps land inside its step: at most four, on the centres of equal
/// slices, so their mean sits at the step's centre.
pub fn comb_offsets(step: usize) -> Vec<usize> {
    let taps = step.clamp(1, 4);
    (0..taps).map(|k| ((2 * k + 1) * step) / (2 * taps)).collect()
}

/// The file's own linear channels, normalised by its table, black and white; `longest_side`
/// subsamples while reading, a comb mean per step so grain does not fold into colour noise.
pub fn linear(path: &Path, longest_side: Option<f64>) -> Option<Linear> {
    let bytes = std::fs::read(path).ok()?;
    decode(&bytes, longest_side)
}

/// `linear` on bytes already read.
pub fn decode(bytes: &[u8], longest_side: Option<f64>) -> Option<Linear> {
    let r = reader(Bytes::Slice(bytes))?;
    if classify(&r)? != Kind::LinearRgb {
        return None;
    }
    let d = main_directory(&r)?;
    // Rounded up, so the result never exceeds the side asked for.
    let step = longest_side.map_or(1, |s| ((d.width.max(d.height) as f64 / s).ceil() as usize).max(1));
    let (out_w, out_h) = ((d.width / step).max(1), (d.height / step).max(1));
    let spp = d.samples_per_pixel;
    let bps = d.bits_per_sample / 8;
    let row_bytes = d.width.checked_mul(spp)?.checked_mul(bps)?;
    let tile_bytes = d.tile_width.checked_mul(spp)?.checked_mul(bps)?;
    out_w.checked_mul(out_h)?.checked_mul(16)?;
    let full = ((1u64 << d.bits_per_sample) - 1) as f64;
    let white: [f64; 3] = std::array::from_fn(|c| {
        let w = if d.white_level.is_empty() { full } else { d.white_level[c.min(d.white_level.len() - 1)] };
        if w > 0.0 { w } else { full }
    });
    let black: [f64; 3] = std::array::from_fn(|c| if d.black_level.is_empty() { 0.0 } else { d.black_level[c.min(d.black_level.len() - 1)] });
    // The samples index a curve onto the white level: skipping it darkens everything about 250 times.
    let table: Option<Vec<f64>> = (!d.linearization.is_empty()).then(|| d.linearization.iter().map(|&v| f64::from(v)).collect());
    let row_offset = |y: usize| -> Option<usize> {
        if !d.strip_offsets.is_empty() {
            let rps = if d.rows_per_strip > 0 { d.rows_per_strip } else { d.height };
            let strip = y / rps;
            return Some(*d.strip_offsets.get(strip)? as usize + (y - strip * rps) * row_bytes);
        }
        // One column of tiles spanning the width only; a real tile grid would need reassembly. A
        // tile's rows run its own width, padding past the image's included.
        if d.tile_width < d.width || d.tile_length == 0 {
            return None;
        }
        let tile = y / d.tile_length;
        Some(*d.tile_offsets.get(tile)? as usize + (y - tile * d.tile_length) * tile_bytes)
    };
    let value = |i: usize, c: usize| -> Option<f64> {
        let mut raw = match d.bits_per_sample {
            8 => f64::from(r.u8(i)?),
            16 => f64::from(r.u16(i)?),
            _ => {
                let v = r.u32(i)?;
                if d.sample_format == 3 { f64::from(f32::from_bits(v)) } else { f64::from(v) }
            }
        };
        // An index clamped both ends, never built from a float sample, which may carry NaN.
        if let Some(t) = &table
            && d.sample_format != 3
        {
            raw = t[(if raw.is_finite() { raw } else { 0.0 } as usize).min(t.len() - 1)];
        }
        Some(if d.sample_format == 3 && table.is_none() { raw } else { (raw - black[c]) / (white[c] - black[c]).max(1.0) })
    };
    let offsets = comb_offsets(step);
    let cols: Vec<usize> = (0..out_w).flat_map(|ox| offsets.iter().map(move |&o| (ox * step + o).min(d.width - 1) * spp * bps)).collect();
    let mut pixels = vec![[0.0f32; 4]; out_w * out_h];
    let mut wrote = false;
    for oy in 0..out_h {
        let rows: Vec<usize> = offsets.iter().filter_map(|&o| row_offset((oy * step + o).min(d.height - 1))).collect();
        for ox in 0..out_w {
            let (mut s, mut taken) = ([0.0f64; 3], 0usize);
            for &base in &rows {
                for &col in &cols[ox * offsets.len()..(ox + 1) * offsets.len()] {
                    let i = base + col;
                    let (Some(a), Some(b), Some(c)) = (value(i, 0), value(i + bps, 1), value(i + 2 * bps, 2)) else { continue };
                    s[0] += a;
                    s[1] += b;
                    s[2] += c;
                    taken += 1;
                }
            }
            if taken > 0 {
                let n = taken as f64;
                pixels[oy * out_w + ox] = [(s[0] / n) as f32, (s[1] / n) as f32, (s[2] / n) as f32, 1.0];
                wrote = true;
            }
        }
    }
    wrote.then_some(Linear { width: out_w, height: out_h, pixels })
}

#[cfg(test)]
mod tests;
