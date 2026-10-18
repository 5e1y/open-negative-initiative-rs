//! The editors' gestures on a real store: the exact values written, the writes the hand's cadence
//! lets through, and the history steps each press leaves.

use super::geometry::Size;
use super::*;
use crate::store::tests::fresh;
use oni_core::EditorStore;
use oni_settings::cg::Point;
use oni_settings::levels::{LevelsChannel, LevelsSet};
use oni_settings::spectrogram::Band;
use std::time::{Duration, Instant};

const SQUARE: Size = Size::new(300.0, 300.0);
const TRACK: Size = Size::new(308.0, 34.0);
const WHEEL: Size = Size::new(308.0, 308.0);
const PLOT: Size = Size::new(308.0, 120.0);

fn gestures() -> Gestures {
    let mut g = Gestures::default();
    g.sizes[Plot::Curves as usize] = SQUARE;
    g.sizes[Plot::Levels as usize] = TRACK;
    g.sizes[Plot::Wheel as usize] = WHEEL;
    g.sizes[Plot::Saturation as usize] = PLOT;
    g
}

/// The window's timers: a scheduled trailing hand-off fires once its delay has run.
struct Clock {
    now: Instant,
    due: Option<Instant>,
}

impl Clock {
    fn new() -> Clock {
        Clock { now: Instant::now(), due: None }
    }

    fn tick(&mut self, g: &mut Gestures, store: &mut EditorStore, by: Duration) {
        self.now += by;
        if let Some(due) = self.due
            && self.now >= due
        {
            self.due = None;
            g.trailed(store, due);
        }
    }

    fn follow(&mut self, then: Then) {
        if let Then::Trail(delay) = then {
            self.due = Some(self.now + delay);
        }
    }
}

/// The settle delay running out: the window's timer records the step a lifted hand left waiting.
fn rested(store: &mut EditorStore) {
    EditorStore::release(store);
}

/// An undo once the last edit has rested, as a hand reaching for the key does.
fn undo(store: &mut EditorStore) {
    rested(store);
    crate::store::Store::apply(store, Command::Undo);
}

#[test]
fn a_curve_drag_writes_at_the_hand_s_pace_and_undoes_in_one_step() {
    let (mut store, dir) = fresh("editors-curve-drag");
    let mut g = gestures();
    let mut clock = Clock::new();
    // A press away from both ends adds a point there and writes it at once.
    let then = g.press(&mut store, Plot::Curves, (150.0, 150.0), clock.now);
    assert_eq!(then, Then::Publish);
    assert_eq!(g.writes, 1);
    assert_eq!(store.settings().manual_curves.linked.points.len(), 3);
    // Sixty moves a millisecond apart: the store takes one value per hand interval.
    let mut moves = 0;
    for k in 1..=60 {
        clock.tick(&mut g, &mut store, Duration::from_millis(1));
        let then = g.drag(&mut store, Plot::Curves, (150.0, 150.0 - k as f64), false, clock.now);
        clock.follow(then);
        moves += 1;
    }
    let ceiling = 1 + (60.0 / (HAND_MS)).ceil() as usize + 1;
    assert!(g.writes > 1 && g.writes <= ceiling, "{} writes for {moves} moves, at most {ceiling}", g.writes);
    // Twin: a write per move would have reached the store sixty times.
    assert!(g.writes < moves);
    assert_eq!(g.release(&mut store, Plot::Curves), Then::Released);
    let p = store.settings().manual_curves.linked.points[1];
    assert_eq!((p.x, p.y), (0.5, 1.0 - 90.0 / 300.0), "the point lands where the pointer left it");
    // One undo takes the whole press back.
    undo(&mut store);
    assert!(store.settings().manual_curves.is_neutral());
    assert!(!crate::store::Store::can_undo(&store), "the press and its drag were one step");
    let _ = std::fs::remove_dir_all(&dir);
}

const HAND_MS: f64 = crate::track::HAND_INTERVAL.as_nanos() as f64 / 1e6;

