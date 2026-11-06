//! A patched curve's points, laid again through the curve's own move: a point placed between the open
//! frame's neighbours may cross another frame's, which the render reads in x order alone.

use super::Field;
use oni_settings::PipelineSettings;
use oni_settings::curves::{CurveSet, SaturationCurve};
use oni_settings::levels::LevelsChannel;

/// The fields holding curves whose points keep their x order.
pub(super) fn bends(field: Field) -> bool {
    matches!(field, Field::ManualCurves | Field::Saturation | Field::ScannerFinish)
}

/// `on`'s curves after a patch, each point it moved laid on `own`, the frame before it, by the curve's move.
pub(super) fn ordered(field: Field, own: &PipelineSettings, on: &mut PipelineSettings) {
    match field {
        Field::ManualCurves => set(&own.manual_curves, &mut on.manual_curves),
        Field::Saturation => saturation(&own.saturation, &mut on.saturation),
        Field::ScannerFinish => {
            set(&own.scanner_finish.manual_curves, &mut on.scanner_finish.manual_curves);
            saturation(&own.scanner_finish.saturation, &mut on.scanner_finish.saturation);
        }
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
}
