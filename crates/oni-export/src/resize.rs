//! Output resizing on one edge, and the Lanczos resampler that applies it.

use crate::Image;

/// None, or a ceiling on one edge: a ceiling on an edge treats the portraits and landscapes of
/// one roll alike, where a ceiling on the width would not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resize {
    Full,
    LongestEdge(i64),
    ShortestEdge(i64),
}

impl Resize {
    /// Never enlarges: that would manufacture blur mistaken for a focus error.
    pub fn scale(self, width: f64, height: f64) -> f64 {
        let (edge, target) = match self {
            Resize::Full => return 1.0,
            Resize::LongestEdge(n) => (width.max(height), n),
            Resize::ShortestEdge(n) => (width.min(height), n),
        };
        if edge.is_nan() || edge <= 0.0 {
            return 1.0;
        }
        (target as f64 / edge).min(1.0)
    }

    /// The field typed decides the edge the size rides: a landscape's width is its long edge.
    /// Clamped to the original, since an export never enlarges; the original reads `Full`.
    pub fn typing_width(width: i64, full: (f64, f64)) -> Resize {
        typed(width, full.0, full.1, true)
    }

    pub fn typing_height(height: i64, full: (f64, f64)) -> Resize {
        typed(height, full.1, full.0, false)
    }

    /// The file's size, rounded as the resampler crops it.
    pub fn output_size(self, width: f64, height: f64) -> (f64, f64) {
        let scale = self.scale(width, height);
        ((width * scale).round(), (height * scale).round())
    }
}

fn typed(value: i64, edge: f64, other: f64, width_typed: bool) -> Resize {
    if value <= 0 || edge.is_nan() || edge <= 0.0 || value as f64 >= edge {
        return Resize::Full;
    }
    let typed_is_long = if width_typed { edge >= other } else { edge > other };
    if typed_is_long { Resize::LongestEdge(value) } else { Resize::ShortestEdge(value) }
}

/// Lanczos lobes; the kernel stretches by the reduction so every source pixel is read.
const LOBES: f64 = 3.0;

fn lanczos(t: f64) -> f64 {
    let t = t.abs();
    if t < 1e-12 {
        return 1.0;
    }
    if t >= LOBES {
        return 0.0;
    }
    let pt = std::f64::consts::PI * t;
    LOBES * pt.sin() * (pt / LOBES).sin() / (pt * pt)
}

/// Where the border is read from: the edge pixel repeated, or transparent black.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Border {
    Clamped,
    #[cfg_attr(not(test), allow(dead_code))]
    Transparent,
}

/// One output index's taps: the first source index (may run outside) and normalised weights.
struct Taps {
    first: i64,
    weights: Vec<f32>,
}

fn taps(output: usize, scale: f64) -> Vec<Taps> {
    let support = LOBES / scale;
    (0..output)
        .map(|i| {
            let centre = (i as f64 + 0.5) / scale;
            let first = (centre - support).floor() as i64;
            let last = (centre + support).ceil() as i64;
            let raw: Vec<f64> = (first..=last).map(|j| lanczos((j as f64 + 0.5 - centre) * scale)).collect();
            let sum: f64 = raw.iter().sum();
            Taps { first, weights: raw.iter().map(|w| (w / sum) as f32).collect() }
        })
        .collect()
}

/// Applied last, after sharpening: scaling first would double the unsharp mask's effective
/// radius in the produced file.
/// Premultiplied while resampled, as Core Image resamples: a clear pixel, whatever colour the
/// chain gave it, lends a neighbour nothing.
pub fn resized(image: &Image, scale: f64) -> Image {
    if scale.is_nan() || scale >= 1.0 {
        return image.clone();
    }
    let mut premultiplied = image.clone();
    for p in premultiplied.pixels.chunks_exact_mut(4) {
        let a = p[3].clamp(0.0, 1.0);
        for v in &mut p[..3] {
            *v *= a;
        }
    }
    let mut out = resampled(&premultiplied, scale, Border::Clamped);
    for p in out.pixels.chunks_exact_mut(4) {
        let a = p[3];
        for v in &mut p[..3] {
            *v = if a > 1e-6 { *v / a } else { 0.0 };
        }
    }
    out
}

