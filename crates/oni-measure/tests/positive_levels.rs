//! The Auto levels key on a slide: its cells at 10 % and 90 % of its tones brought to grey, three
//! channels read together, then one black and one white and its near-grey midtones snapped.

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

/// Where `x` of channel `c` lands through the slide's windows and mids, as the levels kernel lays it.
fn through(l: &[oni_core::balance::Suggestion; 3], c: usize, x: f32) -> f32 {
    let gamma = 0.5f32.ln() / l[c].mid.ln();
    ((x - l[c].black) / (l[c].white - l[c].black)).max(0.0).powf(gamma)
}

/// A slide's cells: magenta shadows, a red subject whose blue is the frame's darkest, mid greys and
/// bluish lights, each with its luma as a cell reads it.
fn scene() -> (Vec<[f32; 3]>, Vec<f32>) {
    let mut cells = Vec::new();
    cells.extend((0..40).map(|i| 0.10 + 0.002 * i as f32).map(|d| [d + 0.03, d - 0.03, d + 0.03]));
    cells.extend((0..20).map(|_| [0.60, 0.12, 0.04]));
    cells.extend((0..20).map(|i| 0.45 + 0.005 * i as f32).map(|m| [m, m, m]));
    cells.extend((0..20).map(|i| 0.80 + 0.003 * i as f32).map(|l| [l - 0.01, l, l + 0.03]));
    let w = oni_settings::axis::pipeline::LUMA_WEIGHTS;
    let luma = cells.iter().map(|p| p[0] * w[0] + p[1] * w[1] + p[2] * w[2]).collect();
    (cells, luma)
}

/// The cells' histogram as a reading counts it, each channel's ends its darkest and lightest cell.
fn counted(cells: &[[f32; 3]], luma: &[f32]) -> Histogram {
    let mut h = oni_measure::blocks::histogram(cells, luma, 1024);
    h.ends = Some((0..3).map(|c| [cells.iter().map(|p| p[c]).fold(1.0, f32::min), cells.iter().map(|p| p[c]).fold(0.0, f32::max)]).collect());
    h
}

/// Whether the cells `band` names land grey through the windows: their channels' medians as one.
fn grey(cells: &[[f32; 3]], band: std::ops::Range<usize>, l: &[oni_core::balance::Suggestion; 3]) -> bool {
    let median = |c: usize| {
        let mut v: Vec<f32> = cells[band.clone()].iter().map(|p| p[c]).collect();
        v.sort_by(f32::total_cmp);
        v[v.len() / 2]
    };
    let at = [0, 1, 2].map(|c| through(l, c, median(c)));
    (at[0] - at[1]).abs() < 5e-3 && (at[1] - at[2]).abs() < 5e-3
}

#[test]
fn a_slide_s_shadows_and_lights_are_read_together_on_its_cells_and_land_grey() {
    let (cells, luma) = scene();
    let h = counted(&cells, &luma);
    let l = auto_levels::slide_points(&h, &cells, &luma).expect("the slide reads");
    // The tones it reads: the shadows at 10 % of the span, the lights at 90 %, the grey midtones.
    let (shadows, lights) = (19..40, 80..87);
    assert!(grey(&cells, shadows.clone(), &l), "the magenta shadows land grey: {l:?}");
    assert!(grey(&cells, lights.clone(), &l), "the bluish lights land grey: {l:?}");
    assert!(grey(&cells, 60..80, &l), "the grey midtones land grey: {l:?}");
    let ends = h.ends.clone().unwrap();
    let black = (0..3).map(|c| through(&l, c, ends[c][0])).fold(f32::INFINITY, f32::min);
    let white = (0..3).map(|c| through(&l, c, ends[c][1])).fold(f32::NEG_INFINITY, f32::max);
    // Within a hundredth: the midtones' bend moves the extremes a little.
    assert!(black.abs() < 1e-2 && (white - 1.0).abs() < 1e-2, "one black and one white: {black} {white}");
    // Twins: each channel's own 10 % and 90 % (the red subject's blue read as the shadows'), each
    // channel on its own ends, and the composite's one window all leave the shadows magenta.
    let share = |c: usize, q: f32| auto_levels::quantile(&h.channel(c), q).unwrap() / 1023.0;
    let apart: [oni_core::balance::Suggestion; 3] = [0, 1, 2].map(|c| {
        let (lo, hi, glo, ghi) = (share(c, 0.1), share(c, 0.9), share(1, 0.1), share(1, 0.9));
        let back = |y: f32| lo + (y - glo) * (hi - lo) / (ghi - glo);
        oni_core::balance::Suggestion { black: back(0.0), white: back(1.0), ..l[c] }
    });
    assert!(!grey(&cells, shadows.clone(), &apart));
    assert!(!grey(&cells, shadows.clone(), &auto_levels::placement(&h, 0.0).unwrap()));
    assert!(!grey(&cells, shadows, &auto_levels::composite(&h, 0.0).unwrap()));
}

/// A scene whose midtones are all colour, a blue sea, keeps them: none stands near grey to snap.
#[test]
fn a_slide_whose_midtones_are_colour_keeps_them() {
    let (mut cells, _) = scene();
    for p in &mut cells[60..80] {
        *p = [0.30, 0.45, 0.75];
    }
    let w = oni_settings::axis::pipeline::LUMA_WEIGHTS;
    let luma: Vec<f32> = cells.iter().map(|p| p[0] * w[0] + p[1] * w[1] + p[2] * w[2]).collect();
    let l = auto_levels::slide_points(&counted(&cells, &luma), &cells, &luma).expect("the slide reads");
    assert!(l.iter().all(|e| e.mid == 0.5), "the sea is not a cast: {l:?}");
    assert!(!grey(&cells, 60..80, &l), "it stays blue");
    // Twin: the grey midtones of `scene` do snap.
    let (cells, luma) = scene();
    assert!(auto_levels::slide_points(&counted(&cells, &luma), &cells, &luma).unwrap().iter().any(|e| e.mid != 0.5));
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
