//! A patched curve's points, laid again through the curve's own move: a point placed between the open
//! frame's neighbours may cross another frame's, which the render reads in x order alone. A patched
//! level's end is laid through its own setter alike, so it never crosses that frame's other end.

use super::Field;
use oni_settings::PipelineSettings;
use oni_settings::blocks::GrainCurve;
use oni_settings::curves::{CurveSet, SaturationCurve};
use oni_settings::levels::{Levels, LevelsChannel, LevelsSet};

/// The fields holding curves whose points keep their x order, and windows whose ends keep theirs.
pub(super) fn bends(field: Field) -> bool {
    matches!(field, Field::ManualCurves | Field::Saturation | Field::Levels | Field::Grain)
}

/// `on`'s curves after a patch from `moved`'s first to its second, each point it moved laid on `own`, the
/// frame before it, by the curve's move.
pub(super) fn ordered(field: Field, moved: (&PipelineSettings, &PipelineSettings), own: &PipelineSettings, on: &mut PipelineSettings) {
    match field {
        Field::ManualCurves => set(&own.manual_curves, &mut on.manual_curves),
        Field::Saturation => saturation(&own.saturation, &mut on.saturation),
        Field::Levels => windows(&own.levels, &mut on.levels),
        Field::Grain => on.grain.curve = grain(&moved.0.grain.curve, &moved.1.grain.curve, own.grain.curve),
        _ => {}
    }
}

fn set(own: &CurveSet, on: &mut CurveSet) {
    for channel in LevelsChannel::ALL {
        let mut out = own.get(channel).clone();
        let patched = on.get_mut(channel);
        if moved(&own.get(channel).points, &patched.points, |i, p| out.move_point(i, p)) {
            *patched = out;
        }
    }
}

fn saturation(own: &SaturationCurve, on: &mut SaturationCurve) {
    let mut out = own.clone();
    if moved(&own.points, &on.points, |i, p| out.move_point(i, p)) {
        *on = out;
    }
}

/// The points moved from `before` to `after` laid on the frame's `own` curve: a curve at rest is not
/// written, so the patch's leaves alone would lay the open frame's whole curve on it.
fn grain(before: &GrainCurve, after: &GrainCurve, mut own: GrainCurve) -> GrainCurve {
    for (i, p) in after.points.iter().enumerate().filter(|(i, p)| before.points[*i] != **p) {
        own.move_point(i, *p);
    }
    own
}

/// Each window's ends as patched: a pair moved together keeps the order it came in; one end moved
/// alone is set against the frame's own other end.
fn windows(own: &LevelsSet, on: &mut LevelsSet) {
    for channel in LevelsChannel::PER_CHANNEL {
        let (was, patched) = (own.get(channel), on.get(channel));
        let (black, white) = (patched.black != was.black, patched.white != was.white);
        if black == white {
            continue;
        }
        let mut out = Levels { black: was.black, white: was.white, ..patched };
        if black {
            out.set_black(patched.black);
        } else {
            out.set_white(patched.white);
        }
        on.set(channel, out);
    }
}

