//! `CurveEditorView` and `CurveSetEditor`: a channel's spline over its histogram, a press near a
//! point taking it to the pointer, a press elsewhere adding one, a double click on a point removing it.

use super::geometry::{Dot, Drawing, Ink, Line, Size, distance, ds, polyline, segments};
use oni_settings::cg::Point;
use oni_settings::curves::{Curve, CurveSet};
use oni_settings::levels::LevelsChannel;

/// `CurveEditorView.nearestPoint`'s reach around a point, in points.
pub const HIT_RADIUS: f64 = 14.0;
/// How near in x the point a press added is looked for again.
pub const FOUND_WITHIN: f64 = 0.01;
/// Samples of the drawn spline.
pub const STEPS: usize = 128;
/// The channels the Curves block offers, in its tab order.
pub const CHANNELS: [LevelsChannel; 5] =
    [LevelsChannel::Linked, LevelsChannel::Luma, LevelsChannel::Red, LevelsChannel::Green, LevelsChannel::Blue];

pub fn tint(channel: LevelsChannel) -> Ink {
    match channel {
        LevelsChannel::Luma | LevelsChannel::Linked => Ink::Primary,
        LevelsChannel::Red => Ink::Red,
        LevelsChannel::Green => Ink::Green,
        LevelsChannel::Blue => Ink::Blue,
    }
}

/// The curve's 0…1, y up, to the view's top-left origin, y down.
pub fn screen(p: Point, size: Size) -> (f64, f64) {
    (p.x * size.w, (1.0 - p.y) * size.h)
}

pub fn normalized(at: (f64, f64), size: Size) -> Point {
    Point::new(at.0 / size.w, 1.0 - at.1 / size.h)
}

/// The closest point strictly inside the reach, the first of a tie.
pub fn nearest(curve: &Curve, at: (f64, f64), size: Size) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, p) in curve.points.iter().enumerate() {
        let d = distance(screen(*p, size), at);
        if d < HIT_RADIUS && best.is_none_or(|(_, b)| d < b) {
            best = Some((i, d));
        }
    }
    best.map(|(i, _)| i)
}

/// The drawn disc under the pointer, the last drawn on top: what a double click removes.
pub fn dot_under(curve: &Curve, at: (f64, f64), size: Size) -> Option<usize> {
    curve.points.iter().rposition(|p| distance(screen(*p, size), at) <= ds::RADIUS_BADGE / 2.0)
}

/// The press: the nearest point within reach goes to the pointer, else a point is added there and
/// the first point within `FOUND_WITHIN` of its x becomes the one the drag moves.
pub fn press(curve: &mut Curve, at: (f64, f64), size: Size) -> Option<usize> {
    if let Some(n) = nearest(curve, at, size) {
        curve.move_point(n, normalized(at, size));
        return Some(n);
    }
    let added = normalized(at, size);
    curve.add(added);
    curve.points.iter().position(|p| (p.x - added.x).abs() < FOUND_WITHIN)
}

/// Each move after the press: the dragged point at the pointer, held by its neighbours.
pub fn drag(curve: &mut Curve, index: usize, at: (f64, f64), size: Size) {
    curve.move_point(index, normalized(at, size));
}

/// The diagonal, the spline and its points, in the channel's tint.
pub fn drawing(curve: &Curve, channel: LevelsChannel, size: Size) -> Drawing {
    let evaluated = curve.evaluated();
    let trace: Vec<(f64, f64)> = (0..=STEPS)
        .map(|s| {
            let x = s as f64 / STEPS as f64;
            screen(Point::new(x, evaluated.value(x)), size)
        })
        .collect();
    let ink = tint(channel);
    Drawing {
        lines: vec![
            Line::new(segments(&[((0.0, size.h), (size.w, 0.0))]), Ink::Separator, ds::STROKE_HAIRLINE),
            Line::new(polyline(&trace), ink, ds::STROKE_TRACE),
        ],
        dots: curve.points.iter().map(|p| Dot::filled(screen(*p, size), ds::RADIUS_BADGE, ink)).collect(),
    }
}

