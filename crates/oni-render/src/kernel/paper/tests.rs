use super::*;

static CURVES: std::sync::LazyLock<[V3; PAPER_CURVE_NODES]> =
    std::sync::LazyLock::new(|| std::array::from_fn(|i| { let d = 0.01 * i as f32; [d, 2.0 * d, 3.0 * d] }));

const IDENTITY: [V3; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// A linear curve on each layer over the window's share, its pick and white apart, pure dyes on no D-min.
fn narrow() -> Paper {
    Paper {
        curves: *CURVES,
        curve_low: -0.5,
        curve_step: 0.01,
        floor: [0.0; 3],
        crosstalk: IDENTITY,
        primaries: IDENTITY,
        latitude: 1.0,
        brightness: 0.0,
        pick_share: 0.5,
        reach: 0.1,
        pick: [0.01, 0.02, 0.03],
        white: [0.8, 0.85, 0.9],
        anchor: [0.01, 0.02, 0.03],
        white_anchor: [0.8, 0.85, 0.9],
        luma: false,
    }
}

#[test]
fn the_curves_hold_their_ends_and_run_between_their_nodes_on_the_windows_share() {
    let k = narrow();
    assert_eq!(k.amounts([-5.0, -5.0, -5.0]), [0.0; 3], "held under the first node");
    let top = k.amounts([5.0; 3]);
    assert!((top[2] - 3.0 * 0.01 * (PAPER_CURVE_NODES - 1) as f32).abs() < 1e-4, "held past the last: {top:?}");
    let mid = k.amounts([0.005; 3]);
    assert!((mid[0] - 0.505).abs() < 1e-5 && (mid[2] - 1.515).abs() < 1e-4, "between two nodes: {mid:?}");
    // Each layer reads its own channel's share alone.
    let apart = k.amounts([0.0, 0.5, 1.0]);
    assert!((apart[0] - 0.5).abs() < 1e-5 && (apart[1] - 2.0).abs() < 1e-4 && (apart[2] - 4.5).abs() < 1e-4, "{apart:?}");
    // Twin: the node below read alone misses the half step.
    assert!((k.amounts([0.0; 3])[0] - 0.505).abs() > 4e-3);
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
fn brightness_moves_the_three_shares_by_one_shift_as_far_as_their_mean_lies_past_the_pick() {
    let k = Paper { brightness: 1.0, ..narrow() };
    // Past the reach, the three shares a stop of light toward the white: log10 2 of the latitude each.
    let far = [1.1, 1.3, 1.2];
    let moved = k.brightened(far);
    assert!((0..3).all(|c| (moved[c] - far[c] - std::f32::consts::LOG10_2).abs() < 1e-6), "{moved:?}");
    // Within the reach, one shift on the three, their differences kept, a part of the stop.
    let between = [0.62, 0.58, 0.6];
    let shifted = k.brightened(between);
    let by: V3 = map(between, |c, v| shifted[c] - v);
    assert!(by[0] > 0.01 && (by[0] - by[1]).abs() < 1e-7 && (by[0] - by[2]).abs() < 1e-7, "{by:?}");
    assert!(by[0] < std::f32::consts::LOG10_2 - 0.01, "{by:?}");
    // A wider latitude moves the shares less: a stop is less of its window.
    let wide = Paper { latitude: 2.0, ..k }.brightened(far);
    assert!((wide[0] - far[0] - 0.5 * std::f32::consts::LOG10_2).abs() < 1e-6, "{wide:?}");
    // The pick and the tones beyond it print where they did.
    let rest = narrow();
    for shares in [[0.5; 3], [0.4, 0.3, 0.2]] {
        assert_eq!(k.shown(shares), rest.shown(shares), "{shares:?}");
    }
    // Twin: each share moved by its own distance past the pick lays different shifts on them.
    let own: V3 = map(between, |_, v| v + lifted(v - k.pick_share, std::f32::consts::LOG10_2, k.reach) - (v - k.pick_share));
    assert!((own[0] - between[0] - (own[2] - between[2])).abs() > 1e-3, "{own:?}");
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
    // The primaries lay each density's light in the working space; twin: the identity keeps it.
    let turned = Paper { primaries: [[0.6, 0.3, 0.1], [0.1, 0.8, 0.1], [0.0, 0.1, 0.9]], ..k };
    let mixed = turned.viewed([0.5, 1.0, 2.0]);
    assert!((mixed[0] - (0.6 * out[0] + 0.3 * out[1] + 0.1 * out[2])).abs() < 1e-7 && (mixed[0] - out[0]).abs() > 0.05, "{mixed:?}");
    // A black and white print reads its luma on every channel.
    let mono = Paper { luma: true, ..k }.printed([0.3, 0.6, 0.9]);
    assert!(mono[0] == mono[1] && mono[1] == mono[2]);
    assert_ne!(k.printed([0.3, 0.6, 0.9])[0], k.printed([0.3, 0.6, 0.9])[1]);
}

#[test]
fn each_channel_reads_its_own_dye_and_the_others_crosstalk_over_the_d_min() {
    let crossed = [[1.0, 0.07, 0.01], [0.11, 1.0, 0.09], [0.04, 0.09, 1.0]];
    let k = Paper { floor: [0.08, 0.1, 0.06], crosstalk: crossed, ..narrow() };
    let a = [0.5, 1.0, 1.5];
    let d = k.dyed(a);
    let want = [0.08 + 0.5 + 0.07 + 0.015, 0.1 + 0.055 + 1.0 + 0.135, 0.06 + 0.02 + 0.09 + 1.5];
    assert!((0..3).all(|c| (d[c] - want[c]).abs() < 1e-6), "{d:?}");
    // Twin: the crosstalk turned, a row read as a column, reads other densities.
    let turned: [V3; 3] = std::array::from_fn(|r| std::array::from_fn(|c| crossed[c][r]));
    assert!((Paper { crosstalk: turned, ..k }.dyed(a)[0] - d[0]).abs() > 0.05);
    // Pure dyes on no D-min print the amounts as the densities, to the bit.
    let pure = narrow();
    for shares in [[-0.4, 0.2, 0.9], [0.3, 1.0, 1.4], [3.0, -3.0, 0.5]] {
        assert_eq!(pure.dyed(pure.amounts(shares)), pure.amounts(shares), "{shares:?}");
        assert_eq!(pure.printed(shares), viewed(pure.amounts(shares)));
    }
}
