//! The Auto levels key on a slide: read once at rest gains, one black and one white on the RGB
//! composite laid on every channel, so its own colour stays; a negative still gets Classic's steps.

use oni_core::balance::levelled;
use oni_measure::auto_levels::{self, Balance, keyed, shift};
use oni_measure::histogram::Histogram;
use oni_measure::roll;
use oni_settings::frame::FlatField;
use oni_settings::{ConversionMode, PipelineSettings};

const POSITIVE: ConversionMode = ConversionMode::Positive;
const NEGATIVE: ConversionMode = ConversionMode::Negative;
const BINS: usize = 256;
const BIN: f32 = 1.0 / (BINS - 1) as f32;

/// A warm frame: each channel a flat band, red above green above blue, moved by its gain as the
/// density axis is.
fn reading(stops: [f32; 3], mode: ConversionMode) -> Histogram {
    let bands = [[0.30, 0.80], [0.22, 0.70], [0.15, 0.62]];
    let mut rgb = vec![[0.0f32; 3]; BINS];
    for c in 0..3 {
        let s = shift(stops[c], mode);
        for (i, b) in rgb.iter_mut().enumerate() {
            let x = i as f32 * BIN;
            if x >= bands[c][0] + s && x <= bands[c][1] + s {
                b[c] = 1.0;
            }
        }
    }
    let ends = (0..3)
        .map(|c| {
            let lit: Vec<f32> = (0..BINS).filter(|&i| rgb[i][c] > 0.0).map(|i| i as f32 * BIN).collect();
            [lit[0], lit[lit.len() - 1]]
        })
        .collect();
    Histogram { rgb, luma: Vec::new(), ends: Some(ends) }
}

/// A frame as the key finds it, gains an earlier balance left on it.
fn frame(mode: ConversionMode) -> PipelineSettings {
    let mut s = PipelineSettings::rest(mode, true);
    s.gains.stops = [0.4, 0.0, -0.7];
    s
}

fn pressed(mode: ConversionMode) -> (Balance, Vec<[f32; 3]>) {
    let mut asked = Vec::new();
    let b = keyed(frame(mode).gains.stops, mode, |s| {
        asked.push(s);
        reading(s, mode)
    })
    .expect("the frame reads");
    (b, asked)
}

#[test]
fn auto_levels_on_a_slide_lays_one_rgb_black_and_white_with_its_gains_at_rest() {
    let (b, asked) = pressed(POSITIVE);
    assert_eq!(asked, vec![[0.0; 3]], "read once, at rest gains");
    assert_eq!(b.stops, [0.0; 3]);
    let l = b.levels;
    assert!(l[0] == l[1] && l[1] == l[2], "{l:?}");
    // The composite's ends: the darkest channel's low, blue's, and the lightest's high, red's.
    assert!((l[0].black - 0.15).abs() <= BIN && (l[0].white - 0.80).abs() <= BIN, "{l:?}");
    let on = levelled(&b.core(), &frame(POSITIVE));
    assert_eq!(on.gains.stops, [0.0; 3], "an earlier balance's gains dropped");
    assert!(on.levels.red == on.levels.green && on.levels.green == on.levels.blue, "{:?}", on.levels);
    assert!(on.roll_reference.is_none() && on.balance_method.is_none(), "a key, never a roll");
    // One window on every channel maps each alike: the warm bands keep their order and gaps.
    let through = |x: f32| (x - l[0].black) / (l[0].white - l[0].black);
    assert!(through(0.55) - through(0.46) > 0.1 && through(0.46) - through(0.385) > 0.1);

    // Twin: the same frame as a negative brings red and blue onto green, each on its own window.
    let (n, asked) = pressed(NEGATIVE);
    assert_eq!(asked[0], frame(NEGATIVE).gains.stops, "a negative starts from the frame's gains");
    assert!(n.stops[0] != 0.0 && n.stops[2] != 0.0, "{:?}", n.stops);
    assert!(n.levels[0] != n.levels[2], "{:?}", n.levels);
}

#[test]
fn a_negative_s_key_is_classic_s_two_steps_and_a_slide_s_is_not() {
    let (n, _) = pressed(NEGATIVE);
    let stops = frame(NEGATIVE).gains.stops;
    let classic = auto_levels::two_steps(&reading(stops, NEGATIVE), stops, NEGATIVE, 0.0, |s| reading(s, NEGATIVE)).unwrap();
    assert_eq!(n, classic);
    // Twin: the slide's composite is not what Classic places on the same frame.
    let (slide, _) = pressed(POSITIVE);
    let rest = [0.0; 3];
    let classic = auto_levels::two_steps(&reading(rest, POSITIVE), rest, POSITIVE, 0.0, |s| reading(s, POSITIVE)).unwrap();
    assert_ne!(slide.levels, classic.levels);
}

#[test]
fn the_composite_takes_the_outermost_ends_and_the_summed_counts_without_them() {
    let mut h = reading([0.0; 3], POSITIVE);
    h.ends = Some(vec![[0.31, 0.79], [0.24, 0.69], [0.17, 0.61]]);
    let carried = auto_levels::composite(&h, 0.0).unwrap();
    assert_eq!((carried[0].black, carried[0].white), (0.17, 0.79));
    h.ends = None;
    let counted = auto_levels::composite(&h, 0.0).unwrap();
    assert!((counted[2].black - 0.15).abs() <= BIN && (counted[2].white - 0.80).abs() <= BIN, "{counted:?}");
    // Twin: per channel, red's own ends are not the composite's.
    let per = auto_levels::placement(&h, 0.0).unwrap();
    assert!((per[0].black - counted[0].black).abs() > 0.1);
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
