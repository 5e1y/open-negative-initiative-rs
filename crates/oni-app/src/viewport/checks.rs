//! `Viewport.selfCheck` and `CanvasEvents.selfCheck`: what the unit tests beside the code leave, each
//! with its twin.

use super::*;

const RAW: Size = (8368.0, 5584.0);

/// A rung reached by multiplying the fitted scale, the shape that misses 1.0 by an ulp.
fn multiplied(target: f64, view: Size) -> f64 {
    let current = Viewport::default().effective_scale(RAW, view);
    clamped(current * (target / current))
}

#[test]
fn the_hundred_rung_lands_on_its_number_at_every_window_width() {
    let widths = 3800..=6000;
    let (mut lands, mut misses) = (0, 0);
    for w in widths.clone() {
        let window = (f64::from(w), (f64::from(w) * 2.0 / 3.0).round());
        let mut port = Viewport::default();
        port.step(Rung::Scale(1.0), RAW, window);
        lands += usize::from(port.scale == Some(1.0));
        let product = Viewport { scale: Some(multiplied(1.0, window)), offset: (0.0, 0.0) };
        misses += usize::from(product.scale != Some(1.0));
    }
    let count = widths.count();
    assert_eq!(lands, count, "the 100 % rung lands on its number at every width");
    // Twin: reached by multiplication it misses on some widths.
    assert!(misses > count / 20, "{misses} misses");
}

/// The rungs − or + reach from `from`, one a press, until the ladder ends, with the scale each shows.
fn walked(view: Size, delta: i32, from: Viewport) -> (Viewport, Vec<(Rung, f64)>) {
    let (mut port, mut path) = (from, Vec::new());
    while let Some(rung) = port.rung(delta, RAW, view) {
        port.step(rung, RAW, view);
        path.push((rung, port.effective_scale(RAW, view)));
    }
    (port, path)
}

#[test]
fn plus_climbs_rung_by_rung_and_minus_descends_through_the_same_values_to_fit() {
    assert!(ZOOM_STEPS.windows(2).all(|w| w[0] < w[1]), "ascending and distinct");
    assert!(ZOOM_STEPS.iter().all(|s| (RANGE.0..=RANGE.1).contains(s)) && ZOOM_STEPS.last() == Some(&RANGE.1));
    // Fitted at 21 %, under every rung, and at 35 %, between 25 % and 50 %.
    for (view, above) in [((1800.0, 1200.0), &ZOOM_STEPS[..]), ((8368.0 * 0.35, 5584.0 * 0.35), &ZOOM_STEPS[1..])] {
        let (top, climb) = walked(view, 1, Viewport::default());
        assert_eq!(climb.iter().map(|&(r, _)| r).collect::<Vec<_>>(), above.iter().map(|&s| Rung::Scale(s)).collect::<Vec<_>>());
        let (bottom, descent) = walked(view, -1, top);
        let mut expected: Vec<Rung> = above[..above.len() - 1].iter().rev().map(|&s| Rung::Scale(s)).collect();
        expected.push(Rung::Fit);
        assert_eq!(descent.iter().map(|&(r, _)| r).collect::<Vec<_>>(), expected);
        assert!(bottom.is_fitted());
        // Each − shows the photo smaller than the last, down to fit.
        assert!(descent.windows(2).all(|w| w[1].1 < w[0].1), "{descent:?}");
    }
    // From a free 62 %: + gives 100 %, − gives 50 %.
    let free = Viewport { scale: Some(0.62), offset: (0.0, 0.0) };
    let view = (1800.0, 1200.0);
    assert_eq!((free.rung(1, RAW, view), free.rung(-1, RAW, view)), (Some(Rung::Scale(1.0)), Some(Rung::Scale(0.5))));
    // Twin: "the nearest rung, then one step" would skip 50 % for 25 %.
    let nearest = (0..ZOOM_STEPS.len()).min_by(|&a, &b| (ZOOM_STEPS[a] - 0.62).abs().total_cmp(&(ZOOM_STEPS[b] - 0.62).abs())).unwrap();
    assert_ne!(free.rung(-1, RAW, view), Some(Rung::Scale(ZOOM_STEPS[nearest.saturating_sub(1)])));
    // Twin: a ladder blind to the fitted 35 % steps from 50 % under it, and its next − zooms in to fit.
    let (view, mut port) = ((8368.0 * 0.35, 5584.0 * 0.35), Viewport { scale: Some(0.5), offset: (0.0, 0.0) });
    let mut scales = vec![0.5];
    for _ in 0..2 {
        if let Some(rung) = next_rung(port.effective_scale(RAW, view), RANGE.0, -1) {
            port.step(rung, RAW, view);
            scales.push(port.effective_scale(RAW, view));
        }
    }
    assert!(!scales.windows(2).all(|w| w[1] < w[0]), "{scales:?}");
}

