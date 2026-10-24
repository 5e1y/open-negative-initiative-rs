//! Where each container keeps its TIFF structure: a JPEG's `Exif` APP1, a RAF's embedded JPEG, a
//! CR3's `CMT2` box, a HEIF or AVIF's `Exif` item, a PNG's `eXIf` chunk, else the file's own head.

use super::Found;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

/// Canon's box under `moov` that holds the CR3's metadata, `CMT1` to `CMT4`.
const CANON_UUID: [u8; 16] = [0x85, 0xc0, 0xb6, 0x87, 0x82, 0x0f, 0x11, 0xe0, 0x81, 0x11, 0xf4, 0xce, 0x46, 0x2b, 0x6a, 0x48];
/// Boxes or chunks looked at in one level: a damaged length never walks a file for ever.
const MOST: usize = 4096;

fn read_at(file: &mut File, at: u64, count: usize) -> Option<Vec<u8>> {
    file.seek(SeekFrom::Start(at)).ok()?;
    let mut out = vec![0; count];
    file.read_exact(&mut out).ok()?;
    Some(out)
}

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// The TIFF header's place by the file's first bytes; a head no container claims is a TIFF's own.
pub(super) fn locate(file: &mut File) -> Option<Found> {
    let len = file.metadata().ok()?.len();
    let head = read_at(file, 0, 16.min(len as usize))?;
    match head.as_slice() {
        [0xFF, 0xD8, ..] => jpeg_exif(file, 0).map(Found::Tiff),
        [b'F', b'U', b'J', b'I', b'F', b'I', b'L', b'M', b'C', b'C', b'D', b'-', b'R', b'A', b'W', ..] => {
            // The embedded JPEG's offset, big-endian at 84, carries the frame's EXIF.
            let at = u64::from(be32(&read_at(file, 84, 4)?));
            jpeg_exif(file, at).map(Found::Tiff)
        }
        [0x89, b'P', b'N', b'G', ..] => png_exif(file, len).map(Found::Tiff),
        [_, _, _, _, b'f', b't', b'y', b'p', b'c', b'r', b'x', b' ', ..] => cr3_exif(file, len).map(Found::Exif),
        [_, _, _, _, b'f', b't', b'y', b'p', ..] => heif_exif(file, len).map(Found::Tiff),
        _ => Some(Found::Tiff(0)),
    }
}

/// The offset of the TIFF header inside the `Exif` APP1 segment of the JPEG starting at `start`.
fn jpeg_exif(file: &mut File, start: u64) -> Option<u64> {
    let mut at = start + 2;
    loop {
        let marker = read_at(file, at, 4)?;
        if marker[0] != 0xFF || marker[1] == 0xDA || marker[1] == 0xD9 {
            return None;
        }
        let length = u64::from(u16::from_be_bytes([marker[2], marker[3]]));
        if marker[1] == 0xE1 && read_at(file, at + 4, 6)? == b"Exif\0\0" {
            return Some(at + 10);
        }
        at += 2 + length;
    }
}

/// The boxes from `start` to `end`: each one's type, where its body starts and where it ends.
fn boxes(file: &mut File, start: u64, end: u64) -> Vec<([u8; 4], u64, u64)> {
    let mut out = Vec::new();
    let mut at = start;
    while at + 8 <= end && out.len() < MOST {
        let Some(head) = read_at(file, at, 8) else { break };
        let kind = [head[4], head[5], head[6], head[7]];
        let (body, size) = match be32(&head) {
            0 => (at + 8, end - at),
            1 => match read_at(file, at + 8, 8) {
                Some(b) => (at + 16, u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])),
                None => break,
            },
            n => (at + 8, u64::from(n)),
        };
        let Some(stop) = at.checked_add(size).filter(|&s| s <= end && s >= body) else { break };
        out.push((kind, body, stop));
        at = stop;
    }
    out
}

fn child(file: &mut File, start: u64, end: u64, kind: &[u8; 4]) -> Option<(u64, u64)> {
    boxes(file, start, end).into_iter().find(|(k, ..)| k == kind).map(|(_, body, stop)| (body, stop))
}

