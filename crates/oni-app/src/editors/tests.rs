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

mod wheel;

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
    store.apply(Command::Undo);
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
    assert_eq!(g.release(&mut store), Then::Released);
    let p = store.settings().manual_curves.linked.points[1];
    assert_eq!((p.x, p.y), (0.5, 1.0 - 90.0 / 300.0), "the point lands where the pointer left it");
    // One undo takes the whole press back.
    undo(&mut store);
    assert!(store.settings().manual_curves.is_neutral());
    assert!(!store.can_undo(), "the press and its drag were one step");
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
        let next = lay(store.settings(), &Edit::Curves(curves.clone()));
        store.apply(Command::Write(Box::new(next)));
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
    g.release(&mut store);
    rested(&mut store);
    assert_eq!(store.settings().manual_curves.linked.points.len(), 3);
    // Two presses on the disc within the double-click interval: the second's release removes it.
    // The second lands inside the settle delay, so both presses are one step.
    let late = t0 + DOUBLE_CLICK + Duration::from_millis(50);
    g.press(&mut store, Plot::Curves, (151.0, 150.0), late);
    g.release(&mut store);
    g.press(&mut store, Plot::Curves, (151.0, 150.0), late + Duration::from_millis(120));
    g.release(&mut store);
    assert_eq!(store.settings().manual_curves.linked.points.len(), 2, "the double click removed the point");
    undo(&mut store);
    let points = &store.settings().manual_curves.linked.points;
    assert_eq!((points.len(), points[1].x), (3, 0.5), "one undo takes back the removal and the first press's move");
    // Twin: the delay running out between the two presses leaves the move a step of its own.
    let again = late + Duration::from_secs(2);
    g.press(&mut store, Plot::Curves, (151.0, 150.0), again);
    g.release(&mut store);
    rested(&mut store);
    g.press(&mut store, Plot::Curves, (151.0, 150.0), again + Duration::from_millis(300));
    g.release(&mut store);
    assert_eq!(store.settings().manual_curves.linked.points.len(), 2);
    undo(&mut store);
    let points = &store.settings().manual_curves.linked.points;
    assert_eq!((points.len(), points[1].x), (3, 151.0 / 300.0), "the undo stops at the first press's move");
    // Twin: the same two presses past the interval only move the point to the pointer.
    let slow = again + Duration::from_secs(2);
    g.press(&mut store, Plot::Curves, (152.0, 150.0), slow);
    g.release(&mut store);
    g.press(&mut store, Plot::Curves, (152.0, 150.0), slow + DOUBLE_CLICK + Duration::from_millis(1));
    g.release(&mut store);
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
    g.release(&mut store);
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
    g.release(&mut store);
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
    g.release(&mut store);
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
    g.release(&mut store);
    g.press(&mut store, Plot::Levels, (x, 17.0), again + Duration::from_millis(100));
    g.release(&mut store);
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
    let mut next = lay(store.settings(), &Edit::Levels(levels::Windows(set)));
    next.manual_curves = curves;
    store.apply(Command::Write(Box::new(next)));
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
fn notches_inside_the_settle_delay_are_one_step_and_rested_ones_are_two() {
    let (mut store, dir) = fresh("editors-notches");
    let mut g = gestures();
    let rest = store.settings().spectrogram;
    // Two accessibility steps without a rest between: the spectrogram never coalesces, so only
    // the delay can make them one step.
    g.band_step(&mut store, 1, 1);
    g.band_step(&mut store, 1, 1);
    assert!(!store.can_undo(), "the step waits for the edit to rest");
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
