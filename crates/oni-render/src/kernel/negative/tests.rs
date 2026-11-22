use super::*;

const COUPLERS: [V3; 3] = [[0.336, 0.353, 0.302], [0.154, 0.319, 0.353], [0.168, 0.226, 0.273]];

/// A smooth negative's three layers, each a toe, a straight line of its own slope and a shoulder, from −3 to 4 log.
fn sigmoid(e: f32, c: usize) -> f32 {
    let gamma = [0.55, 0.6, 0.7][c];
    let range = 2.6;
    range * 0.5 * (1.0 + (2.0 * gamma * (e - 0.6) / range).tanh()) + 0.005 * (e + 3.0)
}

static SMOOTH: std::sync::LazyLock<Development> = std::sync::LazyLock::new(|| {
    let (low, step) = (-3.0f32, 7.0 / (NEGATIVE_CURVE_NODES - 1) as f32);
    let normal: [V3; NEGATIVE_CURVE_NODES] = std::array::from_fn(|i| std::array::from_fn(|c| sigmoid(low + step * i as f32, c)));
    // The curve the couplers' pulled exposure develops on, so a grey develops on `normal`.
    let after = std::array::from_fn(|i| {
        std::array::from_fn(|c| {
            let x = low + step * i as f32;
            let (mut a, mut b) = (-12.0f32, 12.0f32);
            for _ in 0..60 {
                let e = 0.5 * (a + b);
                let pulled = e - (0..3).map(|d| sigmoid(e, d) * COUPLERS[d][c]).sum::<f32>();
                if pulled < x { a = e } else { b = e }
            }
            sigmoid(0.5 * (a + b), c)
        })
    });
    Development { normal, after, log_low: low, log_step: step, foot: [-10.0; 3], top: [10.0; 3], couplers: COUPLERS }
});

fn at_rest() -> Negative {
    let tone = std::array::from_fn(|i| [-1.0 + i as f32 * 6.0 / (DYE_TONE_NODES - 1) as f32; 3]);
    Negative {
        gain: [1.0; 3],
        pedestal: 0.0,
        log_guard: 1e-12,
        black: [0.0; 3],
        sense: 1.0,
        development: &SMOOTH,
        scale: [1.0; 3],
        pivot: [0.0; 3],
        slope: [1.0; 3],
        shift: [0.0; 3],
        own_pivot: [0.0; 3],
        own_slope: [1.0; 3],
        own_shift: [0.0; 3],
        span: [2.0, 2.0, 2.2],
        origin: 0.0,
        points_black: [0.0; 3],
        points_white: [0.0; 3],
        zones: [0.0; 2],
        mids: [0.0; 3],
        tone,
        tone_low: [-1.0; 3],
        tone_step: [6.0 / (DYE_TONE_NODES - 1) as f32; 3],
    }
}

#[test]
fn a_layers_exposure_reads_back_through_the_couplers() {
    let d = &*SMOOTH;
    for log in [[0.0, 0.0, 0.0], [-1.2, 0.4, 1.1], [2.0, -1.0, 0.3], [-1.5, 3.5, 0.0]] {
        let back = d.exposures(d.developed(log));
        assert!((0..3).all(|c| (back[c] - log[c]).abs() < 1e-3), "{log:?} {back:?}");
    }
    // Under the curve's first exposure no exposure develops: the fog.
    let under = d.developed([-5.0; 3]);
    assert!((0..3).all(|c| (under[c] - d.developed([-4.0; 3])[c]).abs() < 1e-6), "{under:?}");
    // A grey develops on the grey's own curve, the couplers' pull included.
    let grey = d.developed([0.4; 3]);
    assert!((0..3).all(|c| (grey[c] - sigmoid(0.4, c)).abs() < 2e-3), "{grey:?}");
    // Twin: read back without the couplers, a colour lands off its exposure.
    let plain = Development { couplers: [[0.0; 3]; 3], ..d.clone() };
    let off = plain.exposures(d.developed([-1.2, 0.4, 1.1]));
    assert!((0..3).any(|c| (off[c] - [-1.2, 0.4, 1.1][c]).abs() > 0.1), "{off:?}");
}