/// `moov`, then Canon's `uuid`, then `CMT2`: a TIFF whose first directory is the EXIF IFD.
fn cr3_exif(file: &mut File, len: u64) -> Option<u64> {
    let (moov, moov_end) = child(file, 0, len, b"moov")?;
    for (kind, body, stop) in boxes(file, moov, moov_end) {
        if &kind == b"uuid" && read_at(file, body, 16)? == CANON_UUID {
            return child(file, body + 16, stop, b"CMT2").map(|(cmt, _)| cmt);
        }
    }
    None
}

/// A big-endian unsigned of `size` bytes (0, 4 or 8) at `at`.
fn sized(file: &mut File, at: u64, size: u64) -> Option<u64> {
    match size {
        0 => Some(0),
        4 => read_at(file, at, 4).map(|b| u64::from(be32(&b))),
        8 => read_at(file, at, 8).map(|b| u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])),
        _ => None,
    }
}

/// `meta`'s `iinf` names the `Exif` item, `iloc` places it; its body opens on the offset to the header.
fn heif_exif(file: &mut File, len: u64) -> Option<u64> {
    let (meta, meta_end) = child(file, 0, len, b"meta")?;
    // `meta` is a full box: four bytes of version and flags before its children.
    let (iinf, iinf_end) = child(file, meta + 4, meta_end, b"iinf")?;
    let version = read_at(file, iinf, 1)?[0];
    let first = iinf + 4 + if version == 0 { 2 } else { 4 };
    let id = boxes(file, first, iinf_end).into_iter().filter(|(k, ..)| k == b"infe").find_map(|(_, body, _)| {
        let v = read_at(file, body, 1)?[0];
        let (id, kind) = match v {
            2 => {
                let b = read_at(file, body + 4, 8)?;
                (u32::from(u16::from_be_bytes([b[0], b[1]])), [b[4], b[5], b[6], b[7]])
            }
            3 => {
                let b = read_at(file, body + 4, 10)?;
                (be32(&b), [b[6], b[7], b[8], b[9]])
            }
            _ => return None,
        };
        (&kind == b"Exif").then_some(id)
    })?;
    let (iloc, _) = child(file, meta + 4, meta_end, b"iloc")?;
    let head = read_at(file, iloc, 6)?;
    let version = head[0];
    let (offset_size, length_size, base_size) = (u64::from(head[4] >> 4), u64::from(head[4] & 15), u64::from(head[5] >> 4));
    let index_size = if version >= 1 { u64::from(head[5] & 15) } else { 0 };
    let (count, mut at) = if version < 2 {
        (sized_u16(file, iloc + 6)?, iloc + 8)
    } else {
        (u64::from(be32(&read_at(file, iloc + 6, 4)?)), iloc + 10)
    };
    for _ in 0..count.min(MOST as u64) {
        let item = if version < 2 { sized_u16(file, at)? } else { u64::from(be32(&read_at(file, at, 4)?)) };
        at += if version < 2 { 2 } else { 4 };
        if version >= 1 {
            at += 2;
        }
        at += 2;
        let base = sized(file, at, base_size)?;
        at += base_size;
        let extents = sized_u16(file, at)?;
        at += 2;
        let mut first = None;
        for _ in 0..extents {
            at += index_size;
            let offset = sized(file, at, offset_size)?;
            at += offset_size + length_size;
            first.get_or_insert(base + offset);
        }
        if item == u64::from(id) {
            // The item's body opens on four bytes: the offset from there to the TIFF header.
            let start = first?;
            return Some(start + 4 + u64::from(be32(&read_at(file, start, 4)?)));
        }
    }
    None
}

fn sized_u16(file: &mut File, at: u64) -> Option<u64> {
    read_at(file, at, 2).map(|b| u64::from(u16::from_be_bytes([b[0], b[1]])))
}

/// The `eXIf` chunk's body, past an `Exif\0\0` some writers leave before the header.
fn png_exif(file: &mut File, len: u64) -> Option<u64> {
    let mut at = 8;
    for _ in 0..MOST {
        let head = read_at(file, at, 8)?;
        let size = u64::from(be32(&head));
        if &head[4..] == b"eXIf" {
            let body = at + 8;
            return Some(if read_at(file, body, 6)? == b"Exif\0\0" { body + 6 } else { body });
        }
        if &head[4..] == b"IEND" || at + 12 + size > len {
            return None;
        }
        at += 12 + size;
    }
    None
}
