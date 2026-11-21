//! A long stroke as the corrections that heal it: each short enough for one patch to match what it crosses.

use super::Correction;
use crate::cg::Point;
use crate::id::Id;

impl Correction {
    /// The most a patch's stroke spans either way, in brush radii: a patch is one copy of a place at least 1.8 times
    /// its stroke's size away, alike enough for a short stroke; at 4 radii a fibre across three tones still left blobs.
    pub const SPAN: f64 = 2.0;

    /// A stroke through `points`, in pixels, as the strokes that span `span` at most either way: each starts where
    /// the last ends, so together they cover what the whole does; a last one under a quarter of it joins the one before.
    pub fn parts(points: &[(f64, f64)], span: f64) -> Vec<Vec<(f64, f64)>> {
        let Some(&first) = points.first() else { return vec![Vec::new()] };
        let wide = |b: &[f64; 4]| (b[2] - b[0]).max(b[3] - b[1]);
        let grown = |b: [f64; 4], p: (f64, f64)| [b[0].min(p.0), b[1].min(p.1), b[2].max(p.0), b[3].max(p.1)];
        let mut out: Vec<Vec<(f64, f64)>> = Vec::new();
        let (mut part, mut held) = (vec![first], [first.0, first.1, first.0, first.1]);
        for &q in &points[1..] {
            loop {
                let p = *part.last().expect("a part holds a point");
                // How far towards `q` the part's box stays within the span, on each axis.
                let along = |from: f64, by: f64, low: f64, high: f64| if by > 0.0 { (low + span - from) / by } else if by < 0.0 { (from - (high - span)) / -by } else { f64::INFINITY };
                let t = along(p.0, q.0 - p.0, held[0], held[2]).min(along(p.1, q.1 - p.1, held[1], held[3]));
                if t >= 1.0 || span <= 0.0 {
                    part.push(q);
                    held = grown(held, q);
                    break;
                }
                let cut = if t > 0.0 { (p.0 + t * (q.0 - p.0), p.1 + t * (q.1 - p.1)) } else { p };
                if t > 0.0 {
                    part.push(cut);
                }
                out.push(std::mem::replace(&mut part, vec![cut]));
                held = [cut.0, cut.1, cut.0, cut.1];
            }
        }
        match out.last_mut() {
            Some(before) if wide(&held) < span / 4.0 => before.extend(part.into_iter().skip(1)),
            _ => out.push(part),
        }
        out
    }

    /// This stroke kept on a scan of `size` pixels, as the corrections that each span `SPAN` radii at most, one gesture's.
    pub fn split(self, size: (f64, f64)) -> Vec<Correction> {
        let points: Vec<(f64, f64)> = self.stroke.iter().map(|p| (p.x * size.0, p.y * size.1)).collect();
        let mut parts = Self::parts(&points, Self::SPAN * self.radius);
        if parts.len() < 2 {
            return vec![self];
        }
        let (radius, gesture) = (self.radius, Some(self.gesture.unwrap_or_default()));
        parts.drain(..).map(|part| Correction { id: Id::new(), stroke: part.into_iter().map(|(x, y)| Point { x: x / size.0, y: y / size.1 }).collect(), radius, gesture }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span_of(part: &[(f64, f64)]) -> f64 {
        let b = part.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p.0), b[1].min(p.1), b[2].max(p.0), b[3].max(p.1)]);
        (b[2] - b[0]).max(b[3] - b[1])
    }

    #[test]
    fn a_long_stroke_is_cut_into_parts_that_join_and_span_no_more_than_asked() {
        // A hair's curve, a fast hand's far-apart points and a turn back on itself.
        let curve: Vec<(f64, f64)> = (0..=300).map(|i| (f64::from(i), 40.0 * (f64::from(i) * 0.03).sin())).collect();
        let fast = vec![(0.0, 0.0), (500.0, 20.0), (500.0, 400.0), (10.0, 390.0)];
        for points in [curve, fast] {
            let parts = Correction::parts(&points, 48.0);
            assert!(parts.len() > 5, "{}", parts.len());
            assert!(parts.iter().all(|p| p.len() >= 2 && span_of(p) <= 48.0 * 1.25 + 1e-9), "{:?}", parts.iter().map(|p| span_of(p)).collect::<Vec<_>>());
            assert!(parts[..parts.len() - 1].iter().all(|p| (span_of(p) - 48.0).abs() < 1e-9), "every part but the last spans the whole of it");
            assert!(parts.windows(2).all(|w| w[0].last() == w[1].first()), "a part starts where the last ends");
            assert_eq!((parts[0][0], *parts.last().unwrap().last().unwrap()), (points[0], *points.last().unwrap()));
            // Every point of the stroke lies on a part: none is dropped by a cut.
            let kept: Vec<(f64, f64)> = parts.iter().flatten().copied().collect();
            assert!(points.iter().all(|p| kept.contains(p)));
            // Twin: a span the stroke fits in leaves it whole.
            assert_eq!(Correction::parts(&points, 600.0), vec![points.clone()]);
        }
    }

    #[test]
    fn a_scribble_on_one_speck_stays_one_stroke_and_a_short_tail_joins_the_part_before() {
        // Back and forth over 30 pixels, 600 long: no wider than its span, however long the hand moved.
        let scribble: Vec<(f64, f64)> = (0..=600).map(|i| (15.0 + 15.0 * (f64::from(i) * 0.1).sin(), 15.0 + 15.0 * (f64::from(i) * 0.07).cos())).collect();
        assert_eq!(Correction::parts(&scribble, 48.0).len(), 1);
        let tailed = Correction::parts(&[(0.0, 0.0), (100.0, 0.0)], 48.0);
        assert_eq!(tailed, vec![vec![(0.0, 0.0), (48.0, 0.0)], vec![(48.0, 0.0), (96.0, 0.0), (100.0, 0.0)]]);
        // Twin: a tail of a quarter span or more is a part of its own.
        assert_eq!(Correction::parts(&[(0.0, 0.0), (110.0, 0.0)], 48.0).len(), 3);
    }

    #[test]
    fn a_long_correction_splits_on_its_scan_and_a_short_one_is_kept_as_it_is() {
        let long = Correction { id: Id::new(), stroke: vec![Point::new(0.1, 0.5), Point::new(0.2, 0.5)], radius: 10.0, gesture: None };
        let parts = long.clone().split((6000.0, 4000.0));
        // 600 pixels at 20 a part.
        assert_eq!(parts.len(), 30);
        assert!(parts.iter().all(|c| c.radius == 10.0 && c.id != long.id));
        assert!(parts[0].gesture.is_some() && parts.iter().all(|c| c.gesture == parts[0].gesture), "one gesture's");
        assert!((parts[0].stroke[0].x - 0.1).abs() < 1e-12 && (parts[29].stroke.last().unwrap().x - 0.2).abs() < 1e-12);
        assert!(parts.windows(2).all(|w| w[0].stroke.last() == w[1].stroke.first()));
        // Twin: on a scan a tenth the size the same stroke spans 60 pixels, and a dot is never cut.
        assert_eq!(long.clone().split((600.0, 400.0)).len(), 3);
        let dot = Correction { stroke: vec![Point::new(0.5, 0.5), Point::new(0.5001, 0.5)], ..long.clone() };
        assert_eq!(dot.clone().split((6000.0, 4000.0)), vec![dot]);
    }
}