#[test]
fn a_rung_from_an_image_pushed_into_its_corner_keeps_it_inside_the_overflow() {
    let view = (1800.0, 1200.0);
    let mut port = Viewport::default();
    port.step(Rung::Scale(RANGE.1), RAW, view);
    port.pan((40_000.0, 40_000.0), RAW, view);
    port.step(Rung::Scale(0.25), RAW, view);
    let s = port.effective_scale(RAW, view);
    let limit = ((RAW.0 * s - view.0).max(0.0) / 2.0, (RAW.1 * s - view.1).max(0.0) / 2.0);
    assert!(port.offset.0.abs() <= limit.0 + 1e-3 && port.offset.1.abs() <= limit.1 + 1e-3, "{:?} past {limit:?}", port.offset);
    // Twin: the scale written by hand keeps the corner's offset, past the overflow.
    let mut by_hand = Viewport::default();
    by_hand.step(Rung::Scale(RANGE.1), RAW, view);
    by_hand.pan((40_000.0, 40_000.0), RAW, view);
    by_hand.scale = Some(0.25);
    assert!(by_hand.offset.0.abs() > limit.0 + 1.0);
}

#[test]
fn the_image_follows_the_hand_on_both_axes() {
    let view = (1801.0, 1201.0);
    let rest = Viewport::centre(view, (0.0, 0.0));
    // A pointer going down and right, top-origin as Slint gives it.
    let followed = Viewport::centre(view, pan_step(1.0, 1.0, 2.0));
    assert!(followed.0 > rest.0 && followed.1 > rest.1, "{followed:?} from {rest:?}");
    // Twin: the vertical negated climbs while the pointer descends.
    let fled = Viewport::centre(view, (2.0, -2.0));
    assert!(fled.1 < rest.1 && fled.0 > rest.0);
}

#[test]
fn each_device_s_zoom_dose_is_its_own_and_a_notch_undoes_its_opposite() {
    let fitted = Viewport::default().effective_scale(RAW, (1801.0, 1201.0));
    let events = |climb: f64, per: f64| climb.ln() / per.ln();
    let notches = events(1.0 / fitted, WHEEL_ZOOM_PER_NOTCH);
    assert!(notches > 8.0 && notches < 30.0, "{notches} notches from fit to 100 %");
    // Twin: the trackpad's dose on that wheel asks for hundreds of notches.
    let as_glide = events(1.0 / fitted, TRACKPAD_ZOOM_PER_POINT);
    assert!(!(as_glide > 8.0 && as_glide < 30.0));
    let per_glide = zoom_factor(100.0, true);
    assert!(per_glide > 1.05 && per_glide < 2.0, "100 pt of glide zoom ×{per_glide}");
    // One dose cannot serve both: the wheel's on that glide crosses the whole range at a stroke.
    assert!(WHEEL_ZOOM_PER_NOTCH.powf(100.0) > RANGE.1 / RANGE.0);
    assert!(zoom_factor(1.0, false) > 1.0 && zoom_factor(-1.0, false) < 1.0);
    assert!((zoom_factor(1.0, false) * zoom_factor(-1.0, false) - 1.0).abs() < 1e-12);
}

#[test]
fn a_pan_inside_the_margin_rendered_needs_no_render_and_one_past_it_does() {
    let (full, view) = ((6000.0, 4000.0), (1000.0, 800.0));
    let v = Viewport { scale: Some(0.5), offset: (0.0, 0.0) };
    // The view and an eighth of it rendered each side; a pan moves that picture with the hand.
    let rendered = (-125.0, -100.0, 1250.0, 1000.0);
    let moved = |dx: f64, dy: f64| (Viewport { offset: (dx, dy), ..v }.placement(full, view), (rendered.0 + dx, rendered.1 + dy, rendered.2, rendered.3));
    assert!(v.placement(full, view).covered_by(rendered));
    let (inside, laid) = moved(100.0, -80.0);
    assert!(inside.covered_by(laid), "a pan within the margin still shows only rendered pixels");
    // Twin: past the margin an edge of the view has nothing rendered under it.
    let (past, laid) = moved(200.0, 0.0);
    assert!(!past.covered_by(laid));
}

#[test]
fn a_fraction_of_a_line_on_windows_is_a_touch_surface_that_pans() {
    let touch = lines(0.25, -0.5, true);
    assert_eq!(touch, Wheel { dx: 15.0, dy: -30.0, precise: true });
    assert!(!zooms(touch.precise, false), "a two-finger scroll pans");
    // Twin: a wheel's whole notch still zooms, and a Mac's fractional lines stay a wheel's.
    let notch = lines(0.0, 1.0, true);
    assert!(!notch.precise && zooms(notch.precise, false));
    assert!(!lines(0.25, -0.5, false).precise);
}
