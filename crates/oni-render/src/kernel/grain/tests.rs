//! The grain's lattice, its table and its colour against their laws, each verdict with a twin.

use super::*;

const WHOLE: [f32; GRAIN_NODES] = [1.0; GRAIN_NODES];
/// The colour slider's top.
const TOP: f32 = 3.0;

#[test]
fn the_ramp_is_the_splines_integral() {
    let b = |u: f32| {
        let a = u.abs();
        if a >= 2.0 {
            0.0
        } else if a >= 1.0 {
            (2.0 - a).powi(3) / 6.0
        } else {
            (4.0 - 6.0 * a * a + 3.0 * a * a * a) / 6.0
        }
    };
    let steps = 4000;
    let mut sum = 0.0f64;
    for k in 0..steps {
        let u = -2.0 + 4.0 * (k as f32 + 0.5) / steps as f32;
        sum += f64::from(b(u)) * 4.0 / f64::from(steps);
        let at = u + 2.0 / steps as f32;
        assert!((ramp(at) as f64 - sum).abs() < 1e-4, "at {at}: {} against {sum}", ramp(at));
    }
    assert_eq!((ramp(-2.5), ramp(2.5), ramp(0.0)), (0.0, 1.0, 0.5));
    // Twin: the quadratic spline's integral, a narrower bell, reaches 5/6 at a half.
    assert!((ramp(0.5) - 5.0 / 6.0).abs() > 0.01);
}

/// Mean, variance and the largest correlation with the next node across or down, on 256² nodes.
fn moments(node: impl Fn([i32; 2]) -> f32) -> (f64, f64, f64) {
    let n = 256;
    let at = |x: i32, y: i32| f64::from(node([x - n / 2, y - n / 2]));
    let all = (n * n) as f64;
    let mean = (0..n).flat_map(|y| (0..n).map(move |x| (x, y))).map(|(x, y)| at(x, y)).sum::<f64>() / all;
    let var = (0..n).flat_map(|y| (0..n).map(move |x| (x, y))).map(|(x, y)| (at(x, y) - mean).powi(2)).sum::<f64>() / all;
    let next = |dx: i32, dy: i32| (0..n - 1).flat_map(|y| (0..n - 1).map(move |x| (x, y))).map(|(x, y)| (at(x, y) - mean) * (at(x + dx, y + dy) - mean)).sum::<f64>() / all / var;
    (mean, var, next(1, 0).abs().max(next(0, 1).abs()))
}

#[test]
fn the_nodes_are_standard_normals_and_uncorrelated() {
    let (mean, var, next) = moments(|i| normal(hash(i)));
    assert!(mean.abs() < 0.01 && (var - 1.0).abs() < 0.02 && next < 0.01, "mean {mean}, variance {var}, neighbours {next}");
    // Twin: a hash of one coordinate alone repeats down the other.
    let (_, _, striped) = moments(|i| normal(hash([i[0], 0])));
    assert!(striped > 0.5, "the twin's neighbours correlate at {striped}");
}

#[test]
fn a_point_s_spread_is_the_splines_and_a_mean_s_falls_as_its_footprint_grows() {
    // A point: ∫ B² of the cubic B-spline, 151/315 an axis.
    assert!((mean_spread(1e-3) - 151.0 / 315.0).abs() < 1e-4, "{}", mean_spread(1e-3));
    // A wide footprint: the mean of F cells falls as 1/F, less the spline's reach.
    for f in [8.0f32, 20.0, 50.0] {
        let wide = mean_spread(f);
        assert!((wide * f - 1.0).abs() < 1.0 / f, "{f}: {wide}");
    }
    // Twin: the spread of a sum, not of a mean, grows instead.
    assert!(mean_spread(2.0) < mean_spread(1.0));
}

