//! The files the container's checks read, written by hand, and the measures they are judged by.

use super::*;

/// A LinearRaw scan in one strip, its table and white level after the IFD.
pub(super) fn test_dng(path: &Path, curve: &[u16], samples: &[u16], w: usize, h: usize, bits: usize) {
    assert_eq!(samples.len(), w * h * 3);
    let mut out: Vec<u8> = Vec::new();
    let u16 = |o: &mut Vec<u8>, v: usize| o.extend((v as u16).to_le_bytes());
    let u32 = |o: &mut Vec<u8>, v: usize| o.extend((v as u32).to_le_bytes());
    out.extend(b"II");
    u16(&mut out, 42);
    u32(&mut out, 8);
    let entries = if curve.is_empty() { 11 } else { 12 };
    let curve_at = 8 + 2 + entries * 12 + 4;
    let white_at = curve_at + curve.len() * 2;
    let bps_at = white_at + 6;
    let pixels_at = bps_at + 6;
    u16(&mut out, entries);
    let tag = |o: &mut Vec<u8>, id: usize, t: usize, n: usize, v: usize| {
        u16(o, id);
        u16(o, t);
        u32(o, n);
        if t == 3 && n == 1 {
            u16(o, v);
            u16(o, 0);
        } else {
            u32(o, v);
        }
    };
    tag(&mut out, 254, 4, 1, 0);
    tag(&mut out, 256, 3, 1, w);
    tag(&mut out, 257, 3, 1, h);
    tag(&mut out, 258, 3, 3, bps_at);
    tag(&mut out, 259, 3, 1, 1);
    tag(&mut out, 262, 3, 1, 34892);
    tag(&mut out, 273, 4, 1, pixels_at);
    tag(&mut out, 277, 3, 1, 3);
    tag(&mut out, 278, 3, 1, h);
    tag(&mut out, 279, 4, 1, w * h * 3 * bits / 8);
    if !curve.is_empty() {
        tag(&mut out, 50712, 3, curve.len(), curve_at);
    }
    tag(&mut out, 50717, 3, 3, white_at);
    u32(&mut out, 0);
    for &v in curve {
        u16(&mut out, v as usize);
    }
    for _ in 0..3 {
        u16(&mut out, 65535);
    }
    for _ in 0..3 {
        u16(&mut out, bits);
    }
    for &v in samples {
        if bits == 16 {
            u16(&mut out, v as usize);
        } else {
            out.push(v as u8);
        }
    }
    std::fs::write(path, out).unwrap();
}

/// One directory of the given entries, then 512 bytes of 0x40.
pub(super) fn raw_tiff(path: &Path, entries: &[(u16, u16, Vec<f64>)]) {
    let mut sorted = entries.to_vec();
    sorted.sort_by_key(|e| e.0);
    let mut data = vec![0x49, 0x49, 0x2A, 0, 8, 0, 0, 0];
    data.extend((sorted.len() as u16).to_le_bytes());
    let overflow_base = (8 + 2 + sorted.len() * 12 + 4) as u32;
    let mut overflow: Vec<u8> = Vec::new();
    for (tag, t, values) in &sorted {
        data.extend(tag.to_le_bytes());
        data.extend(t.to_le_bytes());
        data.extend((values.len() as u32).to_le_bytes());
        let inline = (*t == 3 && values.len() <= 2) || (*t == 4 && values.len() <= 1);
        if inline {
            if *t == 3 {
                for v in values {
                    data.extend((*v as i64 as u16).to_le_bytes());
                }
                for _ in values.len()..2 {
                    data.extend(0u16.to_le_bytes());
                }
            } else {
                data.extend((values[0] as i64 as u32).to_le_bytes());
            }
        } else {
            data.extend((overflow_base + overflow.len() as u32).to_le_bytes());
            for v in values {
                match t {
                    3 => overflow.extend((*v as i64 as u16).to_le_bytes()),
                    10 => {
                        overflow.extend((*v as i64).clamp(i32::MIN as i64, i32::MAX as i64).to_le_bytes()[..4].to_vec());
                        overflow.extend(1i32.to_le_bytes());
                    }
                    _ => overflow.extend((*v as i64 as u32).to_le_bytes()),
                }
            }
        }
    }
    data.extend(0u32.to_le_bytes());
    data.extend(overflow);
    data.extend([0x40u8; 512]);
    std::fs::write(path, data).unwrap();
}

pub(super) fn rgb(image: &Linear) -> Vec<[f64; 3]> {
    image.pixels.iter().map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]).collect()
}

