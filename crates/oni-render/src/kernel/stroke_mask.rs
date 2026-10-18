//! The render graph's hard mask of a stroke.

use super::*;

/// A stroke's hard mask: its points in the input row's first two channels, thickened by `radius`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeMask {
    pub radius: f32,
    pub count: usize,
}

impl StrokeMask {
    pub fn bytes(&self) -> Vec<u8> {
        bytemuck::bytes_of(&[self.radius, self.count as f32, 0.0, 0.0]).to_vec()
    }
}

/// The stroke through `points` thickened by `radius`, on `out`, (x, y, width, height).
pub fn stroke_mask(points: &[[f32; 2]], out: (i32, i32, usize, usize), radius: f32) -> Grid {
    let segment = |c: [f32; 2], a: [f32; 2], b: [f32; 2]| {
        let ab = [b[0] - a[0], b[1] - a[1]];
        let l = ab[0] * ab[0] + ab[1] * ab[1];
        let t = if l <= 0.0 { 0.0 } else { clamp(((c[0] - a[0]) * ab[0] + (c[1] - a[1]) * ab[1]) / l, 0.0, 1.0) };
        let d = [c[0] - (a[0] + t * ab[0]), c[1] - (a[1] + t * ab[1])];
        (d[0] * d[0] + d[1] * d[1]).sqrt()
    };
    Grid::generated(out.0, out.1, out.2, out.3, |p| {
        let c = [p[0] as f32 + 0.5, p[1] as f32 + 0.5];
        let first = points[0];
        let mut d = ((c[0] - first[0]).powi(2) + (c[1] - first[1]).powi(2)).sqrt();
        for pair in points.windows(2) {
            d = d.min(segment(c, pair[0], pair[1]));
        }
        // The grey bitmap's coverage linearised, gamma 2.2, as Core Image reads Swift's.
        let v = clamp(radius + 0.5 - d, 0.0, 1.0).powf(2.2);
        [v, v, v, 1.0]
    })
}
