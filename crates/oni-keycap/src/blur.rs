//! A Gaussian blur on premultiplied planes, transparent past the edges as SwiftUI's non-opaque
//! `.blur` and `.shadow` are: a kernel cut at three deviations when narrow, three boxes when wide.

/// A premultiplied image of `channels` floats per pixel (4 for RGBA, 1 for a coverage).
#[derive(Clone, Debug)]
pub struct Plane {
    pub width: usize,
    pub height: usize,
    pub channels: usize,
    pub data: Vec<f32>,
}

impl Plane {
    pub fn new(width: usize, height: usize, channels: usize) -> Self {
        Self { width, height, channels, data: vec![0.0; width * height * channels] }
    }
}

fn kernel(sigma: f32) -> Vec<f32> {
    let radius = (3.0 * sigma).ceil() as usize;
    let mut k: Vec<f32> = (0..=2 * radius)
        .map(|i| {
            let x = i as f32 - radius as f32;
            (-x * x / (2.0 * sigma * sigma)).exp()
        })
        .collect();
    let sum: f32 = k.iter().sum();
    k.iter_mut().for_each(|v| *v /= sum);
    k
}

/// Past this deviation in pixels three boxes replace the kernel: within 1 % of it, at any width.
const BOXES_FROM: f32 = 3.0;

/// Blurs `plane` in place by `sigma` pixels; under a tenth of a pixel it is left as is.
pub fn gaussian(plane: &mut Plane, sigma: f32) {
    if sigma < 0.1 || plane.width == 0 || plane.height == 0 {
        return;
    }
    // Each direction runs down the columns, whole rows at a time: the rows stay contiguous.
    columns(plane, sigma);
    let mut turned = transpose(plane);
    columns(&mut turned, sigma);
    *plane = transpose(&turned);
}

fn columns(plane: &mut Plane, sigma: f32) {
    let row = plane.width * plane.channels;
    let mut out = vec![0.0f32; plane.data.len()];
    if sigma < BOXES_FROM {
        let k = kernel(sigma);
        let r = k.len() / 2;
        for y in 0..plane.height {
            let dst = &mut out[y * row..(y + 1) * row];
            for (j, &weight) in k.iter().enumerate() {
                let Some(from) = (y + j).checked_sub(r).filter(|&f| f < plane.height) else { continue };
                let src = &plane.data[from * row..(from + 1) * row];
                dst.iter_mut().zip(src).for_each(|(d, s)| *d += s * weight);
            }
        }
        plane.data = out;
        return;
    }
    for r in box_radii(sigma) {
        boxed(&plane.data, &mut out, row, plane.height, r);
        std::mem::swap(&mut plane.data, &mut out);
    }
}

fn transpose(plane: &Plane) -> Plane {
    let (w, h, c) = (plane.width, plane.height, plane.channels);
    let mut out = Plane::new(h, w, c);
    for y in 0..h {
        for x in 0..w {
            let (from, to) = ((y * w + x) * c, (x * h + y) * c);
            out.data[to..to + c].copy_from_slice(&plane.data[from..from + c]);
        }
    }
    out
}

/// Three box radii whose summed variance is the Gaussian's (Kutskir's "boxes for Gauss").
fn box_radii(sigma: f32) -> [usize; 3] {
    let n = 3.0f32;
    let ideal = (12.0 * sigma * sigma / n + 1.0).sqrt();
    let mut lower = ideal.floor() as i32;
    if lower % 2 == 0 {
        lower -= 1;
    }
    let upper = lower + 2;
    let l = lower as f32;
    let m = ((12.0 * sigma * sigma - n * l * l - 4.0 * n * l - 3.0 * n) / (-4.0 * l - 4.0)).round() as i32;
    std::array::from_fn(|i| (if (i as i32) < m { lower } else { upper } as usize - 1) / 2)
}

/// A running-sum box of radius `r` down the rows, zero past the ends.
fn boxed(src: &[f32], dst: &mut [f32], row: usize, h: usize, r: usize) {
    let scale = 1.0 / (2 * r + 1) as f32;
    let mut acc = vec![0.0f32; row];
    let add = |acc: &mut [f32], y: usize, sign: f32| {
        acc.iter_mut().zip(&src[y * row..(y + 1) * row]).for_each(|(a, s)| *a += sign * s);
    };
    for y in 0..r.min(h) {
        add(&mut acc, y, 1.0);
    }
    for y in 0..h {
        if y + r < h {
            add(&mut acc, y + r, 1.0);
        }
        if y > r {
            add(&mut acc, y - r - 1, -1.0);
        }
        dst[y * row..(y + 1) * row].iter_mut().zip(&acc).for_each(|(d, a)| *d = (a * scale).max(0.0));
    }
}
