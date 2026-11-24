//! Film grain: its spread, its coarseness, its strength by the tone the picture shows and its colour.

use super::GrainColour;
use crate::cg::Point;
use crate::curves::{Curve, Evaluated, spaced};
use crate::tolerant;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// Film grain on a digital photo's developed film, uniform in its density: its spread, a share of its track, and its
/// size, a grain's width on the film in micrometres, so every resolution lays it alike on the picture; its strength by
/// tone and its colour along it; either at rest is not written, so a new rest reaches every frame left on it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Grain {
    pub amount: f32,
    pub size: f32,
    #[serde(skip_serializing_if = "GrainCurve::rests")]
    pub curve: GrainCurve,
    #[serde(skip_serializing_if = "GrainColour::rests")]
    pub colour: GrainColour,
}

impl Default for Grain {
    fn default() -> Self {
        Grain { amount: 0.0, size: Grain::SIZE_REST, curve: GrainCurve::REST, colour: GrainColour::REST }
    }
}

impl Grain {
    pub const AMOUNT_RANGE: (f32, f32) = (0.0, 1.0);
    /// Film grain's 10 to 30 µm (Vitale, "Film Grain, Resolution and Fundamental Film Particles", 2010, table 1),
    /// to twice its coarsest.
    pub const SIZE_RANGE: (f32, f32) = (10.0, 60.0);
    /// The owner's 40% of the former track on a landscape scan, one for every stock (no stock's own is published): film
    /// grain's coarse end, an ISO 800 stock's.
    pub const SIZE_REST: f32 = 27.6;
    /// The Grain a digital photo opens with (owner's call).
    pub const DIGITAL_AMOUNT: f32 = 0.25;
    /// The film frame's long side, in micrometres, which the scan's long side stands for: a 135 frame's.
    pub const FRAME_LONG: f64 = 36_000.0;
    /// A grain's mean width along a line, in cells of the texture's lattice: its field's run between sign changes.
    pub const CLUMP_CELLS: f64 = 2.64;

    /// Size in pixels of a scan whose long side is `long` pixels.
    pub fn pixels(size: f32, long: f64) -> f64 {
        f64::from(size) * long / Self::FRAME_LONG
    }

    /// The micrometres `pixels` of a scan whose long side is `long` pixels stand for.
    pub fn microns(pixels: f64, long: f64) -> f32 {
        (pixels * Self::FRAME_LONG / long) as f32
    }

    /// The texture's lattice cell, in pixels of a scan whose long side is `long` pixels.
    pub fn cell(&self, long: f64) -> f64 {
        Self::pixels(self.size, long) / Self::CLUMP_CELLS
    }
}

/// The share of Grain laid at each tone of the picture, from its black point to its white: five points
/// on a natural spline, each y from none to the whole.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct GrainCurve {
    pub points: [Point; 5],
}

impl Default for GrainCurve {
    fn default() -> Self {
        Self::REST
    }
}

/// Five finite points from the black point to the white in x order, each y held to 0…1; anything else
/// reads as rest.
impl<'de> Deserialize<'de> for GrainCurve {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let map = tolerant::object(Value::deserialize(d)?, "GrainCurve")?;
        let Ok(Some(points)) = tolerant::opt::<Vec<Point>, D::Error>(&map, "points") else { return Ok(Self::REST) };
        let Ok(mut points) = <[Point; 5]>::try_from(points) else { return Ok(Self::REST) };
        let finite = points.iter().all(|p| p.x.is_finite() && p.y.is_finite());
        let ordered = points.windows(2).all(|w| w[0].x < w[1].x);
        if !finite || !ordered || points[0].x != 0.0 || points[4].x != 1.0 {
            return Ok(Self::REST);
        }
        for p in &mut points {
            p.y = p.y.clamp(0.0, 1.0);
        }
        Ok(GrainCurve { points })
    }
}

/// The curve's spline, read held to 0…1 as the render and the plot both lay it.
pub struct Strength(Evaluated);

impl Strength {
    pub fn at(&self, tone: f64) -> f64 {
        self.0.value(tone).clamp(0.0, 1.0)
    }
}

impl GrainCurve {
    /// The whole grain at every tone: laid on the negative, the inversion alone weakens it toward the white.
    pub const REST: GrainCurve = GrainCurve {
        points: [Point::new(0.0, 1.0), Point::new(0.25, 1.0), Point::new(0.5, 1.0), Point::new(0.75, 1.0), Point::new(1.0, 1.0)],
    };

    pub fn rests(&self) -> bool {
        *self == Self::REST
    }

    pub fn move_point(&mut self, index: usize, point: Point) {
        spaced(&mut self.points, index, point);
    }

    pub fn strength(&self) -> Strength {
        Strength(Curve { points: self.points.to_vec() }.evaluated())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(json: &str) -> GrainCurve {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn a_curve_of_another_count_or_order_reads_as_rest() {
        assert!(read(r#"{"points":[[0,0.2],[1,0.5]]}"#).rests());
        assert!(read(r#"{"points":[[0,0.2],[0.6,1],[0.5,1],[0.85,0],[1,0]]}"#).rests(), "crossed points");
        assert!(read(r#"{"points":"none"}"#).rests());
        // Twin: five points in order are kept as written.
        let kept = read(r#"{"points":[[0,0.2],[0.3,1],[0.5,0.9],[0.8,0],[1,0]]}"#);
        assert_eq!(kept.points[0], Point::new(0.0, 0.2));
        assert!(!kept.rests());
    }

    #[test]
    fn a_strength_past_its_track_lands_on_its_end() {
        let c = read(r#"{"points":[[0,-0.5],[0.3,1.7],[0.5,0.6],[0.8,0],[1,0]]}"#);
        assert_eq!((c.points[0].y, c.points[1].y), (0.0, 1.0));
        // Twin: an inner value is kept as written.
        assert_eq!(c.points[2].y, 0.6);
    }

    #[test]
    fn a_point_keeps_its_spacing_and_the_ends_their_tone() {
        let mut c = GrainCurve::REST;
        c.move_point(4, Point::new(0.2, 0.5));
        assert_eq!(c.points[4], Point::new(1.0, 0.5));
        c.move_point(2, Point::new(0.95, 1.4));
        assert_eq!(c.points[2], Point::new(0.75 - crate::curves::SPACING, 1.0));
        // Twin: a point set without the move crosses its neighbour.
        let mut raw = GrainCurve::REST;
        raw.points[2] = Point::new(0.95, 1.0);
        assert!(raw.points[2].x > raw.points[3].x);
    }

    #[test]
    fn the_rest_lays_the_whole_grain_at_every_tone() {
        let s = GrainCurve::REST.strength();
        assert!((0..=100).all(|i| s.at(f64::from(i) / 100.0) == 1.0));
        // Twin: a curve lowered at its white end lays less grain there.
        let mut lowered = GrainCurve::REST;
        lowered.points[4].y = 0.4;
        assert!(lowered.strength().at(1.0) < 1.0 && !lowered.rests());
    }
}
