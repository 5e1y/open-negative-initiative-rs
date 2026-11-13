//! The detail the strip shows: the frame's part, its zoom, and the one holding the most specks whole
//! and apart.

use super::*;

/// A mark's number badge, in points, as `CorrectionMark` lays it off its box's corner, and the gap
/// kept between two specks' rings and marks so no number hides another.
pub(super) const BADGE: (f32, f32) = (30.0, 28.0);
pub(super) const GAP: f32 = 4.0;
/// How far inside the strip a speck's ring and mark stay, in points: none at an edge.
pub(super) const MARGIN: f32 = 16.0;

/// The part of the frame the strip shows: its corner on the framed frame in pixels, the zoom in the
/// strip's pixels a frame pixel, and the strip's size in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub x: u32,
    pub y: u32,
    pub zoom: f32,
    pub width: u32,
    pub height: u32,
}

impl View {
    /// The frame's pixels it covers.
    pub(super) fn region(&self) -> (u32, u32) {
        ((self.width as f32 / self.zoom).ceil() as u32, (self.height as f32 / self.zoom).ceil() as u32)
    }

    /// The canvas's picture frame in the strip's points: where the whole framed frame would lie.
    pub(super) fn frame(&self, size: (u32, u32)) -> (f64, f64, f64, f64) {
        let k = f64::from(self.zoom / SCALE);
        (-f64::from(self.x) * k, -f64::from(self.y) * k, f64::from(size.0) * k, f64::from(size.1) * k)
    }

    /// A box on the frame in pixels, in the strip's points.
    pub(super) fn points(&self, b: [f64; 4], size: (u32, u32)) -> [f64; 4] {
        let f = self.frame(size);
        let (sx, sy) = (f.2 / f64::from(size.0), f.3 / f64::from(size.1));
        [f.0 + b[0] * sx, f.1 + b[1] * sy, b[2] * sx, b[3] * sy]
    }

    /// A speck's reading ring, as the canvas lays it: its box, the diameter at least `DustStyle.ring`.
    pub(super) fn ring(&self, f: &Found, size: (u32, u32)) -> [f64; 4] {
        let b = f.bounds.map(f64::from);
        self.points([b[0], b[1], b[2] - b[0], b[3] - b[1]], size)
    }

    /// What a speck lays on the strip at any moment, its ring and its mark with the number: a box in points.
    pub(super) fn footprint(&self, f: &Found, size: (u32, u32), least_ring: f64) -> [f64; 4] {
        let r = self.ring(f, size);
        let d = r[2].max(r[3]).max(least_ring);
        let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
        let m = self.points(f.mark(size), size);
        let badge = [m[0] + m[2], m[1] - f64::from(BADGE.1), f64::from(BADGE.0), f64::from(BADGE.1)];
        let (x0, y0) = ((cx - d / 2.0).min(m[0]).min(badge[0]), (cy - d / 2.0).min(m[1]).min(badge[1]));
        let (x1, y1) = ((cx + d / 2.0).max(m[0] + m[2]).max(badge[0] + badge[2]), (cy + d / 2.0).max(m[1] + m[3]).max(badge[1] + badge[3]));
        [x0, y0, x1 - x0, y1 - y0]
    }
}

/// The view at `zoom` over the frame `size` that shows the most of `found` whole and apart: every speck it
/// touches lies `MARGIN` inside the strip, and no two footprints come within `GAP`; the middlemost on a tie.
pub fn framing(found: &[Found], size: (u32, u32), strip: (u32, u32), zoom: f32, least_ring: f64) -> Option<(View, usize)> {
    let blank = View { x: 0, y: 0, zoom, width: strip.0, height: strip.1 };
    let (rw, rh) = blank.region();
    if rw > size.0 || rh > size.1 {
        return None;
    }
    let (pw, ph) = (f64::from(strip.0) / f64::from(SCALE), f64::from(strip.1) / f64::from(SCALE));
    let step = 16;
    let mut best: Option<(View, usize, f64)> = None;
    for y in (0..=size.1 - rh).step_by(step) {
        for x in (0..=size.0 - rw).step_by(step) {
            let view = View { x, y, ..blank };
            let prints: Vec<[f64; 4]> = found.iter().map(|f| view.footprint(f, size, least_ring)).filter(|b| b[0] + b[2] > 0.0 && b[1] + b[3] > 0.0 && b[0] < pw && b[1] < ph).collect();
            let m = f64::from(MARGIN);
            if prints.is_empty() || prints.iter().any(|b| b[0] < m || b[1] < m || b[0] + b[2] > pw - m || b[1] + b[3] > ph - m) {
                continue;
            }
            let g = f64::from(GAP);
            let clear = |a: &[f64; 4], b: &[f64; 4]| a[0] + a[2] + g <= b[0] || b[0] + b[2] + g <= a[0] || a[1] + a[3] + g <= b[1] || b[1] + b[3] + g <= a[1];
            let apart = prints.iter().enumerate().all(|(i, a)| prints[i + 1..].iter().all(|b| clear(a, b)));
            if !apart {
                continue;
            }
            let n = prints.len();
            let (cx, cy) = prints.iter().fold((0.0, 0.0), |(x, y), b| (x + b[0] + b[2] / 2.0, y + b[1] + b[3] / 2.0));
            let off = (cx / n as f64 - pw / 2.0).hypot(cy / n as f64 - ph / 2.0);
            if best.as_ref().is_none_or(|(_, bn, boff)| n > *bn || (n == *bn && off < *boff)) {
                best = Some((view, n, off));
            }
        }
    }
    best.map(|(v, n, _)| (v, n))
}

/// The view at `zoom` holding the most specks, about the one with most neighbours, for a probe's look
/// when no detail holds them apart.
pub(super) fn busiest(found: &[Found], size: (u32, u32), strip: (u32, u32), zoom: f32) -> Option<(View, usize)> {
    let blank = View { x: 0, y: 0, zoom, width: strip.0, height: strip.1 };
    let (rw, rh) = blank.region();
    if rw > size.0 || rh > size.1 {
        return None;
    }
    found
        .iter()
        .map(|f| {
            let (cx, cy) = ((f.bounds[0] + f.bounds[2]) / 2, (f.bounds[1] + f.bounds[3]) / 2);
            let view = View { x: cx.saturating_sub(rw / 2).min(size.0 - rw), y: cy.saturating_sub(rh / 2).min(size.1 - rh), ..blank };
            let inside = |g: &Found| g.bounds[0] >= view.x && g.bounds[2] <= view.x + rw && g.bounds[1] >= view.y && g.bounds[3] <= view.y + rh;
            (view, found.iter().filter(|g| inside(g)).count())
        })
        .max_by_key(|(_, n)| *n)
}