/// The tabs lit for a curve off the diagonal.
pub fn marked(curves: &CurveSet) -> Vec<bool> {
    CHANNELS.iter().map(|c| !curves.get(*c).is_neutral()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIDE: Size = Size::new(300.0, 300.0);

    #[test]
    fn a_press_near_a_point_takes_it_to_the_pointer_and_elsewhere_adds_one() {
        let mut c = Curve::default();
        // 10 pt from the black end: inside the reach, so the end moves to the pointer.
        let at = (6.0, 300.0 - 8.0);
        assert_eq!(press(&mut c, at, SIDE), Some(0));
        assert_eq!(c.points.len(), 2);
        assert_eq!(c.points[0], normalized(at, SIDE));
        // 15 pt away: outside the reach, a point is added and dragged.
        let mut c = Curve::default();
        let at = (150.0, 90.0);
        let index = press(&mut c, at, SIDE).unwrap();
        assert_eq!((c.points.len(), index), (3, 1));
        drag(&mut c, index, (165.0, 60.0), SIDE);
        assert_eq!(c.points[1], Point::new(0.55, 0.8));
        // Twin: at 14 pt exactly the reach is strict, so a point is added rather than the end taken.
        let mut c = Curve::default();
        press(&mut c, (14.0, 300.0), SIDE);
        assert_eq!(c.points.len(), 3);
    }

    #[test]
    fn the_point_a_press_adds_is_found_again_by_its_x() {
        // An older point 0.005 left of the press, far below it: the drag takes the older point.
        let mut c = Curve { points: vec![Point::new(0.0, 0.0), Point::new(0.5, 0.2), Point::new(1.0, 1.0)] };
        let at = (151.5, 30.0);
        let index = press(&mut c, at, SIDE).unwrap();
        assert_eq!(c.points.len(), 4);
        assert_eq!(c.points[index], Point::new(0.5, 0.2), "Swift's `firstIndex` finds the older point");
        // Twin: a press 0.02 away finds the new one.
        let mut c = Curve { points: vec![Point::new(0.0, 0.0), Point::new(0.5, 0.2), Point::new(1.0, 1.0)] };
        let index = press(&mut c, (156.0, 30.0), SIDE).unwrap();
        assert!((c.points[index].x - 0.52).abs() < 1e-12);
    }

    #[test]
    fn a_double_click_finds_the_disc_under_the_pointer() {
        let c = Curve { points: vec![Point::new(0.0, 0.0), Point::new(0.5, 0.5), Point::new(1.0, 1.0)] };
        assert_eq!(dot_under(&c, (153.0, 150.0), SIDE), Some(1));
        // Twin: 5 pt off the disc's centre is beside its 8 pt disc, though inside the drag's reach.
        assert_eq!(dot_under(&c, (155.0, 150.0), SIDE), None);
        assert_eq!(nearest(&c, (155.0, 150.0), SIDE), Some(1));
    }

    #[test]
    fn the_spline_is_drawn_through_its_points_in_the_channel_s_tint() {
        let c = Curve { points: vec![Point::new(0.0, 0.0), Point::new(0.5, 0.7), Point::new(1.0, 1.0)] };
        let d = drawing(&c, LevelsChannel::Red, SIDE);
        assert_eq!(d.dots.len(), 3);
        assert_eq!(d.lines[1].ink, Ink::Red);
        assert!(d.lines[1].commands.contains("L 150.00 90.00"), "the trace passes through the point");
        assert_eq!(d.lines[1].commands.matches('L').count(), STEPS);
        // Twin: the luma curve is drawn in the primary ink.
        assert_eq!(drawing(&c, LevelsChannel::Luma, SIDE).lines[1].ink, Ink::Primary);
    }
}
