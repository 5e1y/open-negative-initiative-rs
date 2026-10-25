//! SwiftUI's `RoundedRectangle`: circular corners, or `.continuous` ones drawn with UIKit's curve
//! of three cubics, which reaches 1.528665 radii along each edge.

use tiny_skia::{Path, PathBuilder};

/// How far the continuous corner reaches along an edge, in radii.
const REACH: f32 = 1.528_664_8;

/// The three cubics of one continuous corner, in radii from the corner along (edge, other edge):
/// start, then (control 1, control 2, end) each.
const CORNER: [[(f32, f32); 3]; 3] = [
    [(1.088_493_2, 0.0), (0.868_406_9, 0.020_655_46), (0.669_934_3, 0.065_496)],
    [(0.372_823_9, 0.162_329_1), (0.162_329_1, 0.372_823_9), (0.065_496, 0.669_934_3)],
    [(0.020_655_46, 0.868_406_9), (0.0, 1.088_493_2), (0.0, REACH)],
];

/// A rounded rectangle; `None` for an empty one.
pub fn rounded(x: f32, y: f32, w: f32, h: f32, radius: f32, continuous: bool) -> Option<Path> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let mut pb = PathBuilder::new();
    let r = radius.max(0.0);
    if r <= 0.0 {
        pb.push_rect(tiny_skia::Rect::from_xywh(x, y, w, h)?);
        return pb.finish();
    }
    if continuous {
        // UIKit's limit: past it the corner shrinks rather than overlap its neighbour.
        let r = r.min(w.min(h) / 2.0 / REACH);
        // Each corner as (corner point, unit vector along the incoming edge back to it, along the outgoing).
        let corners = [
            ((x + w, y), (-1.0, 0.0), (0.0, 1.0)),
            ((x + w, y + h), (0.0, -1.0), (-1.0, 0.0)),
            ((x, y + h), (1.0, 0.0), (0.0, -1.0)),
            ((x, y), (0.0, 1.0), (1.0, 0.0)),
        ];
        pb.move_to(x + REACH * r, y);
        for ((cx, cy), (ax, ay), (bx, by)) in corners {
            let p = |(u, v): (f32, f32)| (cx + (ax * u + bx * v) * r, cy + (ay * u + by * v) * r);
            let start = p((REACH, 0.0));
            pb.line_to(start.0, start.1);
            for [c1, c2, end] in CORNER {
                let (c1, c2, end) = (p(c1), p(c2), p(end));
                pb.cubic_to(c1.0, c1.1, c2.0, c2.1, end.0, end.1);
            }
        }
        pb.close();
        return pb.finish();
    }
    let r = r.min(w.min(h) / 2.0);
    // A quarter circle as one cubic, the standard 0.5523 handle.
    let k = r * (1.0 - 0.552_284_8);
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - k, y, x + w, y + k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - k, x + w - k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + k, y + h, x, y + h - k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + k, x + k, y, x + r, y);
    pb.close();
    pb.finish()
}
