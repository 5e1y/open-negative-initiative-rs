//! A slide's key: its cells' cast brought to grey by the gains alone, what cannot speak of the light
//! left out, its windows at rest; a negative still gets Classic's steps.

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

/// A slide's cells: a magenta cast over its tones, a saturated red subject, clipped lights and the
/// sensor's black.
fn cells() -> Vec<[f32; 3]> {
    let mut c: Vec<[f32; 3]> = (0..60).map(|i| 0.2 + 0.008 * i as f32).map(|v| [v + 0.03, v - 0.03, v + 0.03]).collect();
    c.extend((0..30).map(|_| [0.8, 0.25, 0.2]));
    c.extend((0..10).map(|_| [0.995; 3]));
    c.extend((0..10).map(|_| [0.005; 3]));
    c
}

/// Where each channel's median of `kept` lands once `stops` moved it.
fn landed(kept: &[[f32; 3]], stops: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|k| {
        let mut v: Vec<f32> = kept.iter().map(|c| c[k]).collect();
        v.sort_by(f32::total_cmp);
        v[v.len() / 2] + shift(stops[k], POSITIVE)
    })
}

#[test]
fn a_slide_s_gains_bring_its_cast_to_grey_and_leave_out_what_cannot_speak_of_the_light() {
    let all = cells();
    let stops = oni_measure::slide_gains::gains(&all, POSITIVE).expect("cells vote");
    assert_eq!(stops[1], 0.0, "green holds");
    let cast = &all[..60];
    let at = landed(cast, stops);
    assert!((at[0] - at[1]).abs() < 2e-3 && (at[2] - at[1]).abs() < 2e-3, "the cast lands grey: {at:?}");
    let kept = oni_measure::slide_gains::voting(&all);
    assert!(kept.iter().all(|c| c.iter().all(|v| (0.02..=0.98).contains(v))), "no black, no clipped light votes");
    assert!(!kept.contains(&[0.8, 0.25, 0.2]), "the saturated subject votes not");
    // Twin: every cell voting, the red subject pulls the balance off the cast's grey.
    let medians = [0, 1, 2].map(|k| {
        let mut v: Vec<f32> = all.iter().map(|c| c[k]).collect();
        v.sort_by(f32::total_cmp);
        v[v.len() / 2]
    });
    let naive = [medians[1] - medians[0], 0.0, medians[1] - medians[2]].map(|t| auto_levels::stops_for_shift(t, POSITIVE));
    let off = landed(cast, naive);
    assert!((off[0] - off[1]).abs() > 0.02 || (off[2] - off[1]).abs() > 0.02, "{off:?}");
    // Nothing usable, nothing placed.
    assert_eq!(oni_measure::slide_gains::gains(&[[0.0; 3], [1.0; 3]], POSITIVE), None);
}

#[test]
fn a_slide_balanced_on_its_gains_keeps_its_windows_at_rest() {
    let b = Balance { stops: [-0.2, 0.0, 0.3], levels: auto_levels::resting(POSITIVE), shift: [0.0; 3] };
    let on = levelled(&b.core(), &frame(POSITIVE));
    let rest = PipelineSettings::rest(POSITIVE, true).levels;
    for (c, (got, want)) in on.levels.windows().iter().zip(rest.windows()).enumerate() {
        assert!((got.black - want.black).abs() < 1e-4 && (got.white - want.white).abs() < 1e-4, "channel {c}'s window at rest: {got:?}");
    }
    assert_eq!(on.gains.stops, [-0.2, 0.0, 0.3]);
}

#[test]
fn a_negative_s_key_is_classic_s_two_steps() {
    let (n, asked) = pressed(NEGATIVE);
    assert_eq!(asked[0], frame(NEGATIVE).gains.stops, "a negative starts from the frame's gains");
    let stops = frame(NEGATIVE).gains.stops;
    let classic = auto_levels::two_steps(&reading(stops, NEGATIVE), stops, NEGATIVE, 0.0, |s| reading(s, NEGATIVE)).unwrap();
    assert_eq!(n, classic);
    // Twin: the negative's gains move red and blue onto green, each channel on its own window.
    assert!(n.stops[0] != 0.0 && n.stops[2] != 0.0 && n.levels[0] != n.levels[2], "{n:?}");
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
