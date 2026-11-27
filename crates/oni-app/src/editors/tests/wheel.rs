//! The colour wheel's, the saturation's and the bands' gestures on a real store: selection, keys, resets and focus.

use super::*;

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
    g.release(&mut store);
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
    let next = lay(store.settings(), &Edit::Spectrogram(s));
    store.apply(Command::Write(Box::new(next)));
    let wheel = spectrogram::Wheel { side: WHEEL.w };
    let tip = wheel.position(spectrogram::Grip { band: Band::Red, part: spectrogram::Part::Tip }, &s);
    let t0 = Instant::now();
    g.press(&mut store, Plot::Wheel, tip, t0);
    g.release(&mut store);
    g.press(&mut store, Plot::Wheel, tip, t0 + Duration::from_millis(150));
    g.release(&mut store);
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
    g.release(&mut store);
    let raised = store.settings().saturation.clone();
    assert!(raised.points[2].y > 0.7, "{:?}", raised.points[2]);
    // A double click on it, past the drag's interval, lays it back on 0.5.
    let at = saturation::screen(raised.points[2], PLOT);
    let t1 = t0 + Duration::from_secs(2);
    g.press(&mut store, Plot::Saturation, at, t1);
    g.release(&mut store);
    g.press(&mut store, Plot::Saturation, at, t1 + Duration::from_millis(120));
    g.release(&mut store);
    let back = &store.settings().saturation;
    assert_eq!(back.points[2].y, 0.5);
    assert!((back.points[2].x - raised.points[2].x).abs() < 1e-9);
    // Twin: two slow presses leave the point where the pointer put it.
    let t2 = t1 + Duration::from_secs(2);
    let high = (at.0, at.1 - 20.0);
    g.press(&mut store, Plot::Saturation, high, t2);
    g.release(&mut store);
    g.press(&mut store, Plot::Saturation, high, t2 + DOUBLE_CLICK + Duration::from_millis(1));
    g.release(&mut store);
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
    let expected = track::value(asked, &(low..=high), track::Bend::Even);
    assert_eq!(store.settings().spectrogram.band(Band::Green), (0.0, expected, 0.0));
    assert_eq!(band_readout(1, expected), "+50%");
    assert_eq!(band_readout(0, -12.4), "−12°");
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
fn a_block_reset_is_one_write_back_to_neutral() {
    let (mut store, dir) = fresh("editors-reset");
    let mut g = gestures();
    g.press(&mut store, Plot::Curves, (150.0, 150.0), Instant::now());
    g.release(&mut store);
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
    g.release(&mut store);
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
