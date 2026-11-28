//! The tone match as it read the wide ring, its source's hole left in: the twin an inquiry on real frames sets
//! against the patch `generate` makes.

use super::*;

/// The patch biased by the wide ring's field, the hole left out of the destination alone.
pub(crate) fn on_ring(src: &Region, c: &Correction, width: f64, height: f64) -> Patch {
    let Some(f) = frame(c, width, height) else { return generate(src, c, width, height) };
    let ring = context_ring(f.centre, f.hole, f.extent);
    copied(src, c, width, height, |band, source, hole| {
        let (dx, dy) = (source.x - band.x, source.y - band.y);
        let shifted = |r: Rect| Rect { x: r.x + dx, y: r.y + dy, ..r };
        let left = |r: Rect| Rect { w: r.w / 2.0, ..r };
        let right = |r: Rect| Rect { x: r.x + r.w / 2.0, w: r.w / 2.0, ..r };
        let bottom = |r: Rect| Rect { h: r.h / 2.0, ..r };
        let top = |r: Rect| Rect { y: r.y + r.h / 2.0, h: r.h / 2.0, ..r };
        let diff = |part: fn(Rect) -> Rect| -> Option<[f32; 3]> {
            let (d, s) = (masked_mean(src, part(ring), hole)?, src.mean(shifted(part(ring)))?);
            Some(std::array::from_fn(|k| d[k] - s[k]))
        };
        let (Some(base), Some(l), Some(r), Some(b), Some(t)) = (diff(|r| r), diff(left), diff(right), diff(bottom), diff(top)) else { return ToneField::default() };
        let (sx, sy) = ((ring.w.max(1.0) / 2.0) as f32, (ring.h.max(1.0) / 2.0) as f32);
        ToneField { base, grad_x: std::array::from_fn(|k| (r[k] - l[k]) / sx), grad_y: std::array::from_fn(|k| (t[k] - b[k]) / sy) }
    })
}