/// Each point of `patched` off `own` handed to `place`, in order; a list of another length was laid
/// whole, in the open frame's own order, and is left.
fn moved(own: &[oni_settings::cg::Point], patched: &[oni_settings::cg::Point], mut place: impl FnMut(usize, oni_settings::cg::Point)) -> bool {
    if own.len() != patched.len() {
        return false;
    }
    for (i, p) in patched.iter().enumerate().filter(|(i, p)| own[*i] != **p) {
        place(i, *p);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::cg::Point;
    use oni_settings::curves::Curve;

    fn curve(xs: [f64; 4]) -> Curve {
        Curve { points: xs.iter().map(|&x| Point::new(x, x)).collect() }
    }

    fn in_order(c: &Curve) -> bool {
        c.points.windows(2).all(|w| w[0].x < w[1].x)
    }

    #[test]
    fn a_patched_point_stays_between_the_frame_s_own_neighbours() {
        let mut was = PipelineSettings::default();
        was.manual_curves.red = curve([0.0, 0.5, 0.9, 1.0]);
        let mut now = was.clone();
        now.manual_curves.red.move_point(1, Point::new(0.85, 0.7));
        let mut frame = PipelineSettings::default();
        frame.manual_curves.red = curve([0.0, 0.2, 0.4, 1.0]);
        let mut on = frame.clone();
        Field::ManualCurves.patch(&was, &now, &mut on);
        let red = &on.manual_curves.red;
        assert!(in_order(red), "{:?}", red.points);
        assert_eq!((red.points[1].y, red.points[2]), (0.7, Point::new(0.4, 0.4)), "the y moved, its neighbour its own");
        assert!((red.points[1].x - (0.4 - Curve::MARGIN)).abs() < 1e-12, "held short of its own neighbour: {}", red.points[1].x);
        // Twin: the leaves laid alone cross the frame's own neighbour.
        let mut leaves = frame.clone();
        Field::ManualCurves.patched_leaves(&was, &now, &mut leaves);
        assert!(!in_order(&leaves.manual_curves.red));
        // The saturation points keep their spacing, the ends their x.
        let was = PipelineSettings::default();
        let mut now = was.clone();
        now.saturation.move_point(1, Point::new(0.45, 0.6));
        let mut on = frame.clone();
        on.saturation.points[2] = Point::new(0.3, 0.8);
        let own = on.clone();
        Field::Saturation.patch(&was, &now, &mut on);
        assert!((on.saturation.points[1].x - (0.3 - SaturationCurve::SPACING)).abs() < 1e-12, "{:?}", on.saturation.points);
        let mut leaves = own;
        Field::Saturation.patched_leaves(&was, &now, &mut leaves);
        assert!(leaves.saturation.points[1].x > leaves.saturation.points[2].x, "twin: {:?}", leaves.saturation.points);
    }

    #[test]
    fn a_patched_grain_point_keeps_its_spacing_on_the_frame_s_own_curve() {
        let was = PipelineSettings::default();
        let mut now = was.clone();
        now.grain.curve.move_point(1, Point::new(0.62, 0.8));
        let mut frame = PipelineSettings::default();
        frame.grain.curve.points[2] = Point::new(0.4, 0.9);
        let mut on = frame.clone();
        Field::Grain.patch(&was, &now, &mut on);
        let points = on.grain.curve.points;
        assert!((points[1].x - (0.4 - oni_settings::curves::SPACING)).abs() < 1e-12, "{points:?}");
        assert_eq!((points[1].y, points[2]), (0.8, Point::new(0.4, 0.9)), "the y moved, its neighbour its own");
        // Twin: the leaves laid alone take the open frame's whole curve, the frame's own point lost.
        let mut leaves = frame.clone();
        Field::Grain.patched_leaves(&was, &now, &mut leaves);
        assert_eq!(leaves.grain.curve, now.grain.curve);
    }

    #[test]
    fn a_patched_level_end_never_crosses_the_frame_s_own_other_end() {
        let was = PipelineSettings::default();
        let mut now = was.clone();
        now.levels.red.set_black(1.6);
        now.levels.green = Levels::with_ends(0.2, 0.7);
        let mut frame = PipelineSettings::default();
        frame.levels.red.set_white(1.4);
        let mut on = frame.clone();
        Field::Levels.patch(&was, &now, &mut on);
        let red = on.levels.red;
        assert_eq!((red.black, red.white), (1.4 - Levels::EPSILON, 1.4), "held short of its own white");
        assert_eq!(on.levels.green, Levels::with_ends(0.2, 0.7), "a pair moved together lands whole");
        // Twin: the leaves laid alone put the black past that frame's white.
        let mut leaves = frame.clone();
        Field::Levels.patched_leaves(&was, &now, &mut leaves);
        assert!(leaves.levels.red.black > leaves.levels.red.white);
    }
}
