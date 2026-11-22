//! `SaturationCurveView`: the five points over the output's histogram, laid on linear light; a double click sets a point
//! neutral.

use super::geometry::{Dot, Drawing, Ink, Line, Size, ds, polyline, segments};
use super::points::{self, Points};
use oni_settings::cg::Point;
use oni_settings::curves::{Curve, SaturationCurve};
use oni_settings::tone::Print;

/// Samples of the drawn spline.
pub const STEPS: usize = 256;

/// The curve reads perceived light; the plot lays it on linear light, as the histogram is.
impl Points for SaturationCurve {
    const GAMMA: f64 = Print::DISPLAY_GAMMA as f64;

    fn points(&self) -> &[Point] {
        &self.points
    }

    fn points_mut(&mut self) -> &mut [Point] {
        &mut self.points
    }

    fn move_point(&mut self, index: usize, point: Point) {
        SaturationCurve::move_point(self, index, point);
    }

    /// Neutral, on its tone.
    fn rested(&self, index: usize) -> Point {
        Point::new(self.points[index].x, 0.5)
    }
}

pub fn screen(p: Point, size: Size) -> (f64, f64) {
    points::screen::<SaturationCurve>(p, size)
}

/// The neutral line, the spline held to the plot, and the five points, the dragged one lit.
pub fn drawing(curve: &SaturationCurve, dragged: Option<usize>, size: Size) -> Drawing {
    let trace = |at: &dyn Fn(f64) -> f64| -> Vec<(f64, f64)> {
        (0..=STEPS)
            .map(|s| {
                let x = s as f64 / STEPS as f64;
                screen(Point::new(x, at(x).clamp(0.0, 1.0)), size)
            })
            .collect()
    };
    let mut lines = vec![Line::new(segments(&[((0.0, size.h / 2.0), (size.w, size.h / 2.0))]), Ink::Separator, ds::STROKE_HAIRLINE)];
    let evaluated = Curve { points: curve.points.clone() }.evaluated();
    lines.push(Line::new(polyline(&trace(&|x| evaluated.value(x))), Ink::Active, ds::STROKE_TRACE));
    Drawing {
        lines,
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
    use super::super::points::Presses;

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
