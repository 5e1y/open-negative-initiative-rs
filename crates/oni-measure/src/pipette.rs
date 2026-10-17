//! `Pipette`: the disc under the pointer, aggregated on the axis the levels are graduated in.

use crate::plane::Plane;
use oni_settings::axis::{graduation, pipeline};
use oni_settings::cg::{Point, Rect};
use oni_settings::ConversionMode;

/// Resting radius in pixels of the measuring copy, so the reading follows neither zoom nor crop.
pub const DEFAULT_RADIUS: f64 = 4.0;

/// What the slider may dial; the track brackets what a photograph asks for.
pub const RADIUS_RANGE: (f64, f64) = (2.0, 24.0);

/// Where the samples are averaged; the two differ on the mean alone, a median commuting with the axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    /// The chain's axis: the geometric mean of the transmittances.
    Density,
    /// Their arithmetic mean, which a sensor integrates.
    Transmittance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Statistic {
    Mean,
    Median,
}

/// Silver grain is log-normal: the axis the sliders are linear on is the one its brightest grains do not drag.
pub const RETAINED_SPACE: Space = Space::Density;

/// The disc is drawn before the click, so a speck is in plain sight and every pixel stays counted.
pub const RETAINED_STATISTIC: Statistic = Statistic::Mean;

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
pub fn read(graduated: &Plane, unit: Point, mode: ConversionMode, radius: f64, space: Space, statistic: Statistic) -> Option<Reading> {
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
    let fractions = aggregate(&samples, mode, space, statistic)?;
    Some(Reading { fractions, pixels: samples.len() })
}

/// Only the answer is bounded onto the window: bounding each sample would drag the mean up out of a channel under zero.
pub fn aggregate(samples: &[[f32; 3]], mode: ConversionMode, space: Space, statistic: Statistic) -> Option<[f32; 3]> {
    if samples.is_empty() {
        return None;
    }
    let mut out = [0.0f32; 3];
    for (c, slot) in out.iter_mut().enumerate() {
        let column: Vec<f32> = samples.iter().map(|s| s[c]).collect();
        let summarised = match (statistic, space) {
            (Statistic::Median, _) => middle(&column),
            // The graduated signal is affine in density, so its mean is the density's.
            (Statistic::Mean, Space::Density) => column.iter().fold(0.0f32, |a, v| a + v) / column.len() as f32,
            (Statistic::Mean, Space::Transmittance) => {
                let mean = column.iter().fold(0.0f32, |a, &v| a + 10f32.powf(-density(v, mode))) / column.len() as f32;
                fraction(-mean.max(f32::MIN_POSITIVE).log10(), mode)
            }
        };
        *slot = summarised.max(0.0).min(1.0);
    }
    Some(out)
}

/// The lower middle on an even count, a value the film really holds.
pub fn middle(column: &[f32]) -> f32 {
    let mut sorted = column.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    sorted[(sorted.len() - 1) / 2]
}

/// The density behind a fraction of the window: the positive branch re-inverts on `DMAX`.
pub fn density(fraction: f32, mode: ConversionMode) -> f32 {
    let value = graduation::value(fraction, mode);
    if mode.invert_flag() > 0.5 { value } else { pipeline::DMAX - value }
}

pub fn fraction(density: f32, mode: ConversionMode) -> f32 {
    graduation::fraction(if mode.invert_flag() > 0.5 { density } else { pipeline::DMAX - density }, mode)
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