#[test]
fn the_mean_over_a_footprint_is_the_mean_of_its_quarters() {
    let k = |reach: f32| Grain { per_cell: 0.7, reach, stops: 1.0, ..Grain::on([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], 1.0, 1.0, 1.0, WHOLE) };
    let field = |at: V2, reach: f32| grain([1.0; 4], at, &k(reach))[0].log2();
    for at in [[3.25f32, 10.5], [40.0, 7.75], [-12.5, 3.0]] {
        // Two pixels a side, 1.4 cells, against its four pixels of 0.7.
        let whole = field(at, 0.7);
        let quarters = [(-0.5, -0.5), (0.5, -0.5), (-0.5, 0.5), (0.5, 0.5)].map(|(dx, dy)| field([at[0] + dx, at[1] + dy], 0.35));
        let mean = quarters.iter().sum::<f32>() / 4.0;
        assert!((whole - mean).abs() < 1e-5, "at {at:?}: {whole} against {mean}");
        // Twin: the field at the centre alone is not the mean.
        assert!((field(at, 1e-3) - whole).abs() > 1e-3, "at {at:?}");
    }
}

#[test]
fn the_share_and_the_colour_pass_through_each_node_and_hold_past_the_ends() {
    let mut k = Grain::on([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], 1.0, 1.0, 1.0, [0.0; GRAIN_NODES]);
    k.share.iter_mut().enumerate().for_each(|(i, v)| *v = (i as f32 * 0.37).sin().abs());
    k.colour.iter_mut().enumerate().for_each(|(i, v)| *v = 2.0 * (i as f32 * 0.21).cos().abs());
    let colour = |y: f32, k: &Grain| between(&k.colour, place(y));
    for i in 0..GRAIN_NODES {
        let y = grain_node_light(i);
        assert!((share(y, &k) - k.share[i]).abs() < 1e-5 && (colour(y, &k) - k.colour[i]).abs() < 1e-5, "node {i}");
    }
    let half = (grain_node_light(10) * grain_node_light(11)).sqrt();
    assert!((share(half, &k) - (k.share[10] + k.share[11]) / 2.0).abs() < 1e-5, "halfway in stops");
    assert!((colour(half, &k) - (k.colour[10] + k.colour[11]) / 2.0).abs() < 1e-5, "halfway in stops");
    assert_eq!((share(0.0, &k), share(-1.0, &k), share(8.0, &k)), (k.share[0], k.share[0], k.share[GRAIN_NODES - 1]));
    // Twin: the table read a node over misses every node.
    let mut over = k;
    over.share.rotate_left(1);
    let gap = (0..GRAIN_NODES).map(|i| (share(grain_node_light(i), &over) - k.share[i]).abs()).fold(0.0, f32::max);
    assert!(gap > 0.1, "the twin passes at {gap}");
}


/// A grain of 0.4 stop on cells of 0.7 pixel, the whole share and `colour` at every light.
fn at_colour(colour: f32) -> Grain {
    Grain::on([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], 0.7, 1.0, 0.4, WHOLE).coloured([colour; GRAIN_NODES])
}

/// 4096 pixel centres, 64 a side.
fn centres() -> impl Iterator<Item = V2> {
    (0..64).flat_map(|y| (0..64).map(move |x| [x as f32 + 0.5, y as f32 + 0.5]))
}

/// The stops the pixel's red lies over its blue: its colour, in a log space the law scales.
fn red_over_blue(p: V4) -> f32 {
    (p[0] / p[2]).log2()
}

/// The zero-sum offsets of a coloured grain drawn at random: three more normals of each node's hash.
fn random_offsets(at: V2, k: &Grain) -> V3 {
    let [x, y] = k.scan;
    let q = [(x[0] * at[0] + x[1] * at[1] + x[2]) * k.per_cell, (y[0] * at[0] + y[1] * at[1] + y[2]) * k.per_cell];
    let ((fx, wx), (fy, wy)) = (weights(q[0] - k.reach, q[0] + k.reach), weights(q[1] - k.reach, q[1] + k.reach));
    let mut c = [0.0f32; 3];
    for (r, wr) in wy.iter().enumerate() {
        for (j, wc) in wx.iter().enumerate() {
            let u = hash([fx + j as i32, fy + r as i32]);
            let n = [u[0] + u[1] - u[2] - u[3], u[0] - u[1] + u[2] - u[3], u[0] - u[1] - u[2] + u[3]];
            (0..3).for_each(|l| c[l] += wr * wc * n[l] * 1.732_050_8);
        }
    }
    let mean = (c[0] + c[1] + c[2]) / 3.0;
    c.map(|v| (v - mean) * 1.224_744_9)
}

