//! The Auto levels key on a slide: read once at rest gains, each channel's 10 % and 90 % aligned, then
//! one black and one white; a negative still gets Classic's steps.

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

/// Where `x` of channel `c` lands through the slide's windows, 0 on its black and 1 on its white.
fn through(l: &[oni_core::balance::Suggestion; 3], c: usize, x: f32) -> f32 {
    (x - l[c].black) / (l[c].white - l[c].black)
}

/// Channel `c`'s share `q` of its counts, on the 0…1 window.
fn share(h: &Histogram, c: usize, q: f32) -> f32 {
    auto_levels::quantile(&h.channel(c), q).unwrap() / (BINS - 1) as f32
}

/// Each channel's 10 % and 90 % land on the same grey, and the aligned channels' outermost ends on
/// the one black and the one white.
fn aligned(h: &Histogram, l: &[oni_core::balance::Suggestion; 3]) -> bool {
    let at = |q: f32| [0, 1, 2].map(|c| through(l, c, share(h, c, q)));
    let same = |v: [f32; 3]| (v[0] - v[1]).abs() < 2e-3 && (v[1] - v[2]).abs() < 2e-3;
    same(at(0.10)) && same(at(0.90))
}

#[test]
fn auto_levels_on_a_slide_aligns_its_chroma_then_lays_one_black_and_white_with_its_gains_at_rest() {
    let (b, asked) = pressed(POSITIVE);
    assert_eq!(asked, vec![[0.0; 3]], "read once, at rest gains");
    assert_eq!(b.stops, [0.0; 3]);
    let h = reading([0.0; 3], POSITIVE);
    let l = b.levels;
    assert!(aligned(&h, &l), "the warm cast aligned away: {l:?}");
    let ends = h.ends.clone().unwrap();
    let black = (0..3).map(|c| through(&l, c, ends[c][0])).fold(f32::INFINITY, f32::min);
    let white = (0..3).map(|c| through(&l, c, ends[c][1])).fold(f32::NEG_INFINITY, f32::max);
    assert!(black.abs() < 1e-3 && (white - 1.0).abs() < 1e-3, "one black and one white: {black} {white}");
    let on = levelled(&b.core(), &frame(POSITIVE));
    assert_eq!(on.gains.stops, [0.0; 3], "an earlier balance's gains dropped");
    assert!(on.roll_reference.is_none() && on.balance_method.is_none(), "a key, never a roll");
    // Twin: the composite's one window on every channel keeps the cast.
    let composite = auto_levels::composite(&h, 0.0).unwrap();
    assert!(!aligned(&h, &composite));

    // Twin: the same frame as a negative brings red and blue onto green, each on its own window.
    let (n, asked) = pressed(NEGATIVE);
    assert_eq!(asked[0], frame(NEGATIVE).gains.stops, "a negative starts from the frame's gains");
    assert!(n.stops[0] != 0.0 && n.stops[2] != 0.0, "{:?}", n.stops);
    assert!(n.levels[0] != n.levels[2], "{:?}", n.levels);
}

/// A specular glint in red past its 90 % pins red's white yet never its chroma: the 10 % and 90 %
/// stay grey, and the glint, aligned, sets the one white.
#[test]
fn a_slide_s_chroma_rests_on_its_10_and_90_percent_never_on_a_glint() {
    let mut h = reading([0.0; 3], POSITIVE);
    let glint = (0.95 / BIN).round() as usize;
    h.rgb[glint][0] = 4.0;
    h.ends = None;
    let l = auto_levels::slide_points(&h).unwrap();
    assert!(aligned(&h, &l), "{l:?}");
    assert!((through(&l, 0, glint as f32 * BIN) - 1.0).abs() < 1e-3, "the glint is the white: {l:?}");
    // Twin: each channel on its own 0 % and 100 % lets the glint tint every light.
    let ends = auto_levels::placement(&h, 0.0).unwrap();
    assert!(!aligned(&h, &ends));
    assert!((through(&ends, 0, share(&h, 0, 0.9)) - through(&ends, 1, share(&h, 1, 0.9))).abs() > 0.02);
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
