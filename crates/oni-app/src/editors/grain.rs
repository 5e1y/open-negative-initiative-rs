//! The grain curve's view: its five points laid straight on the tones the photo shows, from its black
//! point to its white; a double click returns a point to its rest.

use super::geometry::{Dot, Drawing, Ink, Line, Size, ds, polyline};
use super::points::{self, Points};
use oni_settings::blocks::GrainCurve;
use oni_settings::cg::Point;

/// Samples of the drawn spline.
pub const STEPS: usize = 256;

/// The tones as the photo shows them, laid evenly across.
impl Points for GrainCurve {
    const GAMMA: f64 = 1.0;

    fn points(&self) -> &[Point] {
        &self.points
    }

    fn points_mut(&mut self) -> &mut [Point] {
        &mut self.points
    }

    fn move_point(&mut self, index: usize, point: Point) {
        GrainCurve::move_point(self, index, point);
    }

    fn rested(&self, index: usize) -> Point {
        GrainCurve::REST.points[index]
    }
}

pub fn screen(p: Point, size: Size) -> (f64, f64) {
    points::screen::<GrainCurve>(p, size)
}

/// The spline as the render holds it to the plot, and the five points, the dragged one lit.
pub fn drawing(curve: &GrainCurve, dragged: Option<usize>, size: Size) -> Drawing {
    let strength = curve.strength();
    let trace: Vec<(f64, f64)> = (0..=STEPS)
        .map(|s| {
            let x = s as f64 / STEPS as f64;
            screen(Point::new(x, strength.at(x)), size)
        })
        .collect();
    Drawing {
        lines: vec![Line::new(polyline(&trace), Ink::Active, ds::STROKE_TRACE)],
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
    use super::super::{Gestures, Plot};
    use super::*;
    use crate::store::tests::fresh;
    use crate::track::DOUBLE_CLICK;
    use std::time::{Duration, Instant};

    const PLOT: Size = Size::new(308.0, 96.0);

    #[test]
    fn the_curve_is_laid_straight_on_the_tones() {
        let c = GrainCurve::REST;
        assert!((screen(c.points[2], PLOT).0 - 0.596 * PLOT.w).abs() < 1e-9);
        let d = drawing(&c, Some(3), PLOT);
        assert_eq!((d.dots[3].edge, d.dots[0].edge), (Ink::Active, Ink::Idle));
        // Twin: laid on linear light the middle point would sit more than 50 points left.
        let bent = c.points[2].x.powf(2.2) * PLOT.w;
        assert!((screen(c.points[2], PLOT).0 - bent).abs() > 50.0);
    }

    #[test]
    fn a_double_click_returns_a_point_to_its_rest_from_before_its_first_press() {
        let (mut store, dir) = fresh("editors-grain");
        let mut g = Gestures::default();
        g.sizes[Plot::Grain as usize] = PLOT;
        let start = store.settings().grain.curve;
        let mid = screen(start.points[2], PLOT);
        let t0 = Instant::now();
        // A drag lowers the mids' point.
        g.press(&mut store, Plot::Grain, mid, t0);
        g.drag(&mut store, Plot::Grain, (mid.0 + 12.0, mid.1 + 40.0), false, t0 + Duration::from_millis(40));
        g.release(&mut store);
        let lowered = store.settings().grain.curve;
        assert!(lowered.points[2].y < 0.7 && lowered.points[2].x > start.points[2].x, "{:?}", lowered.points[2]);
        assert_eq!(lowered.points[1], start.points[1], "the others as they were");
        // A double click on it lays it back on its rest, its tone and its share.
        let at = screen(lowered.points[2], PLOT);
        let at = (at.0 + 3.0, at.1 - 4.0);
        let t1 = t0 + Duration::from_secs(2);
        g.press(&mut store, Plot::Grain, at, t1);
        g.release(&mut store);
        g.press(&mut store, Plot::Grain, at, t1 + Duration::from_millis(120));
        g.release(&mut store);
        assert_eq!(store.settings().grain.curve, GrainCurve::REST);
        // Twin: two slow presses leave the point where the pointer put it.
        let t2 = t1 + Duration::from_secs(2);
        g.press(&mut store, Plot::Grain, at, t2);
        g.release(&mut store);
        g.press(&mut store, Plot::Grain, at, t2 + DOUBLE_CLICK + Duration::from_millis(1));
        g.release(&mut store);
        assert_ne!(store.settings().grain.curve, GrainCurve::REST);
        // A double click far from every point lays nothing back.
        let presses = super::super::points::Presses::default();
        assert!(presses.double(&store.settings().grain.curve, (150.0, -200.0), PLOT).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