#[test]
fn a_neutral_pixel_stays_neutral_to_the_bit() {
    let (k, mono) = (at_colour(TOP), at_colour(0.0));
    let (mut parted, mut off, mut twin) = (0usize, 0.0f32, 0.0f32);
    for light in [0.001f32, 0.01, 0.05, 0.2, 0.8] {
        let grey = [light, light, light, 1.0];
        for at in centres() {
            let (p, m) = (grain(grey, at, &k), grain(grey, at, &mono));
            parted += usize::from(p[0].to_bits() != p[1].to_bits() || p[1].to_bits() != p[2].to_bits());
            off = off.max((p[0] / m[0] - 1.0).abs());
            // Twin: a hue drawn at random each clump, a colour out of nothing.
            let lanes = random_offsets(at, &k).map(|d| k.stops * (field(at, &k) + TOP * d));
            twin = twin.max(lanes.iter().fold(f32::MIN, |a, &b| a.max(b)) - lanes.iter().fold(f32::MAX, |a, &b| a.min(b)));
        }
    }
    assert!(parted == 0 && off < 1e-6, "{parted} greys part, {off:.2e} off the monochrome grain");
    assert!(twin > 0.01, "the twin parts a grey by {twin} stop");
}

#[test]
fn the_colour_deepens_the_pixel_s_own_hue_only_where_the_grain_darkens_it() {
    let (k, mono) = (at_colour(2.0), at_colour(0.0));
    let skin = [0.05, 0.032, 0.0215, 1.0];
    // The symmetric law, which greys the lighter grains as much as it deepens the darker.
    let symmetric = |s: V4, a: f32| {
        let (y, gamma) = (dot(rgb(s), LUMA), (-2.0 * a).exp2());
        let p = map(rgb(s), |_, v| (v / y).powf(gamma));
        with_alpha(map(p, |_, v| v * a.exp2() * y / dot(p, LUMA)), s[3])
    };
    let (mut lighter, mut darker, mut worst, mut greyed) = (0usize, 0usize, 0.0f32, 0usize);
    for at in centres() {
        let a = k.stops * field(at, &k);
        let (p, m) = (grain(skin, at, &k), grain(skin, at, &mono));
        if a >= 0.0 {
            lighter += 1;
            assert_eq!(p, m, "at {at:?}: a lighter grain moves the colour");
            greyed += usize::from((red_over_blue(symmetric(skin, a)) / red_over_blue(m) - 1.0).abs() > 1e-3);
        } else {
            darker += 1;
            // The colour in stops scales by 2^(colour × the stops darkened), so it rises with them.
            worst = worst.max((red_over_blue(p) / red_over_blue(skin) / (2.0 * -a).exp2() - 1.0).abs());
        }
    }
    assert!(lighter > 1000 && darker > 1000, "{lighter} lighter, {darker} darker");
    assert!(worst < 1e-4, "the colour is {worst:.2e} off its law");
    // Twin: the symmetric law moves the lighter grains' colour.
    assert!(greyed > lighter / 2, "the twin moves {greyed} of {lighter}");
}

