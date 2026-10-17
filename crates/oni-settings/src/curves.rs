//! The curves drawn by hand, a natural cubic spline (C²), and the saturation-by-luminance curve.

use crate::cg::Point;
use crate::levels::LevelsChannel;
use crate::tolerant;
use crate::tone::{Print, contrast};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// A tone curve: control points joined by a natural cubic spline; overshoot is the accepted price.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Curve {
    pub points: Vec<Point>,
}

impl Default for Curve {
    fn default() -> Self {
        Curve { points: vec![Point::new(0.0, 0.0), Point::new(1.0, 1.0)] }
    }
}

/// A curve with its second derivatives solved once.
pub struct Evaluated {
    points: Vec<Point>,
    m: Vec<f64>,
}

impl Evaluated {
    pub fn value(&self, x: f64) -> f64 {
        let p = &self.points;
        if p.len() <= 1 {
            return x;
        }
        if x <= p[0].x {
            return p[0].y;
        }
        if x >= p[p.len() - 1].x {
            return p[p.len() - 1].y;
        }
        let Some(i) = (0..p.len() - 1).rev().find(|&i| p[i].x <= x) else {
            return p[0].y;
        };
        let (p0, p1) = (p[i], p[i + 1]);
        let h = p1.x - p0.x;
        if h <= 0.0 {
            return p0.y;
        }
        let m = &self.m;
        let t = x - p0.x;
        let b = (p1.y - p0.y) / h - h * (2.0 * m[i] + m[i + 1]) / 6.0;
        p0.y + b * t + m[i] / 2.0 * t * t + (m[i + 1] - m[i]) / (6.0 * h) * t * t * t
    }
}

impl Curve {
    pub fn is_neutral(&self) -> bool {
        *self == Curve::default()
    }

    pub fn evaluated(&self) -> Evaluated {
        Evaluated { points: self.points.clone(), m: self.second_derivatives() }
    }

    /// Thomas sweep over the tridiagonal system, end second derivatives zero.
    pub fn second_derivatives(&self) -> Vec<f64> {
        let p = &self.points;
        let n = p.len();
        if n <= 2 {
            return vec![0.0; n.max(1)];
        }
        let h: Vec<f64> = (0..n - 1).map(|i| p[i + 1].x - p[i].x).collect();
        let mut diagonal = vec![0.0; n];
        let mut rhs = vec![0.0; n];
        let mut upper = vec![0.0; n];
        for i in 1..n - 1 {
            if !(h[i - 1] > 0.0 && h[i] > 0.0) {
                continue;
            }
            let after = (p[i + 1].y - p[i].y) / h[i];
            let before = (p[i].y - p[i - 1].y) / h[i - 1];
            diagonal[i] = 2.0 * (h[i - 1] + h[i]);
            upper[i] = h[i];
            rhs[i] = 6.0 * (after - before);
        }
        for i in 2..n - 1 {
            if diagonal[i - 1] == 0.0 {
                continue;
            }
            let factor = h[i - 1] / diagonal[i - 1];
            diagonal[i] -= factor * upper[i - 1];
            rhs[i] -= factor * rhs[i - 1];
        }
        let mut m = vec![0.0; n];
        for i in (1..=n - 2).rev() {
            if diagonal[i] != 0.0 {
                m[i] = (rhs[i] - upper[i] * m[i + 1]) / diagonal[i];
            }
        }
        m
    }

    pub fn value(&self, x: f64) -> f64 {
        self.evaluated().value(x)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CurveSet {
    pub contrast: f32,
    pub luma: Curve,
    pub linked: Curve,
    pub red: Curve,
    pub green: Curve,
    pub blue: Curve,
}

impl CurveSet {
    pub const FOLD_POINTS: usize = 16;

    pub fn get(&self, channel: LevelsChannel) -> &Curve {
        match channel {
            LevelsChannel::Luma => &self.luma,
            LevelsChannel::Linked => &self.linked,
            LevelsChannel::Red => &self.red,
            LevelsChannel::Green => &self.green,
            LevelsChannel::Blue => &self.blue,
        }
    }

    pub fn get_mut(&mut self, channel: LevelsChannel) -> &mut Curve {
        match channel {
            LevelsChannel::Luma => &mut self.luma,
            LevelsChannel::Linked => &mut self.linked,
            LevelsChannel::Red => &mut self.red,
            LevelsChannel::Green => &mut self.green,
            LevelsChannel::Blue => &mut self.blue,
        }
    }

    pub fn touched(&self) -> Vec<LevelsChannel> {
        LevelsChannel::ALL.into_iter().filter(|c| !self.get(*c).is_neutral()).collect()
    }

    pub fn is_neutral(&self) -> bool {
        self.touched().is_empty() && self.contrast == 0.0
    }

    /// The retired colour curves a stored document carries under `key`, channels and contrast retired.
    pub fn retired(settings: Option<&Value>, key: &str) -> Option<CurveSet> {
        let raw = settings?.as_object()?.get(key)?;
        let mut set: CurveSet = serde_json::from_value(raw.clone()).ok()?;
        set.red = Curve::default();
        set.green = Curve::default();
        set.blue = Curve::default();
        set.contrast = 0.0;
        (!set.is_neutral()).then_some(set)
    }

    /// These curves with `colour`'s linked and luma tables run after them, sampled on display values.
    pub fn folding(&self, colour: &CurveSet) -> CurveSet {
        let mut out = self.clone();
        let xs: Vec<f64> = (0..=Self::FOLD_POINTS)
            .map(|i| (i as f64 / Self::FOLD_POINTS as f64).powf(f64::from(Print::DISPLAY_GAMMA)))
            .collect();
        let held = |v: f64| v.clamp(0.0, 1.0);
        if !colour.linked.is_neutral() {
            let mine = self.linked.evaluated();
            let theirs = colour.linked.evaluated();
            out.linked = Curve {
                points: xs
                    .iter()
                    .map(|&x| {
                        let bent = mine.value(x) as f32;
                        let contrasted =
                            if self.contrast != 0.0 { contrast::apply(bent, self.contrast, 0.5) } else { bent };
                        Point::new(x, held(theirs.value(f64::from(contrasted))))
                    })
                    .collect(),
            };
            out.contrast = 0.0;
        }
        if !colour.luma.is_neutral() {
            let mine = self.luma.evaluated();
            let theirs = colour.luma.evaluated();
            out.luma = Curve { points: xs.iter().map(|&x| Point::new(x, held(theirs.value(mine.value(x))))).collect() };
        }
        out
    }
}

/// Saturation by rendered luminance: five points, 0.5 neutral, 0 grey, 1 doubled.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SaturationCurve {
    pub points: Vec<Point>,
}

impl Default for SaturationCurve {
    fn default() -> Self {
        SaturationCurve { points: Self::resting() }
    }
}

/// The two former zone doses win over any points: a merged block also carries resting points.
impl<'de> Deserialize<'de> for SaturationCurve {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let map = tolerant::object(Value::deserialize(d)?, "SaturationCurve")?;
        if map.contains_key("shadows") || map.contains_key("highlights") {
            let shadows = tolerant::opt::<f32, D::Error>(&map, "shadows")?.unwrap_or(0.0);
            let highlights = tolerant::opt::<f32, D::Error>(&map, "highlights")?.unwrap_or(0.0);
            return Ok(Self::converted(shadows, highlights));
        }
        match tolerant::opt::<Vec<Point>, D::Error>(&map, "points")? {
            Some(points) if points.len() == 5 => Ok(SaturationCurve { points }),
            _ => Ok(SaturationCurve::default()),
        }
    }
}

