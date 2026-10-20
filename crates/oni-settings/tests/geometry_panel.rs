//! `GeometryPanel.selfCheck`: the framing reset, the turns and mirror carrying the balance window,
//! the proposed window and the remembered framing, each with its twin.

use oni_settings::cg::Rect;
use oni_settings::frame::{AspectRatio, Geometry};
use oni_settings::geometry::{Driving, Framing, MEASURE_WINDOW_INSET, rotated_crop};

const SIZE: (f64, f64) = (8368.0, 5584.0);

fn close(a: Rect, b: Rect) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9 && (a.width - b.width).abs() < 1e-9 && (a.height - b.height).abs() < 1e-9
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect { x, y, width, height }
}

fn can_reset(g: &Geometry) -> bool {
    Geometry::reset_keeping(g) != *g
}

#[test]
fn one_click_resets_the_framing_and_keeps_the_balance_window() {
    let window = rect(0.3, 0.3, 0.2, 0.2);
    let set = Geometry { quarter_turns: 2, angle: 3.0, crop: rect(0.1, 0.2, 0.5, 0.4), ratio: Some(AspectRatio::ThreeTwo), portrait: true, measure_window: Some(window), ..Geometry::default() };
    assert!(can_reset(&set));
    let reset = Geometry::reset_keeping(&set);
    assert_eq!(Geometry { measure_window: None, ..reset.clone() }, Geometry::default(), "angle, turns, frame, ratio and portrait");
    assert_eq!(reset.measure_window, Some(window), "the measuring window has a reset of its own");
    // Twin: a bare rest drops that window.
    assert_eq!(Geometry::default().measure_window, None);
    assert!(!can_reset(&reset), "at rest the button turns off");
    // Twin: the former gesture, frame and ratio alone, leaves the rest set.
    let partial = Geometry { crop: Rect::UNIT, ratio: None, ..set.clone() };
    assert_ne!(partial, Geometry::default());
    let locked = Geometry { ratio: Some(AspectRatio::Square), ..Geometry::default() };
    assert!(can_reset(&locked) && locked.crop == Rect::UNIT, "a ratio locked on the full frame is something set");
    assert!(can_reset(&Geometry { angle: 1.0, ..Geometry::default() }) && can_reset(&Geometry { quarter_turns: 1, ..Geometry::default() }));
}

#[test]
fn a_quarter_turn_and_a_mirror_carry_the_balance_window_and_a_ratio_moves_the_crop_alone() {
    let start = Geometry { crop: rect(0.1, 0.2, 0.5, 0.4), ..Geometry::default() };
    let mut turning = start.clone();
    for _ in 0..3 {
        turning.rotate(1, SIZE);
    }
    // Twin: at three quarter turns the frame has not returned.
    assert!(turning.quarter_turns == 3 && !close(turning.crop, start.crop));
    turning.rotate(1, SIZE);
    assert!(turning.quarter_turns == 0 && close(turning.crop, start.crop));

    let drawn = rect(0.6, 0.1, 0.3, 0.2);
    let mut windowed = Geometry { measure_window: Some(drawn), ..start.clone() };
    windowed.rotate(1, SIZE);
    let turned = windowed.measure_window.unwrap();
    assert!(close(turned, rotated_crop(drawn)), "a turn carries the window as it carries the crop");
    assert!(!close(turned, drawn), "twin: the turned window has moved");
    windowed.mirror();
    let mirrored = windowed.measure_window.unwrap();
    assert!((mirrored.x - (1.0 - turned.x - turned.width)).abs() < 1e-9 && (mirrored.width - turned.width).abs() < 1e-9);
    let mut constrained = Geometry { ratio: Some(AspectRatio::Square), crop: rect(0.3, 0.3, 0.2, 0.3), ..windowed.clone() };
    constrained.apply_ratio(SIZE, None, Driving::Width);
    constrained.fit_crop_inside_image(SIZE);
    assert!(constrained.crop != windowed.crop && constrained.measure_window == windowed.measure_window);
}

#[test]
fn arming_proposes_a_window_inside_the_crop_once_and_a_remembered_framing_cuts_the_frame() {
    let mut following = Geometry { crop: rect(0.2, 0.2, 0.5, 0.4), ..Geometry::default() };
    assert!(following.measure_window.is_none() && following.measure_window_in_crop().is_none());
    following.propose_measure_window(false);
    let i = MEASURE_WINDOW_INSET;
    let proposed = following.measure_window.unwrap();
    assert!(close(proposed, rect(0.2 + 0.5 * i, 0.2 + 0.4 * i, 0.5 * (1.0 - 2.0 * i), 0.4 * (1.0 - 2.0 * i))), "{proposed:?}");
    following.crop = Rect::UNIT;
    following.propose_measure_window(false);
    assert_eq!(following.measure_window, Some(proposed), "a placed window stays whatever the crop became");
    // Twin: the proposal depends on the crop it is made in.
    let mut bare = Geometry::default();
    bare.propose_measure_window(false);
    assert_ne!(bare.measure_window, Some(proposed));

    let remembered = Framing::new(Some(AspectRatio::ThreeTwo), true).unwrap();
    let mut fresh = Geometry::default();
    fresh.adopt(remembered, SIZE);
    assert!(fresh.ratio == Some(AspectRatio::ThreeTwo) && fresh.portrait && fresh.crop != Rect::UNIT);
    let mut hand_made = Geometry { crop: rect(0.1, 0.1, 0.5, 0.5), ratio: Some(AspectRatio::Square), ..Geometry::default() };
    hand_made.adopt(remembered, SIZE);
    let ratio = hand_made.crop.width * SIZE.0 / (hand_made.crop.height * SIZE.1);
    assert!(hand_made.portrait && (ratio - 2.0 / 3.0).abs() < 0.01, "re-cut where it stands: {ratio}");
    assert_eq!(Framing::new(None, true), None, "a free frame is nothing to remember");
    // Twin: a frame already on the remembered ratio is left as it is.
    let mut same = Geometry { ratio: Some(AspectRatio::ThreeTwo), portrait: true, ..Geometry::default() };
    same.apply_ratio(SIZE, None, Driving::Width);
    let cut = same.clone();
    same.adopt(remembered, SIZE);
    assert_eq!(same, cut);
}