#[test]
fn at_rest_the_negative_reads_through_unmoved_and_a_stop_is_a_stop_of_exposure() {
    let k = at_rest();
    let density = [0.4, 1.1, 0.8];
    assert_eq!(k.redeveloped(density), density, "no move develops nothing again");
    let up = Negative { shift: [std::f32::consts::LOG10_2; 3], ..k };
    let log = SMOOTH.exposures(density);
    let expected = SMOOTH.developed(log.map(|e| e + std::f32::consts::LOG10_2));
    let got = up.redeveloped(density);
    assert!((0..3).all(|c| (got[c] - expected[c]).abs() < 1e-4 && got[c] > density[c]), "{got:?} {expected:?}");
    // Twin: a stop laid on the density, as a print's time would, is not the film's stop.
    assert!((0..3).any(|c| (density[c] + 0.3 * [0.55, 0.6, 0.7][c] - got[c]).abs() > 5e-3), "{got:?}");
}

#[test]
fn a_channels_own_move_stays_on_its_layer_and_the_neutral_moves_pull_on_all() {
    let k = at_rest();
    let density = [0.4, 1.1, 0.8];
    let red = Negative { own_shift: [0.2, 0.0, 0.0], own_slope: [1.0, 1.0, 1.0], ..k }.redeveloped(density);
    assert!(red[0] > density[0] + 0.05 && (red[1] - density[1]).abs() < 1e-4 && (red[2] - density[2]).abs() < 1e-4, "{red:?}");
    let steeper = Negative { own_slope: [1.0, 1.5, 1.0], own_pivot: [0.3; 3], ..k }.redeveloped(density);
    assert!((steeper[0] - density[0]).abs() < 1e-4 && (steeper[1] - density[1]).abs() > 0.02, "{steeper:?}");
    // Twin: the same move laid before the couplers pull moves the other layers too.
    let pulled = Negative { shift: [0.2, 0.0, 0.0], ..k }.redeveloped(density);
    assert!((pulled[1] - density[1]).abs() > 0.01 && (pulled[2] - density[2]).abs() > 0.01, "{pulled:?}");
}

#[test]
fn every_move_keeps_the_negative_rising_and_finite_past_its_curves() {
    let k = at_rest();
    let moves = [
        Negative { shift: [1.2; 3], ..k },
        Negative { shift: [-1.2, -1.0, -1.4], ..k },
        Negative { slope: [2.0; 3], pivot: [0.2; 3], ..k },
        Negative { slope: [0.5, 0.6, 0.5], pivot: [0.2; 3], shift: [0.3, 0.0, -0.3], ..k },
        Negative { own_slope: [2.0, 1.0, 0.5], own_pivot: [0.2; 3], own_shift: [0.3, 0.0, -0.3], shift: [0.5; 3], ..k },
        Negative { points_black: [0.1; 3], points_white: [0.2; 3], zones: [0.25, -0.25], mids: [0.2, 0.0, -0.2], ..k },
    ];
    for (i, m) in moves.iter().enumerate() {
        let mut last = [f32::NEG_INFINITY; 3];
        for step in 0..=400 {
            let d = -2.0 + step as f32 * 0.025;
            let out = m.toned(m.redeveloped([d; 3]));
            assert!(out.iter().all(|v| v.is_finite()), "{i}: {d} {out:?}");
            assert!((0..3).all(|c| out[c] >= last[c] - 1e-5), "{i}: {d} {out:?} {last:?}");
            last = out;
        }
    }
    // Twin: a slope under none turns the negative over.
    let turned = Negative { slope: [-1.0; 3], ..k };
    assert!(turned.redeveloped([1.5; 3])[1] < turned.redeveloped([0.5; 3])[1]);
}

