//! The display kernel's CPU reference: matrix, transfer, dither and quantisation, pixel by pixel.

use super::*;

/// True where a texel falls on a stripe the screen cannot show: `gamut::stripe` on its canvas pixel, y up.
pub(super) fn on_stripe(x: u32, y: u32, k: &Params) -> bool {
    let cx = k.origin[0] + (x as f32 + 0.5) * k.stretch;
    let cy = k.canvas_height - (k.origin[1] + (y as f32 + 0.5) * k.stretch);
    (cx + cy + k.phase).rem_euclid(k.period) < k.duty * k.period
}

pub(super) fn beyond(p: [f32; 4], k: &Params) -> bool {
    (0..3).any(|c| {
        let r = k.rows[c];
        let shown = r[0] * p[0] + r[1] * p[1] + r[2] * p[2];
        shown < k.low[c] || shown > k.high[c]
    })
}

/// `pcg` from Jarzynski and Olano, the hash both sides share bit for bit.
pub(super) fn pcg(v: u32) -> u32 {
    let state = v.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277_803_737);
    (word >> 22) ^ word
}

pub(super) fn unit(v: u32) -> f32 {
    (v >> 8) as f32 / 16_777_216.0
}

/// The sRGB transfer function, which Display P3 shares.
pub(super) fn transfer(v: f32) -> f32 {
    if v <= 0.003_130_8 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

/// One pixel as the kernel writes it: a triangular dither of one step, then the nearest of 256 levels.
pub fn encode_pixel(p: [f32; 4], x: u32, y: u32, k: &Params) -> [u8; 4] {
    let alpha = p[3].clamp(0.0, 1.0);
    let p = if k.zebra > 0.5 && beyond(p, k) && on_stripe(x, y, k) { k.tint } else { p };
    let h = pcg(x ^ pcg(y ^ pcg(k.seed)));
    let mut out = [0u8, 0, 0, 255];
    for c in 0..3 {
        let r = k.rows[c];
        // `composited(over:)` the ground, in light; the rows keep a grey grey.
        let linear = ((r[0] * p[0] + r[1] * p[1] + r[2] * p[2]) * alpha + k.ground * (1.0 - alpha)).clamp(0.0, 1.0);
        let a = pcg(h.wrapping_add(2 * c as u32));
        let b = pcg(h.wrapping_add(2 * c as u32 + 1));
        let d = (unit(a) + unit(b) - 1.0) * k.dither;
        out[c] = (transfer(linear) * 255.0 + d).round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// The whole image on the CPU, row-major from the top: the software renderer's path.
pub fn encode(pixels: &[[f32; 4]], k: &Params) -> Vec<u8> {
    let w = k.width.max(1);
    pixels.iter().enumerate().flat_map(|(i, p)| encode_pixel(*p, i as u32 % w, i as u32 / w, k)).collect()
}
