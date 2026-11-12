//! A synthetic mosaic DNG LibRaw reads as a camera's file, written here so the checks of several crates share it.

use std::path::Path;

pub const MOSAIC_W: usize = 96;
pub const MOSAIC_H: usize = 64;

pub type Tag = (u16, u16, u32, Vec<u8>);

/// Appends one directory, its long values and its strip, `273` pointing at the strip; the next
/// directory follows when `more`.
pub fn directory(out: &mut Vec<u8>, tags: &[Tag], strip: &[u8], more: bool) {
    let long = |v: usize| (v as u32).to_le_bytes();
    let start = out.len();
    let overflow_at = start + 2 + tags.len() * 12 + 4;
    let strip_at = overflow_at + tags.iter().filter(|t| t.3.len() > 4).map(|t| t.3.len()).sum::<usize>();
    let next = (strip_at + strip.len() + 1) & !1;
    out.extend((tags.len() as u16).to_le_bytes());
    let mut extra = Vec::new();
    for (id, kind, count, value) in tags {
        let value = if *id == 273 { long(strip_at).to_vec() } else { value.clone() };
        out.extend(id.to_le_bytes());
        out.extend(kind.to_le_bytes());
        out.extend(count.to_le_bytes());
        if value.len() <= 4 {
            out.extend(&value);
            out.extend(vec![0; 4 - value.len()]);
        } else {
            out.extend(long(overflow_at + extra.len()));
            extra.extend(value);
        }
    }
    out.extend(long(if more { next } else { 0 }));
    out.extend(extra);
    out.extend(strip);
    out.resize(next, 0);
}

/// A 96 x 64 RGGB mosaic DNG in one uncompressed strip, named for the Z f, whose matrix LibRaw's table
/// holds, so it demosaics as a camera's file does; a 16 x 8 RGB preview follows it.
pub fn mosaic_dng(path: &Path) {
    let pixels: Vec<u8> = (0..MOSAIC_W * MOSAIC_H).flat_map(|i| ((2000 + (i % MOSAIC_W) * 300 + (i / MOSAIC_W) * 150 + (i * 7919) % 900) as u16).to_le_bytes()).collect();
    let preview: Vec<u8> = (0..16 * 8 * 3).map(|i| (i * 37 % 251) as u8).collect();
    let rational = |v: f64| [((v * 10000.0).round() as i32).to_le_bytes(), 10000i32.to_le_bytes()].concat();
    let matrix: Vec<u8> = [1.1607, -0.4491, -0.0977, -0.4522, 1.246, 0.2304, -0.0458, 0.1519, 0.7616].iter().flat_map(|&v| rational(v)).collect();
    let short = |v: u16| v.to_le_bytes().to_vec();
    let long = |v: usize| (v as u32).to_le_bytes().to_vec();
    let raw: Vec<Tag> = vec![
        (254, 4, 1, long(0)),
        (256, 4, 1, long(MOSAIC_W)),
        (257, 4, 1, long(MOSAIC_H)),
        (258, 3, 1, short(16)),
        (259, 3, 1, short(1)),
        (262, 3, 1, short(32803)),
        (271, 2, 18, b"NIKON CORPORATION\0".to_vec()),
        (272, 2, 10, b"NIKON Z f\0".to_vec()),
        (273, 4, 1, Vec::new()),
        (274, 3, 1, short(1)),
        (277, 3, 1, short(1)),
        (278, 4, 1, long(MOSAIC_H)),
        (279, 4, 1, long(pixels.len())),
        (284, 3, 1, short(1)),
        (33421, 3, 2, [short(2), short(2)].concat()),
        (33422, 1, 4, vec![0, 1, 1, 2]),
        (50706, 1, 4, vec![1, 4, 0, 0]),
        (50708, 2, 10, b"Nikon Z f\0".to_vec()),
        (50717, 4, 1, long(65535)),
        (50721, 10, 9, matrix),
        (50778, 3, 1, short(21)),
    ];
    let thumb: Vec<Tag> = vec![
        (254, 4, 1, long(1)),
        (256, 4, 1, long(16)),
        (257, 4, 1, long(8)),
        (258, 3, 3, [short(8), short(8), short(8)].concat()),
        (259, 3, 1, short(1)),
        (262, 3, 1, short(2)),
        (273, 4, 1, Vec::new()),
        (277, 3, 1, short(3)),
        (278, 4, 1, long(8)),
        (279, 4, 1, long(preview.len())),
        (284, 3, 1, short(1)),
    ];
    let mut out = b"II*\0".to_vec();
    out.extend(long(8));
    directory(&mut out, &raw, &pixels, true);
    directory(&mut out, &thumb, &preview, false);
    std::fs::write(path, out).unwrap();
}

