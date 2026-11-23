use super::*;

static CURVES: std::sync::LazyLock<[V3; PAPER_CURVE_NODES]> =
    std::sync::LazyLock::new(|| std::array::from_fn(|i| { let d = 0.01 * i as f32; [d, 2.0 * d, 3.0 * d] }));

const IDENTITY: [V3; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// A unit window on each layer, a linear curve on each layer.
fn narrow() -> Paper {
    Paper {
        span: 1.0,
        bend: 1.0,
        mix: IDENTITY,
        filters: [1.0; 3],
        scale: 1.0,
        brightness: 0.0,
        pick_log: -0.5,
        reach: 0.1,
        curves: &CURVES,
        curve_low: -2.0,
        curve_step: 0.01,
        pick: [0.01, 0.02, 0.03],
        white: [0.8, 0.85, 0.9],
        anchor: [0.01, 0.02, 0.03],
        white_anchor: [0.8, 0.85, 0.9],
        luma: false,
    }
}

#[test]
fn each_layer_is_exposed_by_its_own_channels_density_and_a_filter_dims_its_own() {
    let k = narrow();
    let log = k.exposed([0.3, 0.6, 0.9]);
    assert!((0..3).all(|c| (log[c] + [0.3, 0.6, 0.9][c]).abs() < 1e-6), "{log:?}");
    // A yellow filter at 30 CC lowers the blue-sensitive layer's exposure by 0.3, the others not at all.
    let yellow = Paper { filters: [1.0, 1.0, 10f32.powf(-0.3)], ..k }.exposed([0.3, 0.6, 0.9]);
    assert!((yellow[2] - (log[2] - 0.3)).abs() < 1e-5 && (yellow[0] - log[0]).abs() < 1e-6, "{yellow:?}");
    // Twin: the same filter read as a magenta one dims green instead.
    let magenta = Paper { filters: [1.0, 10f32.powf(-0.3), 1.0], ..k }.exposed([0.3, 0.6, 0.9]);
    assert!((magenta[2] - log[2]).abs() < 1e-6 && (magenta[1] - log[1]).abs() > 0.29);
    // Twice the time lifts every layer's exposure by log10 2.
    let longer = Paper { scale: 2.0, ..k }.exposed([0.3, 0.6, 0.9]);
    assert!((0..3).all(|c| (longer[c] - log[c] - std::f32::consts::LOG10_2).abs() < 1e-6), "{longer:?}");
}

#[test]
fn a_share_lies_back_on_the_mean_span_through_its_bend_and_the_mix_reads_the_three() {
    let k = Paper { span: 1.4, bend: 0.8, ..narrow() };
    let d = k.density([0.0, 0.5, 1.0]);
    assert!(d[0] == 0.0 && (d[1] - 1.4 * 0.5f32.powf(0.8)).abs() < 1e-6 && (d[2] - 1.4).abs() < 1e-6, "{d:?}");
    // Under none, the power turned over: odd about the bare film, so a share just under it reads just under none.
    let under = k.density([-0.5, -1e-3, 1e-3]);
    assert!((under[0] + 1.4 * 0.5f32.powf(0.8)).abs() < 1e-6 && under[1] < 0.0 && (under[1] + under[2]).abs() < 1e-7, "{under:?}");
    // A mix whose rows sum to one keeps a neutral neutral and moves a colour.
    let mixed = Paper { mix: [[0.8, 0.2, 0.0], [0.1, 0.8, 0.1], [0.0, 0.3, 0.7]], ..k };
    let grey = mixed.density([0.4; 3]);
    assert!((grey[0] - grey[1]).abs() < 1e-6 && (grey[1] - grey[2]).abs() < 1e-6, "{grey:?}");
    let colour = mixed.density([0.9, 0.2, 0.2]);
    assert!((colour[0] - k.density([0.9, 0.2, 0.2])[0]).abs() > 0.05, "{colour:?}");
    // Twin: the bend at one lays each share straight on the span.
    let straight = Paper { bend: 1.0, ..k }.density([0.5; 3]);
    assert!((straight[0] - 0.7).abs() < 1e-6 && (straight[0] - d[1]).abs() > 0.1);
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

#[test]
fn pure_ends_lay_the_pick_on_black_and_the_papers_white_on_white_exactly() {
    let k = narrow();
    let pure = Paper { anchor: [0.0; 3], white_anchor: [1.0; 3], ..k };
    assert_eq!((pure.ended(k.pick), pure.ended(k.white)), ([0.0; 3], [1.0; 3]), "to the bit");
    // Between them a straight line in light, the middle of the range at one half, rising throughout.
    let mid = map(k.pick, |c, b| b + 0.5 * (k.white[c] - b));
    assert!(pure.ended(mid).iter().all(|v| (v - 0.5).abs() < 1e-6), "{:?}", pure.ended(mid));
    let tones: Vec<V3> = (0..=200).map(|i| pure.ended(map(k.white, |_, w| w * 1.1 * i as f32 / 200.0))).collect();
    assert!(tones.windows(2).all(|w| (0..3).all(|c| w[1][c] >= w[0][c])), "monotone");
    // Twin: the white held, as on the pick's anchor alone, misses pure white by the paper's own base.
    let held = Paper { anchor: [0.0; 3], ..k };
    assert!(held.ended(k.white).iter().all(|v| *v < 0.95), "{:?}", held.ended(k.white));
    // Twin: the white laid alone, the pick on its own anchor, leaves the black where it was.
    let white_only = Paper { white_anchor: [1.0; 3], ..k };
    let (black, white) = (white_only.ended(k.pick), white_only.ended(k.white));
    assert!((0..3).all(|c| (black[c] - k.pick[c]).abs() < 1e-7 && (white[c] - 1.0).abs() < 1e-6), "{black:?} {white:?}");
}

/// The slopes of `f` over `n` steps of x, from under the pick to past the end of the largest darkening's fade.
fn slopes(f: &dyn Fn(f32) -> f32, reach: f32, n: usize) -> Vec<f32> {
    let h = 8.0 * reach / n as f32;
    (0..n).map(|i| -reach + h * i as f32).map(|x| (f(x + h) - f(x)) / h).collect()
}

/// How far the slope of `f` right of `at` departs from its slope left of it, each read over `h`.
fn kink(f: &dyn Fn(f32) -> f32, at: f32, h: f32) -> f32 {
    ((f(at + h) - f(at)) / h - (f(at) - f(at - h)) / h).abs()
}

/// Brightness's lifts in log exposure, its track's ends (±4 stops) included.
const LIFTS: [f32; 9] = [-1.2042, -0.3, -0.15, -0.01, 0.0, 0.01, 0.15, 0.3, 1.2042];

#[test]
fn brightness_fades_in_from_the_pick_and_moves_past_its_reach_as_exposure_does() {
    // A frame's reach, and one so near the pick that the fade's floor sets its width.
    for reach in [0.3f32, 0.05] {
        for lift in LIFTS {
            let f = |x: f32| lifted(x, lift, reach);
            let width = fade(lift, reach);
            assert!(width >= reach && width >= (lift.abs() * std::f32::consts::LOG10_2).sqrt() - 1e-7, "{reach} {lift}: {width}");
            // The pick and the tones beyond it keep their exposure, to the bit.
            for x in [-0.3f32, -1e-7, 0.0] {
                assert_eq!(f(x), x, "{reach} {lift} {x}");
            }
            // Past the fade, and past what a darkening folds under it, the whole lift: Exposure's own shift.
            for x in [width + lift.abs() + 1e-4, width + lift.abs() + 0.5, 4.0] {
                assert!((f(x) - (x + lift)).abs() < 3e-7, "{reach} {lift} {x}: {}", f(x));
            }
            // The deep shadows, a tenth of the reach from the pick, take under 3 % of the lift.
            let deep = 0.1 * reach;
            assert!((f(deep) - deep).abs() <= 0.03 * lift.abs() + 1e-7, "{reach} {lift}: {}", f(deep) - deep);
            // Every slope within a smoothstep's steepest and its inverse, inside 4 and ¼ over the whole track, so no tone
            // passes another; C1 at the pick, the fade's end and where a darkening's fold ends.
            let (low, high) = slopes(&f, width, 4000).into_iter().fold((f32::MAX, 0.0f32), |(l, h), v| (l.min(v), h.max(v)));
            let steepest = 1.0 + 1.5 * lift.abs() / width;
            let (least, most) = if lift >= 0.0 { (1.0, steepest) } else { (1.0 / steepest, 1.0) };
            assert!(low > least - 2e-3 && high < most + 2e-3 && low > 0.25 - 2e-3 && high < 4.0 + 2e-3, "{reach} {lift}: slopes {low} to {high}");
            for at in [0.0, width, width + lift.abs()] {
                let (coarse, fine) = (kink(&f, at, 2e-3), kink(&f, at, 1e-3));
                assert!(fine < 0.6 * coarse + 2e-3, "{reach} {lift} at {at}: {coarse} then {fine}");
            }
            // Darkening undoes brightening: −b brings back what +b moved.
            for i in 0..=200 {
                let x = -0.1 + 1.5 * i as f32 / 200.0;
                assert!((lifted(lifted(x, lift, reach), -lift, reach) - x).abs() < 2e-6, "{reach} {lift} {x}");
            }
        }
        // More lift never moves a tone back: at each x the map rises with the lift.
        for i in 0..=300 {
            let x = -0.1 + 2.0 * i as f32 / 300.0;
            let moved: Vec<f32> = (0..=240).map(|j| lifted(x, -1.2042 + 2.4084 * j as f32 / 240.0, reach)).collect();
            assert!(moved.windows(2).all(|w| w[1] >= w[0] - 1e-6), "{reach} {x}");
        }
    }
    let reach = 0.3;
    // Twin: a corner, the lift laid on straight from the pick, keeps its kink whatever h.
    let corner = |x: f32| x + 0.3 * clamp(x / reach, 0.0, 1.0);
    assert!(kink(&corner, 0.0, 1e-3) > 0.9 * kink(&corner, 0.0, 2e-3));
    // Twin: that straight fade moves the deep shadows by a tenth of the lift.
    assert!(corner(0.1 * reach) - 0.1 * reach > 0.09 * 0.3);
    // Twin: darkening by the same smoothstep, not its inverse, turns tones back at the track's end.
    let folded = |x: f32| x - 1.2042 * share(x, fade(1.2042, reach));
    assert!(slopes(&folded, reach, 4000).iter().any(|s| *s < 0.0));
    // Twin: that fold leaves −b short of undoing +b.
    assert!((folded(lifted(0.2, 1.2042, reach)) - 0.2).abs() > 0.05);
    // Twin: a fade held at a reach near the pick steepens the track's end far past 4.
    let held = |x: f32| x + 1.2042 * share(x, 0.05);
    assert!(slopes(&held, 0.05, 4000).iter().any(|s| *s > 8.0));
    // Twin: a fade widening as the lift itself, not its root, moves the deepest tones back as the lift grows.
    let proportional = |lift: f32| 0.01 + lift * share(0.01, lift);
    assert!(proportional(1.2) < proportional(0.6), "{} {}", proportional(1.2), proportional(0.6));
}

#[test]
fn brightness_moves_the_three_layers_by_one_shift_as_far_as_their_mean_lies_under_the_pick() {
    let k = Paper { brightness: 1.0, ..narrow() };
    // Past the reach, the three layers a stop less exposed: log10 2 each.
    let log = [-1.4, -1.2, -1.3];
    let moved = k.brightened(log);
    assert!((0..3).all(|c| (log[c] - moved[c] - std::f32::consts::LOG10_2).abs() < 1e-6), "{moved:?}");
    // Within the reach, one shift on the three, their differences kept, a part of the stop.
    let between = [-0.52, -0.58, -0.6];
    let shifted = k.brightened(between);
    let by: V3 = map(between, |c, v| v - shifted[c]);
    assert!(by[0] > 0.01 && (by[0] - by[1]).abs() < 1e-7 && (by[0] - by[2]).abs() < 1e-7, "{by:?}");
    assert!(by[0] < std::f32::consts::LOG10_2 - 0.01, "{by:?}");
    // The pick and the tones beyond it print where they did.
    let rest = narrow();
    for shares in [[0.0; 3], [-0.2, -0.1, -0.3]] {
        assert_eq!(k.shown(shares), rest.shown(shares), "{shares:?}");
    }
    // Twin: each layer moved by its own distance under the pick lays different shifts on them.
    let own: V3 = map(between, |_, v| v - (lifted(k.pick_log - v, std::f32::consts::LOG10_2, k.reach) - (k.pick_log - v)));
    assert!((between[0] - own[0] - (between[2] - own[2])).abs() > 1e-3, "{own:?}");
}

#[test]
fn the_print_reads_each_dye_on_its_own_channel_and_a_black_and_white_print_its_luma() {
    let k = narrow();
    let out = k.viewed([0.5, 1.0, 2.0]);
    let expected = [0.5f32, 1.0, 2.0].map(|d| 10f32.powf(-d));
    assert!((0..3).all(|c| (out[c] - expected[c]).abs() < 1e-6), "{out:?}");
    // Each dye moves its own channel alone.
    for c in 0..3 {
        let mut denser = [0.5, 1.0, 2.0];
        denser[c] += 0.3;
        let moved = k.viewed(denser);
        assert!((0..3).all(|i| (moved[i] == out[i]) == (i != c)), "dye {c}: {moved:?}");
    }
    // Twin: a dye absorbing a tenth in its neighbour's channel darkens that channel too.
    let crossed = |d: V3| map(d, |c, v| 10f32.powf(-(v + 0.1 * d[(c + 1) % 3])));
    assert!((crossed([0.5, 1.3, 2.0])[0] - out[0]).abs() > 1e-3);
    // A black and white print reads its luma on every channel.
    let mono = Paper { luma: true, ..k }.printed([0.3, 0.6, 0.9]);
    assert!(mono[0] == mono[1] && mono[1] == mono[2]);
    assert_ne!(k.printed([0.3, 0.6, 0.9])[0], k.printed([0.3, 0.6, 0.9])[1]);
}