/// The RMS gradient of Rec. 2020's two colour differences, two pixels in.
pub(super) fn chroma_energy(w: usize, h: usize, px: &[[f64; 3]]) -> f64 {
    let margin = 2;
    if w <= 2 * margin + 1 || h <= 2 * margin + 1 {
        return 0.0;
    }
    let chroma = |i: usize| {
        let [r, g, b] = px[i];
        let y = 0.2627 * r + 0.6780 * g + 0.0593 * b;
        (r - y, b - y)
    };
    let (mut sum, mut n) = (0.0, 0usize);
    for y in margin..h - margin - 1 {
        for x in margin..w - margin - 1 {
            let i = y * w + x;
            let (c, cx, cy) = (chroma(i), chroma(i + 1), chroma(i + w));
            let (a0, a1, b0, b1) = (cx.0 - c.0, cy.0 - c.0, cx.1 - c.1, cy.1 - c.1);
            sum += a0 * a0 + a1 * a1 + b0 * b0 + b1 * b1;
            n += 1;
        }
    }
    (sum / (4 * n) as f64).sqrt()
}

/// A seeded field box-blurred twice to a 7 px grain, 0.10 spread around 0.5.
pub(super) fn grain(side: usize) -> Vec<u16> {
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut field: Vec<f32> = (0..side * side * 3)
        .map(|_| {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            ((seed >> 33) & 0xFFFF) as f32 / 65535.0
        })
        .collect();
    let c = 7usize;
    let mut next = field.clone();
    for pass in 0..2 {
        for line in 0..side {
            for ch in 0..3 {
                let at = |k: usize| (if pass == 0 { line * side + k } else { k * side + line }) * 3 + ch;
                let mut run: f64 = (0..c).map(|k| field[at(k.min(side - 1))] as f64).sum();
                for k in 0..side {
                    next[at(k)] = (run / c as f64) as f32;
                    run -= field[at(k.saturating_sub(c / 2))] as f64;
                    run += field[at((k + c - c / 2).min(side - 1))] as f64;
                }
            }
        }
        field = next.clone();
    }
    let n = field.len() as f64;
    let mean = field.iter().map(|&v| v as f64).sum::<f64>() / n;
    let spread = (field.iter().map(|&v| (v as f64 - mean).powi(2)).sum::<f64>() / n).sqrt();
    field.iter().map(|&v| ((0.5 + 0.10 * (v as f64 - mean) / spread.max(1e-9)).clamp(0.05, 0.95) * 65535.0) as u16).collect()
}

/// A Lanczos-3 reduction by `1 / step`, separable, its kernel widened by the step as a scaler's is.
pub(super) fn lanczos(w: usize, h: usize, px: &[[f64; 3]], step: usize) -> Vec<[f64; 3]> {
    let kernel = |x: f64| {
        if x.abs() < 1e-12 {
            1.0
        } else if x.abs() >= 3.0 {
            0.0
        } else {
            let p = std::f64::consts::PI * x;
            3.0 * p.sin() * (p / 3.0).sin() / (p * p)
        }
    };
    let s = step as f64;
    let taps = |o: usize, len: usize| -> Vec<(usize, f64)> {
        let centre = (o as f64 + 0.5) * s - 0.5;
        let lo = (centre - 3.0 * s).floor() as i64;
        let hi = (centre + 3.0 * s).ceil() as i64;
        let mut t: Vec<(usize, f64)> = (lo..=hi).map(|i| (i.clamp(0, len as i64 - 1) as usize, kernel((i as f64 - centre) / s))).collect();
        let total: f64 = t.iter().map(|x| x.1).sum();
        t.iter_mut().for_each(|x| x.1 /= total);
        t
    };
    let (ow, oh) = (w / step, h / step);
    let mut rows = vec![[0.0; 3]; ow * h];
    for ox in 0..ow {
        let t = taps(ox, w);
        for y in 0..h {
            let mut acc = [0.0; 3];
            for &(x, k) in &t {
                (0..3).for_each(|c| acc[c] += k * px[y * w + x][c]);
            }
            rows[y * ow + ox] = acc;
        }
    }
    let mut out = vec![[0.0; 3]; ow * oh];
    for oy in 0..oh {
        let t = taps(oy, h);
        for ox in 0..ow {
            let mut acc = [0.0; 3];
            for &(y, k) in &t {
                (0..3).for_each(|c| acc[c] += k * rows[y * ow + ox][c]);
            }
            out[oy * ow + ox] = acc;
        }
    }
    out
}

/// The mean of the given in-step offsets, square, which a reduced reading is compared with.
pub(super) fn resampled(full: &Linear, step: usize, out: (usize, usize), offsets: &[usize]) -> Vec<[f64; 3]> {
    let src = rgb(full);
    let mut px = vec![[0.0; 3]; out.0 * out.1];
    for oy in 0..out.1 {
        for ox in 0..out.0 {
            let mut s = [0.0; 3];
            for &a in offsets {
                for &b in offsets {
                    let (sy, sx) = ((oy * step + a).min(full.height - 1), (ox * step + b).min(full.width - 1));
                    (0..3).for_each(|c| s[c] += src[sy * full.width + sx][c]);
                }
            }
            let n = (offsets.len() * offsets.len()) as f64;
            px[oy * out.0 + ox] = s.map(|v| v / n);
        }
    }
    px
}