#[test]
fn a_positive_reads_its_light_and_its_tone_share_runs_from_its_densest() {
    let line = Development {
        normal: std::array::from_fn(|i| [i as f32 - 128.0; 3]),
        after: std::array::from_fn(|i| [i as f32 - 128.0; 3]),
        log_low: -128.0,
        log_step: 1.0,
        foot: [-127.0; 3],
        top: [127.0; 3],
        couplers: [[0.0; 3]; 3],
    };
    let line: &'static Development = Box::leak(Box::new(line));
    let k = Negative { sense: -1.0, development: line, span: [-2.0; 3], origin: 1.0, shift: [0.3; 3], ..at_rest() };
    // A stop of light takes 0.3 off a slide's density.
    let lit = k.lit([10f32.powf(-1.0), 10f32.powf(-0.5), 1.0, 1.0]);
    assert!((0..3).all(|c| (lit[c] - ([1.0, 0.5, 0.0][c] - 0.3)).abs() < 1e-5), "{lit:?}");
    // Its black point darkens its densest film, where it prints black.
    let pointed = Negative { points_black: [0.1; 3], shift: [0.0; 3], ..k };
    let dense = pointed.toned([1.9; 3]);
    assert!(dense[0] > 1.9 + 0.1 && (pointed.toned([0.0; 3])[0] - 0.0).abs() < 0.25, "{dense:?}");
    // Twin: read as a negative, the same point lifts its clear film instead.
    let negative = Negative { sense: 1.0, span: [2.0; 3], origin: 0.0, ..pointed };
    assert!(negative.toned([0.0; 3])[0] < -0.1);
}

#[test]
fn the_tones_stretch_the_range_lay_the_zones_by_tone_and_hold_both_ends() {
    let k = Negative { span: [1.0, 1.2, 1.4], ..at_rest() };
    let ends = |p: &Negative| (p.toned([0.0; 3]), p.toned(p.span));
    // The points: a black point of 0.1 reads a tenth of the range as bare film, a white point the top tenth as the densest.
    let pointed = Negative { points_black: [0.1; 3], points_white: [0.1; 3], ..k };
    let at = pointed.toned([0.1, 0.12, 0.14]);
    assert!(at.iter().all(|v| v.abs() < 1e-6), "the black point's density reads as bare film: {at:?}");
    let top = pointed.toned([0.9, 1.08, 1.26]);
    assert!((0..3).all(|c| (top[c] - k.span[c]).abs() < 1e-5), "the white point's as the densest: {top:?}");
    // The zones: no density at the bare film or at the densest, Shadows lifting the lower range most, Highlights the upper.
    for zones in [[0.25, 0.0], [-0.25, 0.0], [0.0, 0.25], [0.0, -0.4]] {
        let z = Negative { zones, ..k };
        let (low, high) = ends(&z);
        assert!(low.iter().all(|v| v.abs() < 1e-7) && (0..3).all(|c| (high[c] - k.span[c]).abs() < 1e-6), "{zones:?}");
    }
    let shadows = Negative { zones: [0.25, 0.0], ..k };
    let lifted = |p: &Negative, u: f32| p.toned(k.span.map(|s| s * u))[0] / k.span[0] - u;
    assert!(lifted(&shadows, 0.25) > lifted(&shadows, 0.75) && lifted(&shadows, 0.25) > 0.1, "{}", lifted(&shadows, 0.25));
    let highlights = Negative { zones: [0.0, 0.25], ..k };
    assert!(lifted(&highlights, 0.75) > lifted(&highlights, 0.25) && lifted(&highlights, 0.75) > 0.1);
    // One density by the mean tone: a coloured amount moves on every channel by the same share of its range.
    let colour = shadows.toned([0.2, 0.36, 0.7]);
    let moved: V3 = map(colour, |c, v| v / k.span[c] - [0.2, 0.3, 0.5][c]);
    assert!((moved[0] - moved[2]).abs() < 1e-6 && moved[0] > 0.0, "{moved:?}");
    // The mids: a channel's own, ends held.
    let mids = Negative { mids: [0.2, 0.0, 0.0], ..k };
    let (low, high) = ends(&mids);
    assert!(low == [0.0; 3] && (high[0] - k.span[0]).abs() < 1e-6);
    let half = mids.toned(k.span.map(|s| s * 0.5));
    assert!((half[0] / k.span[0] - 0.7).abs() < 1e-6 && (half[1] / k.span[1] - 0.5).abs() < 1e-6, "{half:?}");
    // Twin: at rest nothing moves, to the bit.
    assert_eq!(k.toned([0.2, 0.36, 0.7]), [0.2, 0.36, 0.7]);
}
