//! Core Image's area average, maximum and histogram, over a rectangle.

use super::*;

/// `CIAreaHistogram`: `count` bins over 0…1, each the share of the pixels it holds times `scale`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AreaHistogram {
    pub count: usize,
    pub scale: f32,
}

/// A rectangle of whole pixels in the buffer's own frame, top row first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

fn area(image: &Image, rect: Rect) -> impl Iterator<Item = V4> + '_ {
    (rect.y..rect.y + rect.height).flat_map(move |y| (rect.x..rect.x + rect.width).map(move |x| image.at(x, y)))
}

/// `CIAreaAverage`, summed in double precision.
pub fn area_average(image: &Image, rect: Rect) -> V4 {
    let mut sum = [0f64; 4];
    for p in area(image, rect) {
        for c in 0..4 {
            sum[c] += p[c] as f64;
        }
    }
    let n = (rect.width * rect.height) as f64;
    sum.map(|v| (v / n) as f32)
}

pub fn area_maximum(image: &Image, rect: Rect) -> V4 {
    area(image, rect).fold([f32::NEG_INFINITY; 4], |m, p| std::array::from_fn(|c| m[c].max(p[c])))
}

/// `CIAreaHistogram`'s bin for `v`: `floor(v * bins)`, held on the first and the last.
pub fn histogram_bin(v: f32, bins: usize) -> usize {
    clamp((v * bins as f32).floor(), 0.0, (bins - 1) as f32) as usize
}

/// The whole-number counts behind `area_histogram`.
pub fn histogram_counts(image: &Image, rect: Rect, bins: usize) -> Vec<[u64; 4]> {
    let mut counts = vec![[0u64; 4]; bins];
    for p in area(image, rect) {
        for c in 0..4 {
            counts[histogram_bin(p[c], bins)][c] += 1;
        }
    }
    counts
}

pub fn area_histogram(image: &Image, rect: Rect, k: &AreaHistogram) -> Vec<V4> {
    let n = (rect.width * rect.height) as f64;
    histogram_counts(image, rect, k.count)
        .into_iter()
        .map(|c| c.map(|v| (v as f64 / n * k.scale as f64) as f32))
        .collect()
}

const AREA_ROWS: Shader = shader!("area_rows");

const AREA_TOTAL: Shader = shader!("area_total");

const HISTOGRAM_COUNT: Shader = shader!("histogram_count");

const HISTOGRAM_SHARE: Shader = shader!("histogram_share");

impl Gpu {
    /// `CIAreaAverage` and `CIAreaMaximum` over `rect`, whole pixels in the buffer's frame.
    pub fn area_average_and_maximum(&self, image: &Image, rect: Rect) -> ([f32; 4], [f32; 4]) {
        check_rect(image, rect);
        let source = self.upload(&image.pixels);
        let partial = self.blank(2 * rect.height);
        let block = uniform(head(rect.height, image.width, 0, 0), &[whole([rect.x, rect.y, rect.width, rect.height])]);
        self.pass(AREA_ROWS, &block, &[&source, &partial], rect.height);
        let output = self.blank(2);
        self.pass(AREA_TOTAL, &uniform(head(rect.height, rect.width * rect.height, 0, 0), &[]), &[&partial, &output], 1);
        let out = self.download(&output, 2);
        (out[0], out[1])
    }

    pub fn area_average(&self, image: &Image, rect: Rect) -> [f32; 4] {
        self.area_average_and_maximum(image, rect).0
    }

    pub fn area_maximum(&self, image: &Image, rect: Rect) -> [f32; 4] {
        self.area_average_and_maximum(image, rect).1
    }

    /// `CIAreaHistogram` over `rect`: `k.count` texels, each channel's share of the pixels.
    pub fn area_histogram(&self, image: &Image, rect: Rect, k: &AreaHistogram) -> Vec<[f32; 4]> {
        check_rect(image, rect);
        let n = rect.width * rect.height;
        let source = self.upload(&image.pixels);
        let counts = self.blank(k.count);
        let block = uniform(head(n, image.width, k.count, 0), &[whole([rect.x, rect.y, rect.width, rect.height])]);
        self.pass(HISTOGRAM_COUNT, &block, &[&source, &counts], n);
        let output = self.blank(k.count);
        self.pass(HISTOGRAM_SHARE, &uniform(head(k.count, n, 0, 0), &[[k.scale, 0.0, 0.0, 0.0]]), &[&counts, &output], k.count);
        self.download(&output, k.count)
    }
}
