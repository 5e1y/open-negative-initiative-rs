//! The scalar side of `wgsl/prelude.wgsl`: luminance, the display gamma, the contrast family, vector helpers.

use crate::Image;

pub(crate) fn v3(a: V3, w: f32) -> V4 {
    [a[0], a[1], a[2], w]
}

pub(crate) fn v22(a: V2, b: V2) -> V4 {
    [a[0], a[1], b[0], b[1]]
}

/// The uniform block a shader reads: the head, then the rows.
pub fn uniform(head: [u32; 4], rows: &[V4]) -> Vec<u8> {
    let mut out = bytemuck::bytes_of(&head).to_vec();
    out.extend_from_slice(bytemuck::cast_slice(rows));
    out
}

pub(crate) const LUMA: [f32; 3] = [0.2627, 0.6780, 0.0593];

pub(crate) const DISPLAY_GAMMA: f32 = 2.2;

pub(crate) const CONTRAST_WARP: f32 = 0.889_272_9;

pub(crate) type V3 = [f32; 3];

/// Metal's `clamp`, `fmin(fmax(x, low), high)`: a NaN takes the low bound, and crossed bounds
/// never panic as `f32::clamp` does.
pub(crate) fn clamp(x: f32, low: f32, high: f32) -> f32 {
    x.max(low).min(high)
}

pub(crate) fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(crate) fn map(a: V3, f: impl Fn(usize, f32) -> f32) -> V3 {
    [f(0, a[0]), f(1, a[1]), f(2, a[2])]
}

pub(crate) fn contrasted(x: f32, travel: f32) -> f32 {
    let b = travel.abs() * 15.2;
    if b <= 1e-4 {
        return x;
    }
    let mut t = clamp(x, 0.0, 1.0).powf(CONTRAST_WARP);
    let low = 1.0 / (1.0 + (b * 0.5).exp());
    let high = 1.0 / (1.0 + (-b * 0.5).exp());
    if travel > 0.0 {
        t = (1.0 / (1.0 + (-b * (t - 0.5)).exp()) - low) / (high - low);
    } else {
        let g = low + t * (high - low);
        t = clamp(0.5 - (1.0 / g - 1.0).ln() / b, 0.0, 1.0);
    }
    t.powf(1.0 / CONTRAST_WARP)
}

pub(crate) type V2 = [f32; 2];

pub(crate) type V4 = [f32; 4];

pub(crate) fn rgb(p: V4) -> V3 {
    [p[0], p[1], p[2]]
}

pub(crate) fn with_alpha(c: V3, a: f32) -> V4 {
    [c[0], c[1], c[2], a]
}

pub(crate) fn mix(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

pub(crate) fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
    let t = clamp((x - low) / (high - low), 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(crate) fn image_map(image: &Image, f: impl Fn(usize, usize, V4) -> V4) -> Image {
    let pixels = (0..image.height)
        .flat_map(|y| (0..image.width).map(move |x| (x, y)))
        .map(|(x, y)| f(x, y, image.at(x, y)))
        .collect();
    image.with_pixels(pixels)
}

pub(crate) fn dot4(a: V4, b: V4) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}
