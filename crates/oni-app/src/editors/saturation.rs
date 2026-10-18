//! `SaturationCurveView`: the five points over the output's histogram, laid on linear light; a press
//! takes the point under it, else the closest along the light, and a double click sets it neutral.

use super::geometry::{Dot, Drawing, Ink, Line, Size, distance, ds, polyline, segments};
use oni_settings::cg::Point;
use oni_settings::curves::{Curve, SaturationCurve};
use oni_settings::tone::Print;

/// Samples of the drawn spline.
pub const STEPS: usize = 256;

/// The curve reads perceived light; the plot lays it on linear light, as the histogram is.
pub fn screen(p: Point, size: Size) -> (f64, f64) {
    (p.x.powf(f64::from(Print::DISPLAY_GAMMA)) * size.w, (1.0 - p.y) * size.h)
}

/// The point under the pointer, within a grab area: the closest, the first of a tie.
pub fn nearest(curve: &SaturationCurve, at: (f64, f64), size: Size) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, p) in curve.points.iter().enumerate() {
        let d = distance(screen(*p, size), at);
        if d < ds::GRAB_SIZE && best.is_none_or(|(_, b)| d < b) {
            best = Some((i, d));
        }
    }
    best.map(|(i, _)| i)
}

/// The point closest along the light, whatever its height.
pub fn closest_along(curve: &SaturationCurve, at: (f64, f64), size: Size) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, p) in curve.points.iter().enumerate() {
        let d = (screen(*p, size).0 - at.0).abs();
        if best.is_none_or(|(_, b)| d < b) {
            best = Some((i, d));
        }
    }
    best.map(|(i, _)| i)
}

/// Where the pointer asks a point to be: across on linear light read back as perceived, up the factor.
pub fn asked(at: (f64, f64), size: Size) -> Point {
    let across = (at.0 / size.w.max(1.0)).clamp(0.0, 1.0);
    Point::new(across.powf(1.0 / f64::from(Print::DISPLAY_GAMMA)), 1.0 - at.1 / size.h.max(1.0))
}

/// The view's memory of the last two presses: a double click's first press has already moved a
/// point to the pointer, so the neutralising starts from before it.
#[derive(Clone, Debug, Default)]
pub struct Presses {
    pub dragged: Option<usize>,
    before_press: Vec<Point>,
    before_last_press: Vec<Point>,
}

impl Presses {
    /// A press takes its point and lays it at the pointer at once.
    pub fn press(&mut self, curve: &mut SaturationCurve, at: (f64, f64), size: Size) {
        self.before_last_press = std::mem::take(&mut self.before_press);
        self.before_press = curve.points.clone();
        self.dragged = nearest(curve, at, size).or_else(|| closest_along(curve, at, size));
        self.drag(curve, at, size);
    }

    pub fn drag(&self, curve: &mut SaturationCurve, at: (f64, f64), size: Size) {
        if let Some(index) = self.dragged {
            curve.move_point(index, asked(at, size));
        }
    }

    pub fn release(&mut self) {
        self.dragged = None;
    }

    /// The double click's end: the points before its first press, the one under the pointer on 0.5.
    pub fn double(&self, curve: &SaturationCurve, at: (f64, f64), size: Size) -> Option<SaturationCurve> {
        let index = nearest(curve, at, size)?;
        let mut next = curve.clone();
        if !self.before_last_press.is_empty() && self.before_last_press.len() == next.points.len() {
            next.points = self.before_last_press.clone();
        }
        let x = next.points[index].x;
        next.move_point(index, Point::new(x, 0.5));
        Some(next)
    }
}

/// The neutral line, the spline held to the plot, and the five points, the dragged one lit.
pub fn drawing(curve: &SaturationCurve, dragged: Option<usize>, size: Size) -> Drawing {
    let evaluated = Curve { points: curve.points.clone() }.evaluated();
    let trace: Vec<(f64, f64)> = (0..=STEPS)
        .map(|s| {
            let x = s as f64 / STEPS as f64;
            screen(Point::new(x, evaluated.value(x).clamp(0.0, 1.0)), size)
        })
        .collect();
    Drawing {
        lines: vec![
            Line::new(segments(&[((0.0, size.h / 2.0), (size.w, size.h / 2.0))]), Ink::Separator, ds::STROKE_HAIRLINE),
            Line::new(polyline(&trace), Ink::Active, ds::STROKE_TRACE),
        ],
        dots: curve
            .points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let at = screen(*p, size);
                let edge = if dragged == Some(i) { Ink::Active } else { Ink::Idle };
                Dot { x: at.0, y: at.1, size: ds::HANDLE_SIZE, fill: Ink::Canvas, edge, edge_width: ds::STROKE_TRACE, halo: 0.0 }
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLOT: Size = Size::new(308.0, 96.0);

    #[test]
    fn a_press_takes_the_point_under_it_else_the_closest_along_the_light() {
        let mut c = SaturationCurve::default();
        let mut presses = Presses::default();
        // Over the middle point, raised: it rises to the pointer at once.
        let middle = screen(c.points[2], PLOT);
        presses.press(&mut c, (middle.0 + 3.0, 20.0), PLOT);
        assert_eq!(presses.dragged, Some(2), "nothing within reach, the closest along the light");
        assert!((c.points[2].y - (1.0 - 20.0 / 96.0)).abs() < 1e-12);
        let expected_x = ((middle.0 + 3.0) / PLOT.w).powf(1.0 / 2.2);
        assert!((c.points[2].x - expected_x).abs() < 1e-6, "the pointer is read back on perceived light");
        presses.release();
        // Twin: a press beside a disc takes that disc over the end within the same reach.
        let mut c = SaturationCurve::default();
        let mut presses = Presses::default();
        let first = screen(c.points[1], PLOT);
        presses.press(&mut c, (first.0 + 2.0, first.1 + 2.0), PLOT);
        assert_eq!(presses.dragged, Some(1));
    }

    #[test]
    fn a_double_click_neutralises_from_before_its_first_press() {
        let mut c = SaturationCurve::default();
        c.points[3].y = 0.9;
        c.points[1].y = 0.2;
        let start = c.clone();
        let mut presses = Presses::default();
        let at = screen(c.points[3], PLOT);
        let at = (at.0 + 6.0, at.1 + 4.0);
        presses.press(&mut c, at, PLOT);
        presses.release();
        presses.press(&mut c, at, PLOT);
        presses.release();
        assert_ne!(c, start, "the two presses moved the point to the pointer");
        let next = presses.double(&c, at, PLOT).unwrap();
        assert_eq!(next.points[3], Point::new(start.points[3].x, 0.5), "back on its x before the first press");
        assert_eq!(next.points[1], start.points[1], "the other points as they were");
        // Twin: a double click far from every point does nothing.
        assert!(presses.double(&c, (150.0, -200.0), PLOT).is_none());
    }

    #[test]
    fn the_curve_is_laid_on_linear_light() {
        let c = SaturationCurve::default();
        assert!((screen(c.points[2], PLOT).0 - 0.5f64.powf(f64::from(Print::DISPLAY_GAMMA)) * PLOT.w).abs() < 1e-9);
        let d = drawing(&c, Some(4), PLOT);
        assert_eq!(d.dots[4].edge, Ink::Active);
        assert_eq!(d.dots[0].edge, Ink::Idle);
        // Twin: laid on perceived light the middle point would sit at the centre.
        assert!((screen(c.points[2], PLOT).0 - PLOT.w / 2.0).abs() > 50.0);
    }
}
