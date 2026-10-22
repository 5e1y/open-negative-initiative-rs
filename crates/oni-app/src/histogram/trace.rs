//! `HistogramTraces`: the luminance as a grey backdrop, then red, green and blue added over it, each
//! coarsened to one column per point of the plot's width and drawn at its size in pixels.

use oni_measure::histogram::Histogram;

/// `HistogramScale.exponent`: low counts brought back up without a log's empty-bin lift.
const EXPONENT: f32 = 0.42;
/// SwiftUI's `.red`, `.green` and `.blue` in the dark appearance.
const CHANNELS: [[f32; 3]; 3] = [[1.0, 0.271, 0.227], [0.196, 0.843, 0.294], [0.039, 0.518, 1.0]];

/// Where a plot is drawn: its width in points, which sets its columns, and its size in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotSize {
    pub points: f32,
    pub width: usize,
    pub height: usize,
    /// `DS.Stroke.line` in pixels.
    pub line: f32,
}

impl PlotSize {
    /// A plot `w` by `h` points on a screen of `scale` pixels per point.
    pub fn of(w: f32, h: f32, scale: f32) -> PlotSize {
        let px = |v: f32| (v * scale).round().max(1.0) as usize;
        PlotSize { points: w.max(1.0), width: px(w), height: px(h), line: scale.max(1.0) }
    }
}

/// `coarsen`: neighbouring bins summed until there are no more columns than `target`.
pub fn coarsened(values: &[f32], target: usize) -> Vec<f32> {
    if values.len() <= target || target == 0 {
        return values.to_vec();
    }
    let group = values.len().div_ceil(target);
    values.chunks(group).map(|g| g.iter().sum()).collect()
}

/// `HistogramScale.heights`: 0 to 1, the peak sought off the two bins at each end.
pub fn heights(counts: &[f32]) -> Vec<f32> {
    let n = counts.len();
    let interior = if n > 4 { &counts[2..n - 2] } else { counts };
    let peak = interior.iter().copied().fold(0.0, f32::max);
    if peak <= 0.0 {
        return vec![0.0; n];
    }
    counts.iter().map(|c| (c.max(0.0) / peak).powf(EXPONENT).min(1.0)).collect()
}

/// One trace's coverage per pixel into `out`: the fill under the curve and the line along it, each 0 to 1.
fn coverage(out: &mut Vec<(f32, f32)>, heights: &[f32], size: PlotSize) {
    let (w, h) = (size.width, size.height);
    out.clear();
    out.resize(w * h, (0.0, 0.0));
    if heights.len() < 2 {
        return;
    }
    let last = (heights.len() - 1) as f32;
    let y_at = |x: f32| {
        let t = (x / w as f32 * last).clamp(0.0, last);
        let (i, f) = (t.floor() as usize, t.fract());
        let v = heights[i] + (heights[(i + 1).min(heights.len() - 1)] - heights[i]) * f;
        h as f32 * (1.0 - v)
    };
    for x in 0..w {
        let (a, b, centre) = (y_at(x as f32), y_at(x as f32 + 1.0), y_at(x as f32 + 0.5));
        let (top, bottom) = (a.min(b) - size.line / 2.0, a.max(b) + size.line / 2.0);
        for y in 0..h {
            let (y0, y1) = (y as f32, y as f32 + 1.0);
            let fill = (y1 - centre.max(y0)).clamp(0.0, 1.0);
            let line = (y1.min(bottom) - y0.max(top)).clamp(0.0, 1.0);
            out[y * w + x] = (fill, line);
        }
    }
}

/// The plot, premultiplied RGBA8 of `size`: a channel out of `focus` drawn at a fifth.
pub fn raster(histogram: &Histogram, focus: Option<usize>, size: PlotSize) -> Vec<u8> {
    Tracer::default().raster(histogram, focus, size).to_vec()
}

/// A tracer's working buffers, kept from one trace to the next: a main plot's are 30 MB at 2x.
#[derive(Default)]
pub struct Tracer {
    out: Vec<[f32; 4]>,
    added: Vec<[f32; 4]>,
    cover: Vec<(f32, f32)>,
    bytes: Vec<u8>,
}

impl Tracer {
    /// `raster` into the kept buffers, each cleared first.
    pub fn raster(&mut self, histogram: &Histogram, focus: Option<usize>, size: PlotSize) -> &mut [u8] {
        let n = size.width * size.height;
        for layer in [&mut self.out, &mut self.added] {
            layer.clear();
            layer.resize(n, [0.0; 4]);
        }
        self.compose(histogram, focus, size)
    }

    /// The layers drawn over what the buffers hold, then flattened to bytes.
    fn compose(&mut self, histogram: &Histogram, focus: Option<usize>, size: PlotSize) -> &mut [u8] {
        let columns = size.points as usize;
        let trace = |cover: &mut Vec<(f32, f32)>, counts: &[f32]| coverage(cover, &heights(&coarsened(counts, columns)), size);
        // The luminance first and outside the added layer, a backdrop free of any colour cast.
        if histogram.luma.iter().any(|v| *v > 0.0) {
            trace(&mut self.cover, &histogram.luma);
            for (px, &(fill, line)) in self.out.iter_mut().zip(&self.cover) {
                let a = line * 0.45 + fill * 0.12 * (1.0 - line * 0.45);
                *px = [a, a, a, a];
            }
        }
        for (c, colour) in CHANNELS.iter().enumerate() {
            let counts: Vec<f32> = histogram.rgb.iter().map(|b| b[c]).collect();
            if !counts.iter().any(|v| *v > 0.0) {
                continue;
            }
            let strength = if focus.is_some_and(|f| f != c) { 0.2 } else { 1.0 };
            trace(&mut self.cover, &counts);
            for (px, &(fill, line)) in self.added.iter_mut().zip(&self.cover) {
                let (l, f) = (line * 0.85 * strength, fill * 0.28 * strength);
                let a = l + f * (1.0 - l);
                // `plusLighter`: premultiplied colours summed, held at one.
                for k in 0..3 {
                    px[k] = (px[k] + colour[k] * a).min(1.0);
                }
                px[3] = (px[3] + a).min(1.0);
            }
        }
        self.bytes.clear();
        for (px, top) in self.out.iter().zip(&self.added) {
            self.bytes.extend((0..4).map(|k| ((top[k] + px[k] * (1.0 - top[3])).clamp(0.0, 1.0) * 255.0).round() as u8));
        }
        &mut self.bytes
    }
}

