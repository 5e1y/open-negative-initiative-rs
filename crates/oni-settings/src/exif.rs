//! The EXIF capture date of a file, `DateTimeOriginal` as stored, read from the TIFF structure its
//! bytes carry: a TIFF-based RAW, a TIFF, a JPEG, a RAF, a CR3, a HEIF or AVIF, a PNG.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

mod containers;

const EXIF_IFD: u16 = 0x8769;
const DATE_TIME_ORIGINAL: u16 = 0x9003;
const ASCII: u16 = 2;

/// Positioned reads in one byte order, offsets counted from the TIFF header at `base`.
struct Reader {
    file: File,
    base: u64,
    little: bool,
}

impl Reader {
    fn bytes(&mut self, at: u64, count: usize) -> Option<Vec<u8>> {
        self.file.seek(SeekFrom::Start(self.base.checked_add(at)?)).ok()?;
        let mut out = vec![0; count];
        self.file.read_exact(&mut out).ok()?;
        Some(out)
    }

    fn u16(&mut self, at: u64) -> Option<u16> {
        let b = self.bytes(at, 2)?;
        Some(if self.little { u16::from_le_bytes([b[0], b[1]]) } else { u16::from_be_bytes([b[0], b[1]]) })
    }

    fn u32(&mut self, at: u64) -> Option<u32> {
        let b = self.bytes(at, 4)?;
        let b = [b[0], b[1], b[2], b[3]];
        Some(if self.little { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) })
    }

    /// The directory entry `tag` of the IFD at `offset`: its type, count and the offset of its value.
    fn entry(&mut self, offset: u64, tag: u16) -> Option<(u16, u32, u64)> {
        let count = self.u16(offset)?;
        for i in 0..u64::from(count) {
            let at = offset + 2 + i * 12;
            if self.u16(at)? == tag {
                let kind = self.u16(at + 2)?;
                let n = self.u32(at + 4)?;
                let size = if kind == ASCII { u64::from(n) } else { 4 };
                let value = if size <= 4 { at + 8 } else { u64::from(self.u32(at + 8)?) };
                return Some((kind, n, value));
            }
        }
        None
    }
}

/// Where a file's TIFF header sits, and whether its first directory is IFD0 or EXIF's own.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Found {
    /// IFD0 first, pointing to the EXIF IFD: a TIFF, a JPEG's APP1, a HEIF item, a PNG chunk.
    Tiff(u64),
    /// The EXIF IFD first, as a CR3's `CMT2` box stores it.
    Exif(u64),
}

/// The file's EXIF directory, wherever its container keeps the TIFF structure.
fn exif_directory(path: &Path) -> Option<(Reader, u64)> {
    let mut file = File::open(path).ok()?;
    let found = containers::locate(&mut file)?;
    let base = match found {
        Found::Tiff(base) | Found::Exif(base) => base,
    };
    let mut reader = Reader { file, base, little: true };
    let order = reader.bytes(0, 4)?;
    reader.little = match &order[..2] {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    // 42 is TIFF; RW2 writes 0x55, ORF "RO" or "RS", all with the same directories after.
    let magic = reader.u16(2)?;
    if ![42, 0x55, 0x4F52, 0x5352].contains(&magic) {
        return None;
    }
    let first = u64::from(reader.u32(4)?);
    if matches!(found, Found::Exif(_)) {
        return Some((reader, first));
    }
    let (_, _, pointer) = reader.entry(first, EXIF_IFD)?;
    let exif = u64::from(reader.u32(pointer)?);
    Some((reader, exif))
}

/// `DateTimeOriginal` as the file stores it (`2026:06:18 14:32:10`), `None` when it carries none.
pub fn capture_date(path: &Path) -> Option<String> {
    let (mut reader, exif) = exif_directory(path)?;
    let (kind, count, value) = reader.entry(exif, DATE_TIME_ORIGINAL)?;
    if kind != ASCII || count == 0 || count > 64 {
        return None;
    }
    let raw = reader.bytes(value, count as usize)?;
    let text = raw.split(|&b| b == 0).next().unwrap_or(&[]);
    let text = String::from_utf8_lossy(text).into_owned();
    (!text.is_empty()).then_some(text)
}

const EXPOSURE_TIME: u16 = 0x829A;
const F_NUMBER: u16 = 0x829D;
const ISO_SPEED_RATINGS: u16 = 0x8827;
const SHORT: u16 = 3;
const LONG: u16 = 4;
const RATIONAL: u16 = 5;

/// Shutter in seconds, aperture as its f-number and ISO, as ImageIO hands them as doubles.
pub fn exposure(path: &Path) -> Option<(f64, f64, f64)> {
    let (mut reader, exif) = exif_directory(path)?;
    let mut rational = |tag: u16| -> Option<f64> {
        let (kind, _, value) = reader.entry(exif, tag)?;
        if kind != RATIONAL {
            return None;
        }
        // A rational's eight bytes never fit the entry: its value field holds their offset.
        let at = u64::from(reader.u32(value)?);
        let (n, d) = (reader.u32(at)?, reader.u32(at + 4)?);
        (d != 0).then(|| f64::from(n) / f64::from(d))
    };
    let shutter = rational(EXPOSURE_TIME)?;
    let aperture = rational(F_NUMBER)?;
    let (kind, count, value) = reader.entry(exif, ISO_SPEED_RATINGS)?;
    let iso = match kind {
        SHORT if count >= 1 => f64::from(reader.u16(value)?),
        LONG if count >= 1 => f64::from(reader.u32(value)?),
        _ => return None,
    };
    Some((shutter, aperture, iso))
}

#[cfg(test)]
mod tests;
