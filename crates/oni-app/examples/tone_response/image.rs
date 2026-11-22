//! The ramps' and charts' pixels: luminance, L*, sRGB, PNG, patches laid and read back.

use super::{COLUMNS, PATCH, V3, V4};
use oni_app::render::Linear;
use std::path::Path;

pub(super) fn luminance(v: &[f32]) -> f32 {
    0.2627 * v[0] + 0.6780 * v[1] + 0.0593 * v[2]
}

pub(super) fn lstar(y: f32) -> f32 {
    let f = if y > 0.008856 { y.cbrt() } else { 7.787 * y + 16.0 / 116.0 };
    116.0 * f - 16.0
}

pub(super) fn srgb(p: &[f32; 4]) -> [u8; 3] {
    let m = [[1.6605, -0.5876, -0.0728], [-0.1246, 1.1329, -0.0083], [-0.0182, -0.1006, 1.1187]];
    let encode = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        let e = if v <= 0.0031308 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
        (e * 255.0 + 0.5) as u8
    };
    std::array::from_fn(|r| encode(m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2]))
}

pub(super) fn save_png(path: &Path, l: &Linear) {
    let file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    let mut enc = png::Encoder::new(file, l.width as u32, l.height as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().unwrap();
    w.write_image_data(&l.pixels.iter().flat_map(srgb).collect::<Vec<u8>>()).unwrap();
}

/// The classic chart's 24 patches, sRGB 8-bit, as linear Rec. 2020.
pub(super) fn chart() -> Vec<V4> {
    let srgb: [[u8; 3]; 24] = [
        [115, 82, 68], [194, 150, 130], [98, 122, 157], [87, 108, 67], [133, 128, 177], [103, 189, 170],
        [214, 126, 44], [80, 91, 166], [193, 90, 99], [94, 60, 108], [157, 188, 64], [224, 163, 46],
        [56, 61, 150], [70, 148, 73], [175, 54, 60], [231, 199, 31], [187, 86, 149], [8, 133, 161],
        [243, 243, 242], [200, 200, 200], [160, 160, 160], [122, 122, 121], [85, 85, 85], [52, 52, 52],
    ];
    let m = [[0.627404, 0.329283, 0.043313], [0.069097, 0.919540, 0.011362], [0.016391, 0.088013, 0.895595]];
    srgb.iter()
        .map(|c| {
            let lin = c.map(|v| {
                let v = f32::from(v) / 255.0;
                if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
            });
            let r: V3 = std::array::from_fn(|i| m[i][0] * lin[0] + m[i][1] * lin[1] + m[i][2] * lin[2]);
            [r[0], r[1], r[2], 1.0]
        })
        .collect()
}

/// Patches laid in rows as an image the engine decodes.
pub(super) fn patches(values: &[V4]) -> Linear {
    let rows = values.len().div_ceil(COLUMNS);
    let (w, h) = (COLUMNS * PATCH, rows * PATCH);
    let mut pixels = vec![[0.0, 0.0, 0.0, 1.0]; w * h];
    for (i, v) in values.iter().enumerate() {
        let (px, py) = ((i % COLUMNS) * PATCH, (i / COLUMNS) * PATCH);
        for y in 0..PATCH {
            for x in 0..PATCH {
                pixels[(py + y) * w + px + x] = *v;
            }
        }
    }
    Linear { width: w, height: h, pixels }
}

/// Each patch's centre read back, its median per channel.
pub(super) fn read_back(l: &Linear, count: usize) -> Vec<V3> {
    (0..count)
        .map(|i| {
            let (px, py) = ((i % COLUMNS) * PATCH, (i / COLUMNS) * PATCH);
            let mut ch: [Vec<f32>; 3] = Default::default();
            for y in PATCH / 4..3 * PATCH / 4 {
                for x in PATCH / 4..3 * PATCH / 4 {
                    let p = l.pixels[(py + y) * l.width + px + x];
                    (0..3).for_each(|c| ch[c].push(p[c]));
                }
            }
            std::array::from_fn(|c| {
                ch[c].sort_by(f32::total_cmp);
                ch[c][ch[c].len() / 2]
            })
        })
        .collect()
}
