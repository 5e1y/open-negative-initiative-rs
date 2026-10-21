//! The stroke being painted: its points and its path on the canvas, grown by one segment a move.

use std::fmt::Write;

/// A point closer than this to the last kept, in canvas points, adds nothing the eye or the patch sees.
const MIN_STEP: f64 = 1.0;

/// The stroke's points in fractions of the rendered frame, y down, and its path in canvas points;
/// a move appends one segment, so it costs the same at the stroke's thousandth point as at its first.
#[derive(Default)]
pub struct Live {
    points: Vec<(f64, f64)>,
    path: String,
    last: Option<(f64, f64)>,
}

impl Live {
    pub fn clear(&mut self) {
        self.points.clear();
        self.path.clear();
        self.last = None;
    }

    pub fn take(&mut self) -> Vec<(f64, f64)> {
        let points = std::mem::take(&mut self.points);
        self.clear();
        points
    }

    /// Adds `p`, laid on the canvas by the picture's frame `f`; false when it falls within
    /// `MIN_STEP` of the last point kept, which leaves the stroke as it was.
    pub fn push(&mut self, p: (f64, f64), f: (f64, f64, f64, f64)) -> bool {
        let at = (f.0 + p.0 * f.2, f.1 + p.1 * f.3);
        if self.last.is_some_and(|l| (at.0 - l.0).hypot(at.1 - l.1) < MIN_STEP) {
            return false;
        }
        let verb = if self.points.is_empty() { "M" } else { " L" };
        let _ = write!(self.path, "{verb} {:.2} {:.2}", at.0, at.1);
        self.points.push(p);
        self.last = Some(at);
        true
    }

    /// The path once two points make a line, else nothing to draw.
    pub fn commands(&self) -> &str {
        if self.points.len() > 1 { &self.path } else { "" }
    }

    /// The path laid again on a frame the view moved, the points kept.
    pub fn relaid(&mut self, f: (f64, f64, f64, f64)) {
        let points = std::mem::take(&mut self.points);
        self.path.clear();
        self.last = None;
        for p in points {
            let at = (f.0 + p.0 * f.2, f.1 + p.1 * f.3);
            let verb = if self.points.is_empty() { "M" } else { " L" };
            let _ = write!(self.path, "{verb} {:.2} {:.2}", at.0, at.1);
            self.points.push(p);
            self.last = Some(at);
        }
    }
}

/// `boundingFraction(of:).mid`: the centre of the points' bounding box, where a mark's line starts.
pub fn bounding_centre(points: impl Iterator<Item = (f64, f64)> + Clone) -> Option<(f64, f64)> {
    let (x0, x1) = points.clone().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.0), b.max(p.0)));
    let (y0, y1) = points.fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
    (x0 <= x1).then_some(((x0 + x1) / 2.0, (y0 + y1) / 2.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: (f64, f64, f64, f64) = (10.0, 20.0, 400.0, 200.0);

    /// The path a stroke's points make when built whole, as the canvas drew it before.
    fn whole(points: &[(f64, f64)], f: (f64, f64, f64, f64)) -> String {
        points.iter().enumerate().map(|(i, p)| format!("{} {:.2} {:.2}", if i == 0 { "M" } else { "L" }, f.0 + p.0 * f.2, f.1 + p.1 * f.3)).collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn a_path_grown_a_segment_a_move_is_the_path_built_whole() {
        let points: Vec<(f64, f64)> = (0..300).map(|i| (0.1 + i as f64 * 0.003, 0.5 + (i as f64 * 0.1).sin() * 0.05)).collect();
        let mut live = Live::default();
        assert!(points.iter().all(|&p| live.push(p, FRAME)));
        assert_eq!(live.commands(), whole(&points, FRAME));
        // Twin: a path built over the wrong frame is another string, so the comparison sees a slip.
        assert_ne!(live.commands(), whole(&points, (FRAME.0 + 1.0, FRAME.1, FRAME.2, FRAME.3)));
        live.relaid((0.0, 0.0, 800.0, 400.0));
        assert_eq!(live.commands(), whole(&points, (0.0, 0.0, 800.0, 400.0)), "the view moved lays it again");
    }

    #[test]
    fn a_move_under_a_point_adds_nothing_and_one_point_draws_nothing() {
        let mut live = Live::default();
        assert!(live.push((0.5, 0.5), FRAME));
        assert_eq!(live.commands(), "", "one point is no line yet");
        // A quarter of a canvas point: 0.000625 of a 400-point-wide frame.
        assert!(!live.push((0.500_625, 0.5), FRAME));
        assert!(live.push((0.503, 0.5), FRAME), "twin: 1.2 points away is kept");
        assert_eq!(live.take().len(), 2);
        assert_eq!(live.commands(), "");
    }

    #[test]
    fn a_marks_line_starts_at_the_bounding_boxs_centre_not_the_points_mean() {
        // Three points bunched on the left: the mean leans left, the box's centre does not.
        let points = [(0.1, 0.2), (0.11, 0.2), (0.5, 0.6)];
        let c = bounding_centre(points.iter().copied()).unwrap();
        assert!((c.0 - 0.3).abs() < 1e-12 && (c.1 - 0.4).abs() < 1e-12, "{c:?}");
        let mean = points.iter().map(|p| p.0).sum::<f64>() / 3.0;
        assert!((mean - c.0).abs() > 0.05, "twin: the mean is another point, which the check would see");
        assert_eq!(bounding_centre(std::iter::empty()), None);
    }
}
