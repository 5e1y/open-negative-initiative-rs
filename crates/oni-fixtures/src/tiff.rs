//! Synthetic 16-bit TIFFs a check renders a frame from without a camera file, written here so the
//! window checks and the demos' stage share them.

use std::path::Path;

/// A 16-bit RGB TIFF naming no colour profile: a scanner's plain output, whose encoding the hand says.
pub fn plain_tiff(path: &Path, w: usize, h: usize, tone: f32) {
    tiff(path, w, h, tone, false);
}

/// A little-endian LinearRaw TIFF, 16-bit RGB in one strip, a smooth ramp scaled by `tone`: the
/// linear branch decodes it.
pub fn linear_tiff(path: &Path, w: usize, h: usize, tone: f32) {
    tiff(path, w, h, tone, true);
}

/// A LinearRaw TIFF whose third channel ramps too, so every channel has two ends to measure.
pub fn graded_tiff(path: &Path, w: usize, h: usize, tone: f32) {
    write_tiff(path, w, h, tone, true, |x, y| [x, y, (x + y) / 2.0]);
}

/// A LinearRaw TIFF whose every pixel the caller gives, each channel from 0 (black) to 1 (white).
pub fn pixels_tiff(path: &Path, w: usize, h: usize, value: impl Fn(usize, usize) -> [f32; 3]) {
    write_tiff(path, w, h, 1.0, true, |x, y| value((x * w as f32).round() as usize, (y * h as f32).round() as usize).map(|v| (v - 0.1) / 0.8));
}

fn tiff(path: &Path, w: usize, h: usize, tone: f32, linear_raw: bool) {
    write_tiff(path, w, h, tone, linear_raw, |x, y| [x, y, 0.5]);
}

fn write_tiff(path: &Path, w: usize, h: usize, tone: f32, linear_raw: bool, channels: impl Fn(f32, f32) -> [f32; 3]) {
    let samples: Vec<u16> = (0..w * h)
        .flat_map(|i| {
            let (x, y) = ((i % w) as f32 / w as f32, (i / w) as f32 / h as f32);
            channels(x, y).map(|v| (100.0 + 900.0 * (0.1 + 0.8 * v * tone)) as u16)
        })
        .collect();
    let data: Vec<u8> = samples.iter().flat_map(|v| v.to_le_bytes()).collect();
    let ifd_at = 8 + data.len();
    let mut entries: Vec<(u16, u16, u32, u32)> = vec![
        (256, 4, 1, w as u32),
        (257, 4, 1, h as u32),
        (258, 3, 1, 16),
        (259, 3, 1, 1),
        (262, 3, 1, if linear_raw { 34892 } else { 2 }),
        (273, 4, 1, 8),
        (277, 3, 1, 3),
        (278, 4, 1, h as u32),
        (279, 4, 1, data.len() as u32),
    ];
    if linear_raw {
        entries.extend([(50714, 4, 1, 100), (50717, 4, 1, 1000)]);
    }
    let mut out = b"II".to_vec();
    out.extend(42u16.to_le_bytes());
    out.extend((ifd_at as u32).to_le_bytes());
    out.extend(data);
    out.extend((entries.len() as u16).to_le_bytes());
    for (tag, t, n, v) in entries {
        out.extend(tag.to_le_bytes());
        out.extend(t.to_le_bytes());
        out.extend(n.to_le_bytes());
        if t == 3 {
            out.extend((v as u16).to_le_bytes());
            out.extend([0, 0]);
        } else {
            out.extend(v.to_le_bytes());
        }
    }
    out.extend(0u32.to_le_bytes());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, out).unwrap();
}