impl SaturationCurve {
    pub const SPACING: f64 = 0.04;

    pub fn resting() -> Vec<Point> {
        [0.0, 0.25, 0.5, 0.75, 1.0].iter().map(|&x| Point::new(x, 0.5)).collect()
    }

    pub fn is_neutral(&self) -> bool {
        self.points.iter().all(|p| p.y == 0.5)
    }

    /// The two zones' factor `1 + s·(1 − t) + h·t` on a smoothstep, laid on the resting points.
    pub fn converted(shadows: f32, highlights: f32) -> SaturationCurve {
        let points = Self::resting()
            .into_iter()
            .map(|p| {
                let x = p.x as f32;
                let t = x * x * (3.0 - 2.0 * x);
                let factor = (1.0 + shadows * (1.0 - t) + highlights * t).max(0.0);
                Point::new(p.x, f64::from((factor / 2.0).min(1.0)))
            })
            .collect();
        SaturationCurve { points }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folded_colour_curve_runs_after_the_hand_s() {
        let hand = CurveSet {
            linked: Curve { points: vec![Point::new(0.0, 0.0), Point::new(0.2, 0.26), Point::new(1.0, 1.0)] },
            contrast: 0.2,
            ..CurveSet::default()
        };
        let colour = CurveSet {
            linked: Curve { points: vec![Point::new(0.0, 0.0), Point::new(0.3, 0.42), Point::new(1.0, 1.0)] },
            ..CurveSet::default()
        };
        let folded = hand.folding(&colour);
        assert_eq!(folded.contrast, 0.0);
        let composed = |x: f64| {
            let bent = contrast::apply(hand.linked.value(x) as f32, hand.contrast, 0.5);
            colour.linked.value(f64::from(bent)).clamp(0.0, 1.0)
        };
        let mut worst: f64 = 0.0;
        let mut unfolded: f64 = 0.0;
        for i in 0..=200 {
            let x = i as f64 / 200.0;
            worst = worst.max((folded.linked.value(x) - composed(x)).abs());
            unfolded = unfolded.max((hand.linked.value(x) - composed(x)).abs());
        }
        assert!(worst < 0.01, "the fold departs from the two passes by {worst}");
        // Twin: the hand's curve alone is far from the two passes, so the fold is doing work.
        assert!(unfolded > 5.0 * worst && unfolded > 0.02, "{unfolded}");
    }

    #[test]
    fn the_former_zones_win_over_resting_points() {
        let read: SaturationCurve =
            serde_json::from_str(r#"{"shadows":0.4,"highlights":-0.2,"points":[[0,0.5],[0.25,0.5],[0.5,0.5],[0.75,0.5],[1,0.5]]}"#)
                .unwrap();
        assert_eq!(read, SaturationCurve::converted(0.4, -0.2));
        assert!(!read.is_neutral());
        // Twin: points alone are read as they stand, and a wrong count falls back on rest.
        let points: SaturationCurve =
            serde_json::from_str(r#"{"points":[[0,0.1],[0.25,0.5],[0.5,0.5],[0.75,0.5],[1,0.5]]}"#).unwrap();
        assert_eq!(points.points[0].y, 0.1);
        let short: SaturationCurve = serde_json::from_str(r#"{"points":[[0,0.1],[1,0.5]]}"#).unwrap();
        assert!(short.is_neutral());
    }

    #[test]
    fn the_spline_passes_through_its_points() {
        let curve = Curve { points: vec![Point::new(0.0, 0.0), Point::new(0.4, 0.55), Point::new(1.0, 1.0)] };
        assert!((curve.value(0.4) - 0.55).abs() < 1e-12);
        assert!(curve.value(0.2) > 0.2);
        assert!((Curve::default().value(0.3) - 0.3).abs() < 1e-12);
    }
}