/// The frame on the uniform `scale`, cropped to the rounded size the sheet announced; output
/// pixel centres map back through the scale alone, as an affine transform places them.
pub(crate) fn resampled(image: &Image, scale: f64, border: Border) -> Image {
    if scale.is_nan() || scale >= 1.0 || image.width == 0 || image.height == 0 {
        return image.clone();
    }
    let out_w = (image.width as f64 * scale).round();
    let out_h = (image.height as f64 * scale).round();
    // A size rounding to nothing crops to an empty frame, which the conversion refuses as a render.
    if out_w < 1.0 || out_h < 1.0 {
        return Image { width: 0, height: 0, pixels: Vec::new() };
    }
    let (out_w, out_h) = (out_w as usize, out_h as usize);
    let across = taps(out_w, scale);
    let down = taps(out_h, scale);

    let read = |len: usize, j: i64| -> Option<usize> {
        if (0..len as i64).contains(&j) {
            Some(j as usize)
        } else if border == Border::Clamped {
            Some(j.clamp(0, len as i64 - 1) as usize)
        } else {
            None
        }
    };

    let mut wide = vec![0.0f32; out_w * image.height * 4];
    rows_parallel(&mut wide, out_w * 4, |y, row| {
        let src = &image.pixels[y * image.width * 4..(y + 1) * image.width * 4];
        for (x, t) in across.iter().enumerate() {
            let mut acc = [0.0f32; 4];
            for (k, w) in t.weights.iter().enumerate() {
                if let Some(j) = read(image.width, t.first + k as i64) {
                    for c in 0..4 {
                        acc[c] += w * src[j * 4 + c];
                    }
                }
            }
            row[x * 4..x * 4 + 4].copy_from_slice(&acc);
        }
    });

    let mut out = vec![0.0f32; out_w * out_h * 4];
    rows_parallel(&mut out, out_w * 4, |y, row| {
        let t = &down[y];
        for (k, w) in t.weights.iter().enumerate() {
            let Some(j) = read(image.height, t.first + k as i64) else { continue };
            let src = &wide[j * out_w * 4..(j + 1) * out_w * 4];
            for (o, s) in row.iter_mut().zip(src) {
                *o += w * s;
            }
        }
    });
    Image { width: out_w, height: out_h, pixels: out }
}

