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
        brightness: 0.0,
        pick_log: -0.5,
        knee: 0.1,
        curves: &CURVES,
        curve_low: -2.0,
        curve_step: 0.01,
        print: &PRINT,
        view: &VIEW,
        density: 1.0,
        pick: [0.01, 0.02, 0.03],
        white: [0.8, 0.85, 0.9],
        anchor: [0.01, 0.02, 0.03],
        luma: false,
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
fn the_ends_lay_the_pick_on_its_anchor_and_hold_the_white() {
    let k = narrow();
    for anchor in [k.pick, [0.0; 3], [0.005, 0.03, 0.01]] {
        let p = Paper { anchor, ..k };
        let (black, white) = (p.ended(k.pick), p.ended(k.white));
        assert!((0..3).all(|c| (black[c] - anchor[c]).abs() < 1e-7 && (white[c] - k.white[c]).abs() < 1e-6), "{black:?} {white:?}");
    }
    // On pure black: a straight line in light from no light to the white, the middle of the range at half the white;
    // under the pick a gain onto no light, past the white a straight line on.
    let laid = Paper { anchor: [0.0; 3], ..k };
    let mid = map(k.pick, |c, b| b + 0.5 * (k.white[c] - b));
    assert!((0..3).all(|c| (laid.ended(mid)[c] - 0.5 * k.white[c]).abs() < 1e-6), "{:?}", laid.ended(mid));
    assert_eq!(laid.ended(map(k.pick, |_, v| v * 0.5)), [0.0; 3]);
    let past = map(k.white, |_, v| v + 0.1);
    assert!((0..3).all(|c| laid.ended(past)[c] > k.white[c]), "{:?}", laid.ended(past));
    // Twin: at rest the print's light passes as it is, the pick its own.
    assert_eq!(k.ended(k.pick), k.pick);
    assert_eq!(k.ended(mid), mid);
}

/// The slopes of `f` over `n` steps of x, from under the pick to past the end of the largest darkening's band.
fn slopes(f: &dyn Fn(f32) -> f32, knee: f32, n: usize) -> Vec<f32> {
    let h = 40.0 * knee / n as f32;
    (0..n).map(|i| -knee + h * i as f32).map(|x| (f(x + h) - f(x)) / h).collect()
}

/// How far the slope of `f` right of `at` departs from its slope left of it, each read over `h`.
fn kink(f: &dyn Fn(f32) -> f32, at: f32, h: f32) -> f32 {
    ((f(at + h) - f(at)) / h - (f(at) - f(at - h)) / h).abs()
}

const LIFTS: [f32; 9] = [-1.2, -0.3, -0.15, -0.01, 0.0, 0.01, 0.15, 0.3, 1.2];