#[test]
fn without_the_hold_each_curve_write_would_be_its_own_step() {
    // Twin of the drag: the curves never coalesce, so the hold alone makes a press one step.
    let (mut store, dir) = fresh("editors-curve-twin");
    let mut curves = store.settings().manual_curves.clone();
    for k in 1..=3 {
        curves.linked.add(Point::new(0.2 * k as f64, 0.5));
        let next = lay(store.settings(), false, &Edit::Curves(curves.clone()));
        crate::store::Store::apply(&mut store, Command::Write(Box::new(next)));
    }
    undo(&mut store);
    assert_eq!(store.settings().manual_curves.linked.points.len(), 4, "one undo took one write back");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_double_click_on_a_curve_point_removes_it_and_a_slow_one_does_not() {
    let (mut store, dir) = fresh("editors-curve-double");
    let mut g = gestures();
    let t0 = Instant::now();
    g.press(&mut store, Plot::Curves, (150.0, 150.0), t0);
    g.release(&mut store, Plot::Curves);
    rested(&mut store);
    assert_eq!(store.settings().manual_curves.linked.points.len(), 3);
    // Two presses on the disc within the double-click interval: the second's release removes it.
    // The second lands inside the settle delay, so both presses are one step.
    let late = t0 + DOUBLE_CLICK + Duration::from_millis(50);
    g.press(&mut store, Plot::Curves, (151.0, 150.0), late);
    g.release(&mut store, Plot::Curves);
    g.press(&mut store, Plot::Curves, (151.0, 150.0), late + Duration::from_millis(120));
    g.release(&mut store, Plot::Curves);
    assert_eq!(store.settings().manual_curves.linked.points.len(), 2, "the double click removed the point");
    undo(&mut store);
    let points = &store.settings().manual_curves.linked.points;
    assert_eq!((points.len(), points[1].x), (3, 0.5), "one undo takes back the removal and the first press's move");
    // Twin: the delay running out between the two presses leaves the move a step of its own.
    let again = late + Duration::from_secs(2);
    g.press(&mut store, Plot::Curves, (151.0, 150.0), again);
    g.release(&mut store, Plot::Curves);
    rested(&mut store);
    g.press(&mut store, Plot::Curves, (151.0, 150.0), again + Duration::from_millis(300));
    g.release(&mut store, Plot::Curves);
    assert_eq!(store.settings().manual_curves.linked.points.len(), 2);
    undo(&mut store);
    let points = &store.settings().manual_curves.linked.points;
    assert_eq!((points.len(), points[1].x), (3, 151.0 / 300.0), "the undo stops at the first press's move");
    // Twin: the same two presses past the interval only move the point to the pointer.
    let slow = again + Duration::from_secs(2);
    g.press(&mut store, Plot::Curves, (152.0, 150.0), slow);
    g.release(&mut store, Plot::Curves);
    g.press(&mut store, Plot::Curves, (152.0, 150.0), slow + DOUBLE_CLICK + Duration::from_millis(1));
    g.release(&mut store, Plot::Curves);
    let points = &store.settings().manual_curves.linked.points;
    assert_eq!(points.len(), 3);
    assert_eq!(points[1].x, 152.0 / 300.0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn each_curve_channel_is_edited_alone() {
    let (mut store, dir) = fresh("editors-curve-channel");
    let mut g = gestures();
    g.pick(0, 2);
    assert_eq!(g.curve_channel(), LevelsChannel::Red);
    g.press(&mut store, Plot::Curves, (150.0, 100.0), Instant::now());
    g.release(&mut store, Plot::Curves);
    let curves = &store.settings().manual_curves;
    assert_eq!(curves.red.points.len(), 3);
    assert!(curves.linked.is_neutral() && curves.green.is_neutral(), "only the picked channel moved");
    assert_eq!(curve::marked(curves), vec![false, false, true, false, false]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_levels_handle_moves_by_the_pointer_s_travel_and_a_double_click_resets_it_alone() {
    let (mut store, dir) = fresh("editors-levels");
    let mut g = gestures();
    let mode = store.settings().mode;
    let channel = LevelsChannel::Red;
    let window = store.settings().levels.get(channel);
    let white = levels::knob_x(levels::Grip::White, &window, channel, mode, TRACK.w);
    // Caught five points left of its centre, then dragged 30 points left.
    let t0 = Instant::now();
    assert_eq!(g.press(&mut store, Plot::Levels, (white - 5.0, 17.0), t0), Then::Draw);
    g.drag(&mut store, Plot::Levels, (white - 35.0, 17.0), false, t0 + Duration::from_millis(40));
    g.release(&mut store, Plot::Levels);
    let start = levels::place(levels::Grip::White, &window, channel, mode);
    let asked = track::clamped(start - 30.0 / TRACK.w as f32);
    let expected = levels::moved(levels::Grip::White, &window, asked, channel, mode);
    let got = store.settings().levels.get(channel);
    assert_eq!(got, expected, "the handle moved by the travel from where it was caught");
    // Twin: an absolute drag would have put the handle under the pointer, five points off.
    let absolute = levels::moved(levels::Grip::White, &window, ((white - 35.0) / TRACK.w) as f32, channel, mode);
    assert_ne!(got.white, absolute.white);
    // Option slows the hand to a fifth.
    let moved = got;
    let x = levels::knob_x(levels::Grip::White, &moved, channel, mode, TRACK.w);
    let later = t0 + Duration::from_secs(2);
    g.press(&mut store, Plot::Levels, (x, 17.0), later);
    g.drag(&mut store, Plot::Levels, (x - 50.0, 17.0), true, later + Duration::from_millis(40));
    g.release(&mut store, Plot::Levels);
    let fine = levels::moved(
        levels::Grip::White,
        &moved,
        track::clamped(levels::place(levels::Grip::White, &moved, channel, mode) - 50.0 / TRACK.w as f32 * track::FINE),
        channel,
        mode,
    );
    assert_eq!(store.settings().levels.get(channel), fine);
    // A double click on the white puts the white alone back at its rest.
    let black_before = store.settings().levels.get(channel).black;
    let x = levels::knob_x(levels::Grip::White, &fine, channel, mode, TRACK.w);
    let again = later + Duration::from_secs(2);
    g.press(&mut store, Plot::Levels, (x, 17.0), again);
    g.release(&mut store, Plot::Levels);
    g.press(&mut store, Plot::Levels, (x, 17.0), again + Duration::from_millis(100));
    g.release(&mut store, Plot::Levels);
    let rest = LevelsSet::resting(channel, mode, store.settings().density_ceiling);
    let after = store.settings().levels.get(channel);
    assert_eq!(after.white, rest.white);
    assert_eq!(after.black, black_before);
    // Twin: a press off every handle catches nothing and writes nothing.
    let writes = g.writes;
    assert_eq!(g.press(&mut store, Plot::Levels, (TRACK.w / 2.0 + 40.0, 17.0), again + Duration::from_secs(2)), Then::Nothing);
    assert_eq!(g.writes, writes);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_scroll_along_the_track_moves_the_nearest_handle_and_one_across_is_left_to_the_panel() {
    let (mut store, dir) = fresh("editors-levels-scroll");
    let mut g = gestures();
    let mode = store.settings().mode;
    let channel = LevelsChannel::Red;
    let window = store.settings().levels.get(channel);
    let black = levels::knob_x(levels::Grip::Black, &window, channel, mode, TRACK.w);
    assert_eq!(g.scroll(&mut store, Plot::Levels, black + 2.0, 40.0, 0.0, false), Some(Then::Publish));
    let asked = track::clamped(levels::place(levels::Grip::Black, &window, channel, mode) + 40.0 * TRACKPAD_TRAVEL / TRACK.w as f32);
    assert_eq!(store.settings().levels.get(channel), levels::moved(levels::Grip::Black, &window, asked, channel, mode));
    // Twin: a scroll mostly across the track is the panel's.
    assert_eq!(g.scroll(&mut store, Plot::Levels, black, 4.0, 10.0, false), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_levels_reset_puts_back_the_three_windows_and_nothing_else() {
    let (mut store, dir) = fresh("editors-levels-reset");
    let mut g = gestures();
    let mut set = store.settings().levels;
    let mut red = set.get(LevelsChannel::Red);
    red.white = 0.8;
    set.set(LevelsChannel::Red, red);
    let mut curves = store.settings().manual_curves.clone();
    curves.linked.add(Point::new(0.5, 0.6));
    let mut next = lay(store.settings(), false, &Edit::Levels(set));
    next.manual_curves = curves;
    crate::store::Store::apply(&mut store, Command::Write(Box::new(next)));
    g.reset(&mut store, Reset::Levels);
    let after = store.settings().levels;
    let mode = store.settings().mode;
    let ceiling = store.settings().density_ceiling;
    assert_eq!(after.get(LevelsChannel::Red), LevelsSet::resting(LevelsChannel::Red, mode, ceiling));
    assert_eq!(after, LevelsSet::neutral(mode, ceiling), "no roll reference: the bare axis");
    // Twin: what is not the block's stays as the hand left it.
    assert_eq!(store.settings().manual_curves.linked.points.len(), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_wheel_selects_on_a_press_and_moves_by_the_travel_from_where_it_grabbed() {
    let (mut store, dir) = fresh("editors-wheel");
    let mut g = gestures();
    let wheel = spectrogram::Wheel { side: WHEEL.w };
    let rest = store.settings().spectrogram;
    let orange = wheel.position(spectrogram::Grip { band: Band::Orange, part: spectrogram::Part::Tip }, &rest);
    let t0 = Instant::now();
    // A press alone selects the nearest point and writes nothing.
    let at = (orange.0 + 3.0, orange.1 + 2.0);
    assert_eq!(g.press(&mut store, Plot::Wheel, at, t0), Then::Draw);
    assert_eq!(g.selected, spectrogram::Grip { band: Band::Orange, part: spectrogram::Part::Tip });
    assert!(g.focused && g.active().is_some());
    assert_eq!(store.settings().spectrogram, rest);
    g.drag(&mut store, Plot::Wheel, (at.0 + 25.0, at.1 - 10.0), false, t0 + Duration::from_millis(40));
    g.release(&mut store, Plot::Wheel);
    let grab = spectrogram::grab(wheel, at, &rest);
    let expected = spectrogram::dragged(wheel, grab, (25.0, -10.0), &rest);
    assert_eq!(store.settings().spectrogram, expected);
    // Twin: jumped to the pointer, the point would read the pointer's place, three points off.
    let jumped = spectrogram::dragged(wheel, spectrogram::Grab { grip: grab.grip, origin: at }, (25.0, -10.0), &rest);
    assert_ne!(store.settings().spectrogram, jumped);
    undo(&mut store);
    assert_eq!(store.settings().spectrogram, rest, "the drag was one step");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_wheel_s_keys_step_the_selected_point_and_tab_walks_it_only_while_it_holds_the_focus() {
    let (mut store, dir) = fresh("editors-wheel-keys");
    let mut g = gestures();
    let up: slint::SharedString = slint::platform::Key::UpArrow.into();
    let tab: slint::SharedString = slint::platform::Key::Tab.into();
    // Twin first: unfocused, the keys go on to the window.
    assert_eq!(g.key(&mut store, &up, false), None);
    g.focused = true;
    assert_eq!(g.key(&mut store, &up, false), Some(Then::Publish));
    let (_, saturation, _) = store.settings().spectrogram.band(Band::Red);
    assert!((saturation - 0.04).abs() < 1e-6, "two percent of the saturation's travel: {saturation}");
    assert_eq!(g.key(&mut store, &tab, false), Some(Then::Draw));
    assert_eq!(g.selected.band, Band::Magenta, "Tab walks clockwise");
    g.key(&mut store, &tab, true);
    g.key(&mut store, &tab, true);
    assert_eq!(g.selected.band, Band::Orange, "Option-Tab walks the other way");
    assert_eq!(g.key(&mut store, "a", false), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_double_click_on_the_wheel_resets_the_point_s_own_axes() {
    let (mut store, dir) = fresh("editors-wheel-double");
    let mut g = gestures();
    let mut s = store.settings().spectrogram;
    s.set(Band::Red, 5.0, 0.4, 0.3);
    let next = lay(store.settings(), false, &Edit::Spectrogram(s));
    crate::store::Store::apply(&mut store, Command::Write(Box::new(next)));
    let wheel = spectrogram::Wheel { side: WHEEL.w };
    let tip = wheel.position(spectrogram::Grip { band: Band::Red, part: spectrogram::Part::Tip }, &s);
    let t0 = Instant::now();
    g.press(&mut store, Plot::Wheel, tip, t0);
    g.release(&mut store, Plot::Wheel);
    g.press(&mut store, Plot::Wheel, tip, t0 + Duration::from_millis(150));
    g.release(&mut store, Plot::Wheel);
    assert_eq!(store.settings().spectrogram.band(Band::Red), (0.0, 0.0, 0.3), "hue and saturation back, luminance kept");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_saturation_double_click_puts_its_point_back_on_neutral() {
    let (mut store, dir) = fresh("editors-saturation");
    let mut g = gestures();
    let rest = store.settings().saturation.clone();
    let middle = saturation::screen(rest.points[2], PLOT);
    let t0 = Instant::now();
    // A drag raises the middle point.
    g.press(&mut store, Plot::Saturation, middle, t0);
    g.drag(&mut store, Plot::Saturation, (middle.0, middle.1 - 30.0), false, t0 + Duration::from_millis(40));
    g.release(&mut store, Plot::Saturation);
    let raised = store.settings().saturation.clone();
    assert!(raised.points[2].y > 0.7, "{:?}", raised.points[2]);
    // A double click on it, past the drag's interval, lays it back on 0.5.
    let at = saturation::screen(raised.points[2], PLOT);
    let t1 = t0 + Duration::from_secs(2);
    g.press(&mut store, Plot::Saturation, at, t1);
    g.release(&mut store, Plot::Saturation);
    g.press(&mut store, Plot::Saturation, at, t1 + Duration::from_millis(120));
    g.release(&mut store, Plot::Saturation);
    let back = &store.settings().saturation;
    assert_eq!(back.points[2].y, 0.5);
    assert!((back.points[2].x - raised.points[2].x).abs() < 1e-9);
    // Twin: two slow presses leave the point where the pointer put it.
    let t2 = t1 + Duration::from_secs(2);
    let high = (at.0, at.1 - 20.0);
    g.press(&mut store, Plot::Saturation, high, t2);
    g.release(&mut store, Plot::Saturation);
    g.press(&mut store, Plot::Saturation, high, t2 + DOUBLE_CLICK + Duration::from_millis(1));
    g.release(&mut store, Plot::Saturation);
    assert!(store.settings().saturation.points[2].y > 0.5);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_band_slider_writes_its_axis_and_a_double_click_zeroes_it() {
    let (mut store, dir) = fresh("editors-band");
    let mut g = gestures();
    g.pick(2, 3);
    assert_eq!(g.selected.band, Band::Green);
    let t0 = Instant::now();
    let (double, _) = g.band_press(&mut store, 1, t0);
    assert!(!double);
    g.band_drag(&mut store, 1, 77.0, 308.0, false, t0 + Duration::from_millis(40));
    g.band_release(&mut store);
    let (low, high) = Spectrogram::SATURATION_RANGE;
    let asked = 0.5 + 77.0 / 308.0;
    let expected = track::value(asked, &(low..=high), None);
    assert_eq!(store.settings().spectrogram.band(Band::Green), (0.0, expected, 0.0));
    assert_eq!(band_readout(1, expected), "+50%");
    assert_eq!(band_readout(0, -12.4), "-12°");
    // Twin: a double click puts the axis back on zero instead of starting a drag.
    let t1 = t0 + Duration::from_secs(2);
    g.band_press(&mut store, 1, t1);
    g.band_release(&mut store);
    let (double, _) = g.band_press(&mut store, 1, t1 + Duration::from_millis(100));
    assert!(double);
    assert_eq!(store.settings().spectrogram.band(Band::Green), (0.0, 0.0, 0.0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_scanner_face_edits_its_own_finishing_and_keeps_pro_s() {
    let (mut store, dir) = fresh("editors-scanner");
    store.scanner_face = true;
    let mut g = gestures();
    g.press(&mut store, Plot::Curves, (150.0, 150.0), Instant::now());
    g.release(&mut store, Plot::Curves);
    let s = store.settings();
    assert_eq!(s.scanner_finish.manual_curves.linked.points.len(), 3);
    // Twin: Pro's own curves are untouched by a gesture on the other face.
    assert!(s.manual_curves.is_neutral());
    assert_eq!(g.view(&mut store).manual_curves.linked.points.len(), 3, "the face draws its own");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_block_reset_is_one_write_back_to_neutral() {
    let (mut store, dir) = fresh("editors-reset");
    let mut g = gestures();
    g.press(&mut store, Plot::Curves, (150.0, 150.0), Instant::now());
    g.release(&mut store, Plot::Curves);
    rested(&mut store);
    let writes = g.writes;
    g.reset(&mut store, Reset::Curves);
    assert_eq!(g.writes, writes + 1);
    assert!(store.settings().manual_curves.is_neutral());
    undo(&mut store);
    assert!(!store.settings().manual_curves.is_neutral(), "the reset was one step");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_press_off_the_wheel_releases_its_focus_and_one_on_it_keeps_it() {
    let (mut store, dir) = fresh("editors-wheel-release");
    let mut g = gestures();
    g.origins[Plot::Wheel as usize] = (40.0, 500.0);
    assert!(!g.pressed_anywhere((0.0, 0.0)), "an unfocused wheel has nothing to release");
    g.press(&mut store, Plot::Wheel, (154.0, 154.0), Instant::now());
    g.release(&mut store, Plot::Wheel);
    assert!(g.active().is_some());
    // Twin: a press inside the wheel's own bounds, its corners included, keeps the point lit.
    assert!(!g.pressed_anywhere((40.0 + WHEEL.w, 500.0 + WHEEL.h)));
    assert!(g.active().is_some());
    assert!(g.pressed_anywhere((40.0 + WHEEL.w + 1.0, 500.0)));
    assert_eq!(g.active(), None, "the point unlit");
    let up: slint::SharedString = slint::platform::Key::UpArrow.into();
    assert_eq!(g.key(&mut store, &up, false), None, "the arrows went back to the window");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn notches_inside_the_settle_delay_are_one_step_and_rested_ones_are_two() {
    let (mut store, dir) = fresh("editors-notches");
    let mut g = gestures();
    let rest = store.settings().spectrogram;
    // Two accessibility steps without a rest between: the spectrogram never coalesces, so only
    // the delay can make them one step.
    g.band_step(&mut store, 1, 1);
    g.band_step(&mut store, 1, 1);
    assert!(!crate::store::Store::can_undo(&store), "the step waits for the edit to rest");
    undo(&mut store);
    assert_eq!(store.settings().spectrogram, rest, "one undo took both notches");
    // Twin: the delay running out between them leaves two steps.
    g.band_step(&mut store, 1, 1);
    rested(&mut store);
    let one = store.settings().spectrogram;
    g.band_step(&mut store, 1, 1);
    undo(&mut store);
    assert_eq!(store.settings().spectrogram, one);
    let _ = std::fs::remove_dir_all(&dir);
}
