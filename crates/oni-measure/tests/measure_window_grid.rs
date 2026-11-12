//! Both copies of the measuring window's rule agree on a grid of rects, degenerate ones included.

use oni_measure::window;
use oni_settings::cg::Rect;
use oni_settings::frame::Geometry;
use oni_settings::geometry;

fn rects() -> Vec<Rect> {
    let at = [-0.2, 0.0, 0.3, 1.0, f64::NAN];
    let side = [0.0, 0.4, 1.0, -0.1, f64::NAN];
    let mut all = Vec::new();
    for x in at {
        for y in at {
            for w in side {
                for h in side {
                    all.push(Rect::new(x, y, w, h));
                }
            }
        }
    }
    all
}

fn same(a: Rect, b: Rect) -> bool {
    [(a.x, b.x), (a.y, b.y), (a.width, b.width), (a.height, b.height)].iter().all(|(p, q)| p.to_bits() == q.to_bits() || (p.is_nan() && q.is_nan()))
}

/// The settings' former form, which computed through a side not a number.
fn computing_through(rect: Rect, base: Rect) -> Rect {
    let cut = window::intersection(&rect, &base);
    if base.width <= 0.0 || base.height <= 0.0 || cut.width <= 0.0 || cut.height <= 0.0 {
        return Rect::UNIT;
    }
    Rect::new((cut.x - base.x) / base.width, (cut.y - base.y) / base.height, cut.width / base.width, cut.height / base.height)
}

#[test]
fn both_copies_agree_on_every_rect_of_the_grid() {
    let grid = rects();
    let mut apart = 0;
    for &base in &grid {
        for sheet in [false, true] {
            assert!(same(window::proposal(&base, sheet), geometry::proposal(base, sheet)), "proposal {base:?}");
            let (mut a, mut b) = (Geometry { crop: base, ..Geometry::default() }, Geometry { crop: base, ..Geometry::default() });
            window::propose(&mut a, sheet);
            b.propose_measure_window(sheet);
            assert!(same(a.measure_window.unwrap(), b.measure_window.unwrap()), "propose {base:?}");
        }
        for &rect in &grid {
            assert!(same(window::relative(&rect, &base), geometry::relative(rect, base)), "relative {rect:?} in {base:?}");
            assert!(same(window::absolute(&rect, &base), geometry::absolute(rect, base)), "absolute {rect:?} in {base:?}");
            for bypassed in [false, true] {
                let g = Geometry { crop: base, crop_bypassed: bypassed, measure_window: Some(rect), ..Geometry::default() };
                match (window::in_crop(&g), g.measure_window_in_crop()) {
                    (Some(p), Some(q)) => assert!(same(p, q), "in crop {rect:?} in {base:?}"),
                    (p, q) => assert_eq!(p.is_none(), q.is_none(), "in crop {rect:?} in {base:?}"),
                }
            }
            if !same(window::relative(&rect, &base), computing_through(rect, base)) {
                apart += 1;
            }
        }
        let untouched = Geometry { crop: base, ..Geometry::default() };
        assert_eq!(window::in_crop(&untouched), untouched.measure_window_in_crop());
    }
    // Twin: the settings' former relative parts from the measure's on a base not a number.
    assert!(apart > 0);
}