#[test]
fn brightness_holds_the_knee_moves_past_it_by_its_lift_and_never_turns_a_tone_back() {
    let knee = 0.12;
    for lift in LIFTS {
        // The pick, the tones beyond it and every tone up to the knee keep their exposure, to the bit.
        for x in [-0.3f32, -1e-7, 0.0, 1e-7, 0.05, knee] {
            assert_eq!(kneed(x, lift, knee), x, "{lift} {x}");
        }
        // Past the knee's fade and the excess that lands the lift, the whole lift: Exposure's own shift.
        let gain = PAPER_KNEE_SLOPE - 1.0;
        let whole = knee * 1.5 + if lift >= 0.0 { 2.0 * lift / gain } else { -(gain + 2.0) * lift / gain };
        for x in [whole + 1e-4, whole + 0.5, 4.0] {
            assert!((kneed(x, lift, knee) - (x + lift)).abs() < 3e-7, "{lift} {x}: {}", kneed(x, lift, knee));
        }
        // Every slope within a quarter and 4, so no tone passes another and no layer turns back; C1 at each joint, the
        // slopes either side meeting as h shrinks.
        let f = |x: f32| kneed(x, lift, knee);
        let (low, high) = slopes(&f, knee, 20000).into_iter().fold((f32::MAX, 0.0f32), |(l, h), v| (l.min(v), h.max(v)));
        let (least, most) = (1.0 / PAPER_KNEE_SLOPE, PAPER_KNEE_SLOPE);
        assert!(low > least - 1e-3 && high < most + 1e-3, "{lift}: slopes {low} to {high}");
        for at in [knee, 2.0 * knee, whole] {
            let (coarse, fine) = (kink(&f, at, 2e-3), kink(&f, at, 1e-3));
            assert!(fine < 0.6 * coarse + 1.5e-3, "{lift} at {at}: {coarse} then {fine}");
        }
    }
    // More lift never moves a tone back: at each x the map rises with the lift.
    for i in 0..=300 {
        let x = -0.1 + 4.0 * i as f32 / 300.0;
        let moved: Vec<f32> = (0..=240).map(|j| kneed(x, -1.2 + 2.4 * j as f32 / 240.0, knee)).collect();
        assert!(moved.windows(2).all(|w| w[1] >= w[0] - 1e-6), "{x}");
    }
    // Twin: a corner, the whole lift laid straight on from the knee, keeps its kink whatever h.
    let corner = |x: f32| x + 0.3 * clamp(x / knee - 1.0, 0.0, 1.0);
    assert!(kink(&corner, knee, 1e-3) > 0.9 * kink(&corner, knee, 2e-3));
    // Twin: a fade of fixed width, a knee wide, darkening by 1.2 turns tones back.
    let fixed = |x: f32| x - 1.2 * { let t = clamp(x / knee - 1.0, 0.0, 1.0); t * t * (3.0 - 2.0 * t) };
    assert!(slopes(&fixed, knee, 20000).iter().any(|s| *s < 0.0));
    // Twin: a fade as wide as the knee and the lift, eased by a smoothstep, gives a grey 0.8 under a knee of 0.36 less
    // at +1.2 than at +0.6.
    let wide = |x: f32, lift: f32| x + lift * { let t = clamp((x - 0.36) / (0.36 + lift), 0.0, 1.0); t * t * (3.0 - 2.0 * t) };
    assert!(wide(0.8, 1.2) < wide(0.8, 0.6), "{} {}", wide(0.8, 1.2), wide(0.8, 0.6));
}

#[test]
fn brightness_moves_the_three_layers_by_one_shift_as_far_as_their_mean_lies_under_the_pick() {
    let k = Paper { brightness: 1.0, ..narrow() };
    // Past twice the knee, the three layers a stop less exposed: log10 2 each.
    let log = [-1.4, -1.2, -1.3];
    let moved = k.brightened(log);
    assert!((0..3).all(|c| (log[c] - moved[c] - std::f32::consts::LOG10_2).abs() < 1e-6), "{moved:?}");
    // Under the knee, nothing moves; between, one shift on the three, their differences kept.
    let near = [-0.52, -0.58, -0.6];
    assert_eq!(k.brightened(near), near);
    let between = [-0.62, -0.68, -0.7];
    let shifted = k.brightened(between);
    let by: V3 = map(between, |c, v| v - shifted[c]);
    assert!(by[0] > 0.01 && (by[0] - by[1]).abs() < 1e-7 && (by[0] - by[2]).abs() < 1e-7, "{by:?}");
    // The pick and the tones beyond it print where they did.
    let rest = narrow();
    for amounts in [[0.0; 3], [-0.2, -0.1, -0.3]] {
        assert_eq!(k.shown(amounts), rest.shown(amounts), "{amounts:?}");
    }
    // Twin: each layer moved by its own distance under the pick lays different shifts on them.
    let own: V3 = map(between, |_, v| v - (kneed(k.pick_log - v, std::f32::consts::LOG10_2, k.knee) - (k.pick_log - v)));
    assert!(by[0] < std::f32::consts::LOG10_2 - 0.01, "between, a part of the stop: {by:?}");
    assert!((between[0] - own[0] - (between[2] - own[2])).abs() > 1e-3, "{own:?}");
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

