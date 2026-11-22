use super::*;

static NEGATIVE: [V4; 3] = [[1.0, 0.2, 0.05, 0.1], [0.1, 1.0, 0.2, 0.2], [0.02, 0.1, 1.0, 0.4]];
static EXPOSURE: [V3; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
static DICHROIC: [V3; 3] = [[0.0, 1.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 0.0]];
static PRINT: [V4; 3] = [[1.0, 0.0, 0.0, 0.05], [0.0, 1.0, 0.0, 0.05], [0.0, 0.0, 1.0, 0.05]];
static VIEW: [V3; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
static CURVES: std::sync::LazyLock<[V3; PAPER_CURVE_NODES]> =
    std::sync::LazyLock::new(|| std::array::from_fn(|i| { let d = 0.01 * i as f32; [d, 2.0 * d, 3.0 * d] }));

/// Three narrow bands, each alone in a layer and in a viewing channel, a linear curve on each layer.
fn narrow() -> Paper {
    Paper {
        gain: [1.0; 3],
        pedestal: 0.0,
        log_guard: 1e-12,
        black: [0.0; 3],
        span: [1.0, 1.2, 1.4],
        points_black: [0.0; 3],
        points_white: [0.0; 3],
        zones: [0.0; 2],
        mids: [0.0; 3],
        negative: &NEGATIVE,
        exposure: &EXPOSURE,
        dichroic: &DICHROIC,
        filters: [1.0; 3],
        scale: 1.0,
        pivot: [0.0; 3],
        slope: [1.0; 3],
        curves: &CURVES,
        curve_low: -2.0,
        curve_step: 0.01,
        print: &PRINT,
        view: &VIEW,
        density: 1.0,
        pick: [0.01, 0.02, 0.03],
        white: [0.8, 0.85, 0.9],
        anchor: [0.01, 0.02, 0.03],
        brightness: 0.0,
        luma: false,
        reversal: false,
    }
}

#[test]
fn a_narrow_paper_exposes_each_layer_through_its_band_and_a_filter_dims_its_own() {
    let k = narrow();
    let log = k.exposed([0.3, 0.6, 0.9]);
    let expected = [-(0.3 + 0.12 + 0.045 + 0.1), -(0.03 + 0.6 + 0.18 + 0.2), -(0.006 + 0.06 + 0.9 + 0.4)];
    assert!((0..3).all(|c| (log[c] - expected[c]).abs() < 1e-5), "{log:?}");
    // A yellow filter at 30 CC dims the blue band by 0.3 in density, the others not at all.
    let yellow = Paper { filters: [1.0, 1.0, 10f32.powf(-0.3)], ..k }.exposed([0.3, 0.6, 0.9]);
    assert!((yellow[2] - (expected[2] - 0.3)).abs() < 1e-5 && (yellow[0] - expected[0]).abs() < 1e-6, "{yellow:?}");
    // Twin: the same filter read as a magenta one dims green instead.
    let magenta = Paper { filters: [1.0, 10f32.powf(-0.3), 1.0], ..k }.exposed([0.3, 0.6, 0.9]);
    assert!((magenta[2] - expected[2]).abs() < 1e-6 && (magenta[1] - expected[1]).abs() > 0.29);
}

#[test]
fn the_curves_hold_their_ends_and_run_between_their_nodes() {
    let k = narrow();
    assert_eq!(k.developed([-5.0, -5.0, -5.0]), [0.0; 3], "held under the first node");
    let top = k.developed([5.0; 3]);
    assert!((top[2] - 3.0 * 0.01 * (PAPER_CURVE_NODES - 1) as f32).abs() < 1e-4, "held past the last: {top:?}");
    let mid = k.developed([-1.495; 3]);
    assert!((mid[0] - 0.505).abs() < 1e-5 && (mid[2] - 1.515).abs() < 1e-4, "between two nodes: {mid:?}");
    // Twin: the node below read alone misses the half step.
    assert!((k.developed([-1.5; 3])[0] - 0.505).abs() > 4e-3);
}

#[test]
fn the_grade_turns_each_layer_about_its_pivot() {
    let k = Paper { pivot: [-1.0, -0.5, 0.0], slope: [2.0, 0.5, 1.5], ..narrow() };
    assert_eq!(k.graded([-1.0, -0.5, 0.0]), [-1.0, -0.5, 0.0], "the pivot stays");
    let turned = k.graded([-0.8, -0.3, 0.2]);
    let expected = [-0.6, -0.4, 0.3];
    assert!((0..3).all(|c| (turned[c] - expected[c]).abs() < 1e-6), "{turned:?}");
    // Twin: the plain slope leaves every exposure where it is.
    assert_eq!(narrow().graded([-0.8, -0.3, 0.2]), [-0.8, -0.3, 0.2]);
}

#[test]
fn the_tones_stretch_the_range_lay_the_zones_by_tone_and_hold_both_ends() {
    let k = narrow();
    let ends = |p: &Paper| (p.toned([0.0; 3]), p.toned(p.span));
    // The points: a black point of 0.1 reads a tenth of the range as bare film, a white point the top tenth as the densest.
    let pointed = Paper { points_black: [0.1; 3], points_white: [0.1; 3], ..k };
    let at = pointed.toned([0.1, 0.12, 0.14]);
    assert!(at.iter().all(|v| v.abs() < 1e-6), "the black point's density reads as bare film: {at:?}");
    let top = pointed.toned([0.9, 1.08, 1.26]);
    assert!((0..3).all(|c| (top[c] - k.span[c]).abs() < 1e-5), "the white point's as the densest: {top:?}");
    // The zones: no density at the bare film or at the densest, Shadows lifting the lower range most, Highlights the upper.
    for zones in [[0.25, 0.0], [-0.25, 0.0], [0.0, 0.25], [0.0, -0.4]] {
        let z = Paper { zones, ..k };
        let (low, high) = ends(&z);
        assert!(low.iter().all(|v| v.abs() < 1e-7) && (0..3).all(|c| (high[c] - k.span[c]).abs() < 1e-6), "{zones:?}");
    }
    let shadows = Paper { zones: [0.25, 0.0], ..k };
    let lifted = |p: &Paper, u: f32| p.toned(k.span.map(|s| s * u))[0] / k.span[0] - u;
    assert!(lifted(&shadows, 0.25) > lifted(&shadows, 0.75) && lifted(&shadows, 0.25) > 0.1, "{}", lifted(&shadows, 0.25));
    let highlights = Paper { zones: [0.0, 0.25], ..k };
    assert!(lifted(&highlights, 0.75) > lifted(&highlights, 0.25) && lifted(&highlights, 0.75) > 0.1);
    // One density by the mean tone: a coloured amount moves on every channel by the same share of its range.
    let colour = shadows.toned([0.2, 0.36, 0.7]);
    let moved: V3 = map(colour, |c, v| v / k.span[c] - [0.2, 0.3, 0.5][c]);
    assert!((moved[0] - moved[2]).abs() < 1e-6 && moved[0] > 0.0, "{moved:?}");
    // The mids: a channel's own, ends held.
    let mids = Paper { mids: [0.2, 0.0, 0.0], ..k };
    let (low, high) = ends(&mids);
    assert!(low == [0.0; 3] && (high[0] - k.span[0]).abs() < 1e-6);
    let half = mids.toned(k.span.map(|s| s * 0.5));
    assert!((half[0] / k.span[0] - 0.7).abs() < 1e-6 && (half[1] / k.span[1] - 0.5).abs() < 1e-6, "{half:?}");
    // Twin: at rest nothing moves, to the bit.
    assert_eq!(k.toned([0.2, 0.36, 0.7]), [0.2, 0.36, 0.7]);
}

#[test]
fn a_reversal_paper_reads_the_tones_from_the_densest_film_its_print_s_black() {
    let k = Paper { reversal: true, ..narrow() };
    assert_eq!(k.toned([0.2, 0.36, 0.7]), [0.2, 0.36, 0.7], "at rest nothing moves");
    // A black point of 0.1 reads the densest tenth of the range as the densest film, the bare film held.
    let pointed = Paper { points_black: [0.1; 3], ..k };
    let top = pointed.toned([0.9, 1.08, 1.26]);
    assert!((0..3).all(|c| (top[c] - k.span[c]).abs() < 1e-5), "{top:?}");
    assert!(pointed.toned([0.0; 3]).iter().all(|v| v.abs() < 1e-6));
    // Shadows lift the dense end, a slide's shadows: less density there, the bare end barely moved.
    let shadows = Paper { zones: [0.25, 0.0], ..k };
    let at = |u: f32| shadows.toned(k.span.map(|s| s * u))[0] / k.span[0];
    assert!(at(0.75) < 0.65 && 0.75 - at(0.75) > (at(0.25) - 0.25).abs(), "{} {}", at(0.75), at(0.25));
    // A channel's mids take density away from its middle, as they brighten it.
    let mids = Paper { mids: [0.2, 0.0, 0.0], ..k };
    assert!((mids.toned(k.span.map(|s| s * 0.5))[0] / k.span[0] - 0.3).abs() < 1e-6);
    // Twin: on a negative paper the same black point reads the bare end as black.
    let negative = Paper { points_black: [0.1; 3], ..narrow() };
    assert!(negative.toned([0.1, 0.12, 0.14]).iter().all(|v| v.abs() < 1e-6));
    assert!(pointed.toned([0.1, 0.12, 0.14]).iter().all(|v| v.abs() > 1e-2));
}

#[test]
fn the_ends_lay_the_pick_on_its_anchor_hold_the_white_and_brighten_between() {
    let k = narrow();
    let anchors = [k.pick, [0.0; 3], [0.005, 0.03, 0.01]];
    for (brightness, anchor) in [(1.0, anchors[0]), (-1.0, anchors[0]), (0.0, anchors[1]), (2.0, anchors[1]), (0.5, anchors[2])] {
        let p = Paper { brightness, anchor, ..k };
        let (black, white) = (p.ended(k.pick), p.ended(k.white));
        assert!((0..3).all(|c| (black[c] - anchor[c]).abs() < 1e-7 && (white[c] - k.white[c]).abs() < 1e-6), "{black:?} {white:?}");
    }
    // Brightness: the odds gain between the ends, a stop lifting the middle of the range to two thirds.
    let bright = Paper { brightness: 1.0, ..k };
    let mid = map(k.pick, |c, b| b + 0.5 * (k.white[c] - b));
    let lifted = bright.ended(mid);
    assert!((0..3).all(|c| ((lifted[c] - k.pick[c]) / (k.white[c] - k.pick[c]) - 2.0 / 3.0).abs() < 1e-6), "{lifted:?}");
    // On pure black: a straight line in light from no light to the white, so the middle of the range lands at half the
    // white; under the pick a gain onto no light, past the white a straight line on.
    let laid = Paper { anchor: [0.0; 3], ..k };
    assert!((0..3).all(|c| (laid.ended(mid)[c] - 0.5 * k.white[c]).abs() < 1e-6), "{:?}", laid.ended(mid));
    assert_eq!(laid.ended(map(k.pick, |_, v| v * 0.5)), [0.0; 3]);
    let past = map(k.white, |_, v| v + 0.1);
    assert!((0..3).all(|c| laid.ended(past)[c] > k.white[c]), "{:?}", laid.ended(past));
    // Twin: at rest the print's light passes as it is, the pick its own.
    assert_eq!(k.ended(k.pick), k.pick);
    assert_eq!(k.ended(mid), mid);
}

#[test]
fn the_colour_density_raises_each_channel_and_undoes_the_lumas_power() {
    let k = Paper { density: 2.0, ..narrow() };
    let light = [0.2, 0.3, 0.5];
    let out = k.coloured(light);
    let p: V3 = light.map(|v| v * v);
    let f = (LUMA[0] * p[0] + LUMA[1] * p[1] + LUMA[2] * p[2]).powf(-0.5);
    assert!((0..3).all(|c| (out[c] - p[c] * f).abs() < 1e-7), "{out:?}");
    // Twin: at unit density the light passes to the bit.
    assert_eq!(narrow().coloured(light), light);
}

#[test]
fn the_print_reads_its_dyes_and_base_and_the_reading_lies_above_the_black() {
    let k = narrow();
    let out = k.viewed([0.5, 1.0, 2.0]);
    let expected = [0.55, 1.05, 2.05].map(|d: f32| 10f32.powf(-d));
    assert!((0..3).all(|c| (out[c] - expected[c]).abs() < 1e-6), "{out:?}");
    let read = Paper { black: [0.2; 3], gain: [2.0, 1.0, 1.0], ..k }.amounts([0.01, 0.01, 1e-20, 1.0]);
    assert!((read[0] - (-(0.02f32).log10() - 0.2)).abs() < 1e-5 && (read[2] - 11.8).abs() < 1e-4, "held at the guard: {read:?}");
    // Twin: no black reads the full density.
    assert!((k.amounts([0.01, 0.01, 0.01, 1.0])[1] - 2.0).abs() < 1e-5);
    // A black and white print reads its luma on every channel.
    let mono = Paper { luma: true, ..k }.printed([0.3, 0.6, 0.9]);
    assert!(mono[0] == mono[1] && mono[1] == mono[2]);
    assert_ne!(k.printed([0.3, 0.6, 0.9])[0], k.printed([0.3, 0.6, 0.9])[1]);
}
