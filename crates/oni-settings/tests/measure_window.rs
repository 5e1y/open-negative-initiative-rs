//! What a measure reads: the window seen inside the crop, or the frame while the crop is bypassed,
//! on fixed expectations that hold the degenerate cases.

use oni_settings::cg::Rect;
use oni_settings::frame::Geometry;
use oni_settings::geometry::{absolute, proposal, relative};

const NAN: f64 = f64::NAN;
const CROP: Rect = Rect { x: 0.25, y: 0.25, width: 0.5, height: 0.5 };

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect { x, y, width, height }
}

/// A window, the base it is seen in, what it reads as there, and in the frame once the crop is bypassed.
fn seen() -> Vec<(Rect, Rect, Rect, Option<Rect>)> {
    let middle = rect(0.25, 0.25, 0.5, 0.5);
    vec![
        (rect(0.375, 0.25, 0.25, 0.5), CROP, rect(0.25, 0.0, 0.5, 1.0), Some(rect(0.375, 0.25, 0.25, 0.5))),
        (rect(0.0, 0.0, 0.5, 0.5), CROP, rect(0.0, 0.0, 0.5, 0.5), Some(rect(0.0, 0.0, 0.5, 0.5))),
        (CROP, CROP, Rect::UNIT, Some(CROP)),
        (rect(0.875, 0.875, 0.125, 0.125), CROP, Rect::UNIT, Some(rect(0.875, 0.875, 0.125, 0.125))),
        (rect(0.75, 0.25, 0.25, 0.5), CROP, Rect::UNIT, Some(rect(0.75, 0.25, 0.25, 0.5))),
        (rect(0.375, 0.375, 0.0, 0.25), CROP, Rect::UNIT, None),
        (rect(0.375, 0.375, -0.25, 0.25), CROP, Rect::UNIT, None),
        (Rect::UNIT, CROP, Rect::UNIT, None),
        (middle, rect(0.0, 0.0, 0.0, 1.0), Rect::UNIT, Some(middle)),
        (middle, rect(0.0, 0.0, -0.5, 1.0), Rect::UNIT, Some(middle)),
        (middle, rect(0.0, 0.0, NAN, 1.0), Rect::UNIT, Some(middle)),
        (middle, rect(0.0, 0.0, 1.0, NAN), Rect::UNIT, Some(middle)),
        (middle, Rect::UNIT, middle, Some(middle)),
    ]
}

/// The settings' former relative, which computed through a side not a number.
fn computing_through(rect: Rect, base: Rect) -> Rect {
    let (x0, y0) = (rect.x.max(base.x), rect.y.max(base.y));
    let (x1, y1) = ((rect.x + rect.width).min(base.x + base.width), (rect.y + rect.height).min(base.y + base.height));
    if base.width <= 0.0 || base.height <= 0.0 || x1 <= x0 || y1 <= y0 {
        return Rect::UNIT;
    }
    Rect { x: (x0 - base.x) / base.width, y: (y0 - base.y) / base.height, width: (x1 - x0) / base.width, height: (y1 - y0) / base.height }
}

#[test]
fn a_window_reads_as_seen_from_its_base_and_the_whole_when_it_cannot() {
    for (window, base, expected, _) in seen() {
        assert_eq!(relative(window, base), expected, "{window:?} in {base:?}");
    }
    assert_eq!(absolute(rect(0.25, 0.0, 0.5, 1.0), CROP), rect(0.375, 0.25, 0.25, 0.5));
    // Twin: a relative computing through a side not a number misses the two rows that hold one.
    let missed = seen().into_iter().filter(|&(w, b, e, _)| computing_through(w, b) != e).count();
    assert_eq!(missed, 2);
}

#[test]
fn the_crop_or_the_bypassed_frame_is_what_the_window_is_seen_in() {
    for (window, base, expected, framed) in seen() {
        let cropped = Geometry { crop: base, measure_window: Some(window), ..Geometry::default() };
        assert_eq!(cropped.measure_window_in_crop(), (expected != Rect::UNIT).then_some(expected), "{window:?} in {base:?}");
        assert_eq!(Geometry { crop_bypassed: true, ..cropped }.measure_window_in_crop(), framed, "{window:?} bypassed");
        assert_eq!(Geometry { measure_window: None, ..cropped }.measure_window_in_crop(), None, "untouched");
    }
    // Twin: bypassing the crop reads the same window in the frame's own fractions, not the crop's.
    let window = rect(0.375, 0.25, 0.25, 0.5);
    let g = Geometry { crop: CROP, measure_window: Some(window), ..Geometry::default() };
    assert_eq!(Geometry { crop_bypassed: true, ..g }.measure_window_in_crop(), Some(window));
    assert_ne!(g.measure_window_in_crop(), Some(window));
}

#[test]
fn a_frame_proposes_its_crop_clear_of_a_tenth_and_a_sheet_the_whole_crop() {
    let p = proposal(Rect::UNIT, false);
    assert!((p.x - 0.1).abs() < 1e-12 && (p.y - 0.1).abs() < 1e-12 && (p.width - 0.8).abs() < 1e-12 && (p.height - 0.8).abs() < 1e-12, "{p:?}");
    assert_eq!(proposal(CROP, true), CROP);
    let mut g = Geometry { crop: CROP, ..Geometry::default() };
    g.propose_measure_window(false);
    assert_eq!(g.measure_window, Some(proposal(CROP, false)));
    // Twin: a frame's proposal is not the sheet's, and a placed window stays where it is.
    assert_ne!(proposal(CROP, false), CROP);
    let placed = rect(0.3, 0.3, 0.1, 0.1);
    let mut kept = Geometry { measure_window: Some(placed), ..Geometry::default() };
    kept.propose_measure_window(true);
    assert_eq!(kept.measure_window, Some(placed));
}