/// Premultiplied RGBA8 made straight, as a texture the interface samples is read.
pub fn straighten(bytes: &mut [u8]) {
    for px in bytes.chunks_exact_mut(4) {
        let a = u16::from(px[3]);
        if a > 0 {
            for v in &mut px[..3] {
                *v = ((u16::from(*v) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plot_has_a_column_per_point_and_its_peak_ignores_the_clipped_ends() {
        assert_eq!(coarsened(&[1.0; 1024], 300).len(), 256, "groups of four: no more columns than points");
        assert_eq!(coarsened(&[1.0; 200], 300).len(), 200, "fewer bins than points stay as they are");
        // A clipped end holds a thousand times the rest: the interior still reaches the top.
        let mut counts = vec![1.0; 16];
        counts[0] = 1000.0;
        let h = heights(&counts);
        assert_eq!((h[0], h[8]), (1.0, 1.0));
        // Twin: a peak taken over the ends would flatten the interior to 0.055.
        assert!((1.0f32 / 1000.0).powf(EXPONENT) < 0.06);
    }

    #[test]
    fn a_plot_is_drawn_at_its_size_with_the_luminance_under_the_channels() {
        let mut h = Histogram { rgb: vec![[0.0; 3]; 1024], luma: vec![0.0; 1024], ends: None };
        for b in 300..700 {
            h.rgb[b] = [5.0, 0.0, 0.0];
            h.luma[b] = 5.0;
        }
        let size = PlotSize::of(280.0, 120.0, 2.0);
        let px = raster(&h, None, size);
        assert_eq!(px.len(), 560 * 240 * 4);
        let at = |x: usize, y: usize| &px[(y * 560 + x) * 4..(y * 560 + x) * 4 + 4];
        let inside = at(280, 230);
        assert!(inside[0] > inside[1] && inside[1] > 0, "red over the grey backdrop: {inside:?}");
        // Twin: outside the counted range nothing is drawn.
        assert_eq!(at(20, 230), [0, 0, 0, 0]);
    }

    #[test]
    fn a_channel_out_of_focus_is_drawn_at_a_fifth_and_an_empty_reading_draws_nothing() {
        let rgb: Vec<[f32; 3]> = (0..64).map(|i| [i as f32, 32.0, (64 - i) as f32]).collect();
        let h = Histogram { rgb, luma: vec![0.0; 64], ends: None };
        let size = PlotSize::of(64.0, 20.0, 1.0);
        let (all, green) = (raster(&h, None, size), raster(&h, Some(1), size));
        let at = |v: &[u8], x: usize, y: usize, c: usize| v[(y * 64 + x) * 4 + c];
        assert!(at(&green, 40, 19, 0) < at(&all, 40, 19, 0) && at(&green, 40, 19, 1) >= at(&all, 40, 19, 1) / 2);
        // Twin: an empty reading draws nothing at all.
        assert!(raster(&Histogram::default(), None, PlotSize::of(8.0, 8.0, 1.0)).iter().all(|&b| b == 0));
    }

    #[test]
    fn a_tracer_kept_across_traces_draws_the_bytes_a_fresh_one_draws() {
        let ramp = |k: f32| Histogram { rgb: (0..256).map(|i| [i as f32 * k, 256.0 - i as f32, 30.0]).collect(), luma: vec![k; 256], ends: None };
        let (big, small) = (PlotSize::of(120.0, 60.0, 2.0), PlotSize::of(64.0, 30.0, 1.0));
        let mut kept = Tracer::default();
        kept.raster(&ramp(3.0), None, big);
        assert_eq!(kept.raster(&ramp(1.0), Some(2), small).to_vec(), raster(&ramp(1.0), Some(2), small));
        assert_eq!(kept.raster(&ramp(2.0), None, big).to_vec(), raster(&ramp(2.0), None, big));
        // Twin: layers not cleared keep the last trace under the next one.
        kept.raster(&ramp(3.0), None, small);
        assert_ne!(kept.compose(&ramp(1.0), Some(2), small).to_vec(), raster(&ramp(1.0), Some(2), small));
    }

    #[test]
    fn a_straight_texel_is_its_premultiplied_colour_over_its_alpha() {
        let mut px = [64, 32, 0, 128, 100, 20, 30, 200, 10, 20, 30, 0];
        straighten(&mut px);
        assert_eq!(px, [128, 64, 0, 128, 128, 26, 38, 200, 10, 20, 30, 0]);
        // Twin: a truncating division would land 100 over 200 on 127, which the bytes above refuse.
        assert_eq!(100 * 255 / 200, 127);
    }
}