/// Fills `buffer` row by row on every core.
fn rows_parallel(buffer: &mut [f32], row_len: usize, fill: impl Fn(usize, &mut [f32]) + Sync) {
    let rows = buffer.len() / row_len;
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(rows.max(1));
    let per = rows.div_ceil(threads);
    std::thread::scope(|scope| {
        for (chunk_index, chunk) in buffer.chunks_mut(per * row_len).enumerate() {
            let fill = &fill;
            scope.spawn(move || {
                for (i, row) in chunk.chunks_mut(row_len).enumerate() {
                    fill(chunk_index * per + i, row);
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plate(width: usize, height: usize, grey: f32) -> Image {
        Image { width, height, pixels: [grey, grey, grey, 1.0].repeat(width * height) }
    }

    /// The lowest alpha on the four border lines: a partly covered edge reads as a pale hairline.
    fn edge_alpha(image: &Image) -> f32 {
        let (w, h) = (image.width, image.height);
        let a = |x: usize, y: usize| image.pixels[(y * w + x) * 4 + 3];
        let mut lowest = 1.0f32;
        for x in 0..w {
            lowest = lowest.min(a(x, 0)).min(a(x, h - 1));
        }
        for y in 0..h {
            lowest = lowest.min(a(0, y)).min(a(w - 1, y));
        }
        lowest
    }

    #[test]
    fn a_resized_export_is_opaque_to_its_last_pixel() {
        // 512 of 837 leaves the short side on 341.33: neither axis lands on the source grid.
        let source = plate(837, 558, 0.16);
        let ratio = 512.0 / 837.0;
        let shrunk = resized(&source, ratio);
        assert!((edge_alpha(&shrunk) - 1.0).abs() < 1e-6, "edge alpha {}", edge_alpha(&shrunk));
        let (w, h) = Resize::LongestEdge(512).output_size(837.0, 558.0);
        assert_eq!((shrunk.width as f64, shrunk.height as f64), (w, h));
        assert_eq!((shrunk.width, shrunk.height), (512, 341));
        // A flat plate stays flat: the weights sum to one.
        assert!(shrunk.pixels.chunks(4).all(|p| (p[0] - 0.16).abs() < 1e-6));
        // Twin: read past the border as transparent, the edge leaks.
        let leaking = resampled(&source, ratio, Border::Transparent);
        assert!(edge_alpha(&leaking) < 0.99, "unclamped edge alpha {}", edge_alpha(&leaking));
    }

    #[test]
    fn lanczos_matches_an_independent_lanczos3_inside_the_frame() {
        use image::{ImageBuffer, Rgba, imageops};
        let (w, h) = (400usize, 300usize);
        let mut state = 0x9E37_79B9u32;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as f32 / u32::MAX as f32
        };
        // Within a quarter of mid-grey, so the oracle's clip to [0, 1] never meets the ringing.
        let mut sample = move || 0.25 + 0.5 * next();
        let pixels: Vec<f32> = (0..w * h).flat_map(|_| [sample(), sample(), sample(), 1.0]).collect();
        let source = Image { width: w, height: h, pixels: pixels.clone() };
        let buffer: ImageBuffer<Rgba<f32>, Vec<f32>> = ImageBuffer::from_raw(w as u32, h as u32, pixels).unwrap();
        let ours = resized(&source, 0.5);
        let worst = |filter| {
            let theirs = imageops::resize(&buffer, 200, 150, filter);
            let mut worst = 0.0f32;
            // Away from the border, where the oracle renormalises instead of repeating the edge.
            for y in 8..142 {
                for x in 8..192 {
                    for c in 0..3 {
                        let a = ours.pixels[(y * 200 + x) * 4 + c];
                        let b = theirs.get_pixel(x as u32, y as u32)[c];
                        worst = worst.max((a - b).abs());
                    }
                }
            }
            worst
        };
        let lanczos = worst(imageops::FilterType::Lanczos3);
        assert!(lanczos < 1e-4, "Lanczos3 gap {lanczos}");
        // Twin: the same oracle on a triangle kernel is visibly another filter.
        let triangle = worst(imageops::FilterType::Triangle);
        assert!(triangle > 1e-2, "triangle gap {triangle}");
    }

    #[test]
    fn full_scale_returns_the_frame_untouched() {
        let source = plate(10, 7, 0.4);
        assert_eq!(resized(&source, 1.0), source);
        assert_eq!(resized(&source, f64::NAN), source, "a scale that is no number reads as no resize");
        // Twin: a real reduction does change the frame.
        assert_ne!(resized(&source, 0.5), source);
    }

    #[test]
    fn a_size_rounding_to_nothing_is_an_empty_frame() {
        let source = plate(837, 558, 0.4);
        for scale in [0.0, -0.5, 1e-9, 1e-12] {
            let out = resized(&source, scale);
            assert_eq!((out.width, out.height, out.pixels.len()), (0, 0, 0), "scale {scale}");
        }
        // Twin: the smallest scale that still rounds one row keeps a frame of 2 × 1.
        let thin = resized(&source, 1.0 / 558.0);
        assert_eq!((thin.width, thin.height), (2, 1));
        assert!(thin.pixels.chunks(4).all(|p| (p[0] - 0.4).abs() < 1e-5));
    }
}
