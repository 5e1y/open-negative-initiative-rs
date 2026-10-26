//! The hand, drawn over each shot: a macOS-like arrow, black edged in white; pressed, it shrinks a
//! little over a pale disc, so a press reads without a word.

use crate::photo::Rgba;

/// The arrow's outline in points, its tip at the origin.
const ARROW: [(f32, f32); 7] = [(0.0, 0.0), (0.0, 16.0), (4.0, 12.4), (6.8, 18.6), (9.4, 17.4), (6.7, 11.4), (11.4, 11.4)];
/// The white edge around the arrow, in points.
const EDGE: f32 = 1.3;
/// The disc a press lays under the tip, its radius in points and its opacity.
const PRESS_RADIUS: f32 = 7.0;
const PRESS_ALPHA: f32 = 0.3;
const PRESSED_SCALE: f32 = 0.88;
/// Samples a pixel's side is cut into for its coverage.
const SAMPLES: usize = 4;

/// The hand at `at` points over `frame`, drawn at `scale` pixels a point.
pub fn draw(frame: &mut Rgba, at: (f32, f32), down: bool, scale: f32) {
    let tip = (at.0 * scale, at.1 * scale);
    if down {
        disc(frame, tip, PRESS_RADIUS * scale, PRESS_ALPHA);
    }
    let size = if down { PRESSED_SCALE } else { 1.0 } * scale;
    let shape: Vec<(f32, f32)> = ARROW.iter().map(|(x, y)| (tip.0 + x * size, tip.1 + y * size)).collect();
    let edge = EDGE * size;
    let (x0, y0) = ((tip.0 - edge).floor() as i64, (tip.1 - edge).floor() as i64);
    let (x1, y1) = ((tip.0 + 12.0 * size + edge).ceil() as i64, (tip.1 + 19.0 * size + edge).ceil() as i64);
    for py in y0.max(0)..y1.min(frame.height as i64) {
        for px in x0.max(0)..x1.min(frame.width as i64) {
            let (mut white, mut black) = (0usize, 0usize);
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let p = (px as f32 + (sx as f32 + 0.5) / SAMPLES as f32, py as f32 + (sy as f32 + 0.5) / SAMPLES as f32);
                    if inside(&shape, p) {
                        if distance(&shape, p) >= edge * 0.55 { black += 1 } else { white += 1 }
                    } else if distance(&shape, p) < edge * 0.45 {
                        white += 1;
                    }
                }
            }
            let n = (SAMPLES * SAMPLES) as f32;
            blend(frame, px as u32, py as u32, [255, 255, 255], white as f32 / n);
            blend(frame, px as u32, py as u32, [0, 0, 0], black as f32 / n);
        }
    }
}

fn disc(frame: &mut Rgba, centre: (f32, f32), radius: f32, alpha: f32) {
    let r = radius.ceil() as i64 + 1;
    for py in (centre.1 as i64 - r).max(0)..(centre.1 as i64 + r).min(frame.height as i64) {
        for px in (centre.0 as i64 - r).max(0)..(centre.0 as i64 + r).min(frame.width as i64) {
            let d = ((px as f32 + 0.5 - centre.0).powi(2) + (py as f32 + 0.5 - centre.1).powi(2)).sqrt();
            let cover = (radius - d + 0.5).clamp(0.0, 1.0);
            blend(frame, px as u32, py as u32, [255, 255, 255], cover * alpha);
        }
    }
}

fn blend(frame: &mut Rgba, x: u32, y: u32, ink: [u8; 3], alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    let at = ((y * frame.width + x) * 4) as usize;
    for c in 0..3 {
        let under = f32::from(frame.pixels[at + c]);
        frame.pixels[at + c] = (under + (f32::from(ink[c]) - under) * alpha).round() as u8;
    }
}

/// Even-odd: a ray to the right crosses the outline an odd number of times.
fn inside(shape: &[(f32, f32)], p: (f32, f32)) -> bool {
    let mut odd = false;
    for i in 0..shape.len() {
        let (a, b) = (shape[i], shape[(i + 1) % shape.len()]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0) {
            odd = !odd;
        }
    }
    odd
}

fn distance(shape: &[(f32, f32)], p: (f32, f32)) -> f32 {
    (0..shape.len())
        .map(|i| {
            let (a, b) = (shape[i], shape[(i + 1) % shape.len()]);
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            ((a.0 + t * dx - p.0).powi(2) + (a.1 + t * dy - p.1).powi(2)).sqrt()
        })
        .fold(f32::INFINITY, f32::min)
}
