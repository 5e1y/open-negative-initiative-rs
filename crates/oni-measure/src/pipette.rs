//! `Pipette`: the disc under the pointer, aggregated on the axis the levels are graduated in.

use crate::plane::Plane;
use oni_settings::cg::{Point, Rect};

/// Resting radius in pixels of the measuring copy, so the reading follows neither zoom nor crop.
pub const DEFAULT_RADIUS: f64 = 4.0;

/// What the slider may dial; the track brackets what a photograph asks for.
pub const RADIUS_RANGE: (f64, f64) = (2.0, 24.0);

/// A stored preference held to the track; an unreadable one reads as the rest.
pub fn radius(stored: f64) -> f64 {
    if stored.is_finite() { stored.clamp(RADIUS_RANGE.0, RADIUS_RANGE.1) } else { DEFAULT_RADIUS }
}

/// The three fractions of the window the disc holds, and how many pixels it held.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reading {
    pub fractions: [f32; 3],
    pub pixels: usize,
}

/// The disc centred on `unit`, a point of the image with y downward; `None` outside the frame.
pub fn read(graduated: &Plane, unit: Point, radius: f64) -> Option<Reading> {
    let extent = graduated.extent();
    if extent.width < 1.0 || extent.height < 1.0 || radius < 0.5 || !(0.0..=1.0).contains(&unit.x) || !(0.0..=1.0).contains(&unit.y) {
        return None;
    }
    // Core Image counts from the bottom, a pointer from the top.
    let centre = Point::new(extent.x + unit.x * extent.width, extent.y + extent.height - unit.y * extent.height);
    let side = (2.0 * radius).ceil() + 1.0;
    let box_ = crate::window::intersection(&Rect::new((centre.x - radius).floor(), (centre.y - radius).floor(), side, side), &extent);
    if box_.width < 1.0 || box_.height < 1.0 {
        return None;
    }
    let (w, h, pixels) = graduated.bitmap(&box_);
    // Row 0 is the box's top, so the centre is measured from its top: off the bottom the disc slides on an even height.
    let cx = (centre.x - box_.x) as f32 - 0.5;
    let cy = (box_.y + box_.height - centre.y) as f32 - 0.5;
    let r2 = (radius * radius) as f32;
    let mut samples = Vec::with_capacity(w * h);
    for row in 0..h {
        let dy = row as f32 - cy;
        for column in 0..w {
            let dx = column as f32 - cx;
            if dx * dx + dy * dy <= r2 {
                let p = pixels[row * w + column];
                samples.push([p[0], p[1], p[2]]);
            }
        }
    }
    let fractions = mean(&samples)?;
    Some(Reading { fractions, pixels: samples.len() })
}

/// The mean on the graduated axis, affine in density: log-normal grain's brightest grains drag no
/// density mean. Only the answer is bounded: bounding each sample would lift a channel under zero.
fn mean(samples: &[[f32; 3]]) -> Option<[f32; 3]> {
    if samples.is_empty() {
        return None;
    }
    let n = samples.len() as f32;
    Some([0, 1, 2].map(|c| (samples.iter().fold(0.0f32, |a, s| a + s[c]) / n).max(0.0).min(1.0)))
}

/// The integer offsets a disc of `radius` holds about a pixel centre.
pub fn disc_offsets(radius: f64) -> Vec<(i64, i64)> {
    let reach = radius.floor() as i64;
    let r2 = (radius * radius) as f32;
    let mut out = Vec::new();
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            if ((dx * dx + dy * dy) as f32) <= r2 {
                out.push((dx, dy));
            }
        }
    }
    out
}
