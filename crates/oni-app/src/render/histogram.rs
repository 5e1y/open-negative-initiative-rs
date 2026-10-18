//! The picture encoded for the screen, and the histogram counted off it.

use super::*;

/// `display::encode` on every `step`-th row and column, its rows shared among the cores: the shown
/// bytes at a step of 1, a histogram's sample past it. A 2000 px preview costs 150 ms on one core.
pub fn encoded(linear: &Linear, k: &Params, step: usize) -> Vec<u8> {
    let (w, h) = (linear.width, linear.height);
    let rows: Vec<usize> = (0..h).step_by(step).collect();
    let row_bytes = w.div_ceil(step) * 4;
    let mut out = vec![0u8; rows.len() * row_bytes];
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let per = rows.len().div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        for (band, chunk) in rows.chunks(per).zip(out.chunks_mut(per * row_bytes)) {
            scope.spawn(move || {
                for (&y, line) in band.iter().zip(chunk.chunks_mut(row_bytes)) {
                    for (i, x) in (0..w).step_by(step).enumerate() {
                        let p = display::encode_pixel(linear.pixels[y * w + x], x as u32, y as u32, k);
                        line[i * 4..i * 4 + 4].copy_from_slice(&p);
                    }
                }
            });
        }
    });
    out
}

/// The shown image's three channels in 256 bins each.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Histogram {
    pub bins: Vec<[u32; 3]>,
}

impl Histogram {
    pub fn of(rgba8: &[u8]) -> Histogram {
        let mut bins = vec![[0u32; 3]; 256];
        for p in rgba8.chunks_exact(4) {
            for c in 0..3 {
                bins[p[c] as usize][c] += 1;
            }
        }
        Histogram { bins }
    }

    /// `DSHistogram`: each channel's bins as a filled curve, red, green and blue added where they
    /// overlap, the tallest bin at the top; premultiplied RGBA8 of `w` by `h`.
    pub fn raster(&self, w: usize, h: usize) -> Vec<u8> {
        let mut out = vec![0u8; w * h * 4];
        if self.bins.is_empty() || w == 0 || h == 0 {
            return out;
        }
        // The two ends hold every clipped pixel; scaling by them would flatten the rest.
        let top = self.bins[1..255].iter().flat_map(|b| b.iter().copied()).max().unwrap_or(1).max(1) as f32;
        for x in 0..w {
            let bin = (x * 256 / w).min(255);
            for c in 0..3 {
                let tall = ((self.bins[bin][c] as f32 / top).min(1.0) * h as f32).round() as usize;
                for y in h - tall..h {
                    let px = &mut out[(y * w + x) * 4..(y * w + x) * 4 + 4];
                    px[c] = px[c].saturating_add(150);
                    px[3] = px[3].max(150);
                }
            }
        }
        out
    }
}
