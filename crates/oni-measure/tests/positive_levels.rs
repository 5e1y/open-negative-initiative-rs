//! A slide's gains: the white picked brought to grey by them alone, a black or clipped one placing
//! nothing, its windows at rest.

use oni_core::balance::{Balance, levelled};
use oni_measure::auto_levels::{self, shift};
use oni_measure::roll;
use oni_settings::frame::FlatField;
use oni_settings::{ConversionMode, PipelineSettings};

const POSITIVE: ConversionMode = ConversionMode::Positive;
const NEGATIVE: ConversionMode = ConversionMode::Negative;

/// A frame as the key finds it, gains an earlier balance left on it.
fn frame(mode: ConversionMode) -> PipelineSettings {
    let mut s = PipelineSettings::rest(mode, true);
    s.gains.stops = [0.4, 0.0, -0.7];
    s
}

#[test]
fn a_picked_white_s_cast_is_brought_to_grey_by_the_gains_alone() {
    // A white under a warm light: red above green above blue on the counted window.
    let white = [0.74, 0.70, 0.61];
    let stops = oni_measure::slide_gains::white(white, POSITIVE).expect("the white speaks of the light");
    assert!(stops[0] < stops[1] && stops[1] < stops[2], "red down, blue up: {stops:?}");
    let at = [0, 1, 2].map(|k| white[k] + shift(stops[k], POSITIVE));
    assert!((at[0] - at[1]).abs() < 2e-3 && (at[2] - at[1]).abs() < 2e-3, "the white lands grey: {at:?}");
    // Its colour alone moves: the gains' mean holds, where green held still would lift it.
    assert!(stops.iter().sum::<f32>().abs() < 1e-5, "{stops:?}");
    let green_held = [0, 1, 2].map(|k| auto_levels::stops_for_shift(white[1] - white[k], POSITIVE));
    assert!(green_held.iter().sum::<f32>().abs() > 0.05, "{green_held:?}");
    // Twins: a clipped light or the sensor's black holds no colour of the light, and places nothing.
    assert_eq!(oni_measure::slide_gains::white([0.99, 0.97, 0.90], POSITIVE), None);
    assert_eq!(oni_measure::slide_gains::white([0.30, 0.01, 0.20], POSITIVE), None);
}

#[test]
fn a_slide_balanced_on_its_gains_keeps_its_windows_at_rest() {
    let b = Balance { stops: [-0.2, 0.0, 0.3], levels: auto_levels::resting(POSITIVE) };
    let on = levelled(&b, &frame(POSITIVE));
    let rest = PipelineSettings::rest(POSITIVE, true).levels;
    for (c, (got, want)) in on.levels.windows().iter().zip(rest.windows()).enumerate() {
        assert!((got.black - want.black).abs() < 1e-4 && (got.white - want.white).abs() < 1e-4, "channel {c}'s window at rest: {got:?}");
    }
    assert_eq!(on.gains.stops, [-0.2, 0.0, 0.3]);
}

#[test]
fn a_slide_joins_its_roll_on_its_window_unmeasured_its_earlier_roll_and_look_at_rest() {
    let mut before = PipelineSettings::rest(POSITIVE, true);
    before.gains.stops = [0.2, 0.0, -0.5];
    before.print.exposure = 0.7;
    before.roll_reference = Some(oni_settings::frame::RollReference {
        id: oni_settings::Id::new(),
        stops: [0.2, 0.0, -0.5],
        black: [0.2; 3],
        white: [0.8; 3],
        base: None,
        median: None,
    });
    let mask = FlatField { amount: 0.5, ..FlatField::default() };
    let window = oni_settings::cg::Rect::new(0.1, 0.1, 0.8, 0.8);
    let s = roll::slide(&before, window, Some(&mask), 0.3);
    let rest = PipelineSettings::rest(POSITIVE, true);
    assert_eq!((s.mode, s.roll_reference.as_ref(), s.gains, s.levels), (POSITIVE, None, rest.gains, rest.levels));
    assert_eq!((s.print.exposure, s.capture_stops, s.flat_field.amount), (rest.print.exposure, 0.3, 0.5));
    assert_eq!(s.geometry.measure_window, Some(window), "the roll's window laid on the slide");
    // Twin: a negative's start keeps its roll and lays the window to measure on.
    let mut negative = before.clone();
    negative.mode = NEGATIVE;
    let n = roll::start(&negative, NEGATIVE, window, Some(&mask), 0.3);
    assert!(n.roll_reference.is_some() && n.geometry.measure_window == Some(window));
}
