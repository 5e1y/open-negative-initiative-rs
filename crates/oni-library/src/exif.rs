//! The EXIF capture date of a file, `DateTimeOriginal` as stored, read from the TIFF structure its
//! bytes carry: a TIFF-based RAW (NEF, DNG, RW2, ORF, ARW, CR2, PEF...), a TIFF, or a JPEG.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

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

/// `DateTimeOriginal` as the file stores it (`2026:06:18 14:32:10`), `None` when it carries none.
pub fn capture_date(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut head = [0u8; 4];
    file.read_exact(&mut head).ok()?;
    let base = if head[0] == 0xFF && head[1] == 0xD8 { jpeg_exif(&mut file)? } else { 0 };
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
    let (_, _, pointer) = reader.entry(first, EXIF_IFD)?;
    let exif = u64::from(reader.u32(pointer)?);
    let (kind, count, value) = reader.entry(exif, DATE_TIME_ORIGINAL)?;
    if kind != ASCII || count == 0 || count > 64 {
        return None;
    }
    let raw = reader.bytes(value, count as usize)?;
    let text = raw.split(|&b| b == 0).next().unwrap_or(&[]);
    let text = String::from_utf8_lossy(text).into_owned();
    (!text.is_empty()).then_some(text)
}

/// The offset of the TIFF header inside a JPEG's `Exif` APP1 segment.
fn jpeg_exif(file: &mut File) -> Option<u64> {
    let mut at = 2u64;
    loop {
        file.seek(SeekFrom::Start(at)).ok()?;
        let mut marker = [0u8; 4];
        file.read_exact(&mut marker).ok()?;
        if marker[0] != 0xFF || marker[1] == 0xDA || marker[1] == 0xD9 {
            return None;
        }
        let length = u64::from(u16::from_be_bytes([marker[2], marker[3]]));
        if marker[1] == 0xE1 {
            let mut name = [0u8; 6];
            file.read_exact(&mut name).ok()?;
            if &name == b"Exif\0\0" {
                return Some(at + 10);
            }
        }
        at += 2 + length;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    /// A little or big-endian TIFF whose EXIF IFD holds `date`, built byte by byte.
    fn tiff(little: bool, date: &str) -> Vec<u8> {
        let w16 = |v: u16| if little { v.to_le_bytes() } else { v.to_be_bytes() };
        let w32 = |v: u32| if little { v.to_le_bytes() } else { v.to_be_bytes() };
        let mut out = Vec::new();
        out.extend_from_slice(if little { b"II" } else { b"MM" });
        out.extend_from_slice(&w16(42));
        out.extend_from_slice(&w32(8));
        // IFD0 at 8: one entry, the EXIF pointer to 26.
        out.extend_from_slice(&w16(1));
        out.extend_from_slice(&w16(EXIF_IFD));
        out.extend_from_slice(&w16(4));
        out.extend_from_slice(&w32(1));
        out.extend_from_slice(&w32(26));
        out.extend_from_slice(&w32(0));
        // EXIF IFD at 26: DateTimeOriginal, its 20 bytes at 44.
        out.extend_from_slice(&w16(1));
        out.extend_from_slice(&w16(DATE_TIME_ORIGINAL));
        out.extend_from_slice(&w16(ASCII));
        out.extend_from_slice(&w32(20));
        out.extend_from_slice(&w32(44));
        out.extend_from_slice(&w32(0));
        out.extend_from_slice(date.as_bytes());
        out.push(0);
        out
    }

    #[test]
    fn a_tiff_and_a_jpeg_give_their_original_date_in_either_byte_order() {
        let scratch = Scratch::new("exif");
        let date = "2026:06:18 14:32:10";
        for little in [true, false] {
            let path = scratch.path().join(format!("t{little}.tif"));
            std::fs::write(&path, tiff(little, date)).unwrap();
            assert_eq!(capture_date(&path).as_deref(), Some(date));
        }
        let body = tiff(true, date);
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 0, 0, 0xFF, 0xE1];
        jpeg.extend_from_slice(&((body.len() + 8) as u16).to_be_bytes());
        jpeg.extend_from_slice(b"Exif\0\0");
        jpeg.extend_from_slice(&body);
        let path = scratch.path().join("t.jpg");
        std::fs::write(&path, &jpeg).unwrap();
        assert_eq!(capture_date(&path).as_deref(), Some(date));

        // Twin: the same bytes with the byte-order mark flipped read nothing sensible.
        let mut flipped = tiff(true, date);
        flipped[..2].copy_from_slice(b"MM");
        let path = scratch.path().join("flipped.tif");
        std::fs::write(&path, flipped).unwrap();
        assert_ne!(capture_date(&path).as_deref(), Some(date));
        // Twin: an empty file carries no date.
        let path = scratch.path().join("empty.NEF");
        std::fs::write(&path, b"").unwrap();
        assert_eq!(capture_date(&path), None);
    }
}