#[test]
fn the_luma_is_the_monochrome_grain_s_and_the_channels_keep_their_order() {
    let (k, mono) = (at_colour(TOP), at_colour(0.0));
    let patches = [[0.05, 0.032, 0.0215, 1.0], [0.02, 0.0024, 0.0016, 1.0], [0.003, 0.0045, 0.03, 1.0], [0.03, 0.055, 0.015, 1.0]];
    let (mut off, mut twin) = (0.0f32, 0.0f32);
    for (n, s) in patches.into_iter().enumerate() {
        for at in centres() {
            let (p, m) = (grain(s, at, &k), grain(s, at, &mono));
            off = off.max((dot(rgb(p), LUMA) / dot(rgb(m), LUMA) - 1.0).abs());
            for (i, j) in [(0, 1), (1, 2), (0, 2)] {
                assert_eq!(s[i] > s[j], p[i] > p[j], "at {at:?}: channels {i} and {j} swap");
            }
            // Twin, on the red: the vignette's renormalisation, which puts back a power mean, not the luma.
            let power = (TOP * (-k.stops * field(at, &k)).max(0.0)).exp2();
            let e = map(rgb(m), |_, v| v.powf(power));
            let back = map(e, |_, v| v * dot(e, LUMA).powf(1.0 / power - 1.0));
            if n == 1 {
                twin = twin.max((dot(back, LUMA) / dot(rgb(m), LUMA) - 1.0).abs());
            }
        }
    }
    assert!(off < 1e-6, "the luma is {off:.2e} off the monochrome grain's");
    assert!(twin > 1e-3, "the twin's luma is {twin:.2e} off");
}

#[test]
fn a_channel_at_or_under_zero_carries_no_chroma_and_is_kept() {
    let k = at_colour(TOP);
    let (zero, under, black, dark) = ([0.04, 0.0, 0.02, 1.0], [0.04, -0.003, 0.02, 1.0], [0.0, 0.0, 0.0, 1.0], [-0.01, 0.002, 0.001, 1.0]);
    let mut twin = 0.0f32;
    for at in centres().take(512) {
        let a = k.stops * field(at, &k);
        for s in [zero, under, black, dark] {
            assert!(grain(s, at, &k).iter().all(|v| v.is_finite()), "at {at:?}: {s:?}");
        }
        assert_eq!(grain(zero, at, &k)[1], 0.0, "at {at:?}: a zero channel stays zero");
        assert_eq!(grain(black, at, &k), black);
        assert_eq!(grain(dark, at, &k), with_alpha(map(rgb(dark), |_, v| v * a.exp2()), 1.0), "at {at:?}: a luma under zero takes the grain alone");
        // Under zero, the channel stays under and only follows the luma, the grain's.
        let (p, y) = (grain(under, at, &k), dot(rgb(under), LUMA));
        assert!(p[1] < 0.0 && (dot(rgb(p), LUMA) / (a.exp2() * y) - 1.0).abs() < 1e-6, "at {at:?}: {p:?}");
        // Twin: the channel floored at zero first.
        let floored = laid(with_alpha(map(rgb(under), |_, v| v.max(0.0)), 1.0), a, TOP);
        twin = twin.max((floored[1] - p[1]).abs() / under[1].abs());
    }
    assert!(twin > 0.5, "the twin moves the channel {twin}");
}

#[test]
fn the_share_scales_the_stops_the_colour_reads() {
    let whole = at_colour(2.0);
    let half = Grain { share: [0.5; GRAIN_NODES], ..whole };
    let s = [0.05, 0.02, 0.2, 1.0];
    let mut twin = 0.0f32;
    for at in centres().take(512) {
        let b = grain(s, at, &half);
        assert_eq!(b, laid(s, half.stops * 0.5 * field(at, &half), 2.0), "at {at:?}");
        // Twin: the colour read off the whole stops, the light's alone halved.
        let a = whole.stops * field(at, &whole);
        let unshared = map(rgb(laid(s, a, 2.0)), |_, v| v * (-0.5 * a).exp2());
        twin = twin.max((0..3).map(|c| (unshared[c] / b[c] - 1.0).abs()).fold(0.0, f32::max));
    }
    assert!(twin > 1e-3, "the twin passes at {twin:.2e}");
    // A light given no share is kept to the bit.
    assert_eq!(grain(s, [3.25, 10.5], &Grain { share: [0.0; GRAIN_NODES], ..whole }), s);
}
