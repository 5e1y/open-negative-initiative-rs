//! Film grain: a texture in stops on a lattice anchored on the scan's full-resolution grid, each output
//! pixel the field's mean over its own footprint; its share and its colour read off the pixel's light.

use super::*;

/// The widest footprint averaged node by node, in cells: seven nodes an axis at most.
pub const GRAIN_FOOTPRINT_CAP: f32 = 3.0;

/// The table's nodes, evenly spaced in stops of the scene's light from `GRAIN_LOW_STOPS` to
/// `GRAIN_HIGH_STOPS`: 4.2 a stop, so the curve's fall to none spans about nine.
pub const GRAIN_NODES: usize = 64;
pub const GRAIN_LOW_STOPS: f32 = -14.0;
pub const GRAIN_HIGH_STOPS: f32 = 1.0;

/// The scene's light at node `i` of the table.
pub fn grain_node_light(i: usize) -> f32 {
    (GRAIN_LOW_STOPS + i as f32 / nodes_per_stop()).exp2()
}

fn nodes_per_stop() -> f32 {
    (GRAIN_NODES - 1) as f32 / (GRAIN_HIGH_STOPS - GRAIN_LOW_STOPS)
}

/// Where `stops` comes from: the field's spread on the full-resolution grid and on this render's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grain {
    /// An output pixel's centre to its point on the scan's full-resolution grid, in rows.
    pub scan: [[f32; 3]; 2],
    /// One over the lattice's cell side, in full-resolution pixels.
    pub per_cell: f32,
    /// Half the footprint the mean spans, in cells, at most half `GRAIN_FOOTPRINT_CAP`.
    pub reach: f32,
    /// Stops a unit of the mean moves the light by.
    pub stops: f32,
    /// The share of `stops` laid at each node's light, from 0 to 1.
    pub share: [f32; GRAIN_NODES],
    /// The colour at each node's light: 1 moves each channel as far as the light.
    pub colour: [f32; GRAIN_NODES],
}

impl Grain {
    /// `spread` stops at full resolution on cells of `cell` full-resolution pixels, rendered at `scale`,
    /// each light taking its `share`; past the cap the cap's mean stands in, scaled to the whole footprint's spread. No colour.
    pub fn on(scan: [[f32; 3]; 2], cell: f32, scale: f32, spread: f32, share: [f32; GRAIN_NODES]) -> Self {
        let full = 1.0 / cell;
        let here = full / scale;
        let kept = here.min(GRAIN_FOOTPRINT_CAP);
        let stops = spread * mean_spread(here) / (mean_spread(full) * mean_spread(kept));
        Grain { scan, per_cell: 1.0 / cell, reach: kept / 2.0, stops, share, colour: [0.0; GRAIN_NODES] }
    }

    /// This grain coloured by `colour` at each node's light.
    pub fn coloured(self, colour: [f32; GRAIN_NODES]) -> Self {
        Grain { colour, ..self }
    }

    /// The lattice's rows, the dose, then a row a node holding its share, the next one's, its colour and
    /// the next one's, so the shader reads one row a pixel and indexes no vector's lane.
    pub fn rows(&self) -> Vec<V4> {
        let [x, y] = self.scan;
        let mut rows = vec![[x[0], x[1], x[2], self.per_cell], [y[0], y[1], y[2], self.reach], [self.stops, GRAIN_LOW_STOPS, nodes_per_stop(), 0.0]];
        let next = |i: usize| (i + 1).min(GRAIN_NODES - 1);
        rows.extend((0..GRAIN_NODES).map(|i| [self.share[i], self.share[next(i)], self.colour[i], self.colour[next(i)]]));
        rows
    }
}

/// Where light `y` falls on the table's stops: its node and the way to the next one.
fn place(y: f32) -> (usize, f32) {
    let t = ((y.max(1e-30).log2() - GRAIN_LOW_STOPS) * nodes_per_stop()).clamp(0.0, (GRAIN_NODES - 1) as f32);
    let i = (t.floor() as usize).min(GRAIN_NODES - 2);
    (i, t - i as f32)
}

/// `table` read at a place, a line between two nodes.
fn between(table: &[f32; GRAIN_NODES], (i, f): (usize, f32)) -> f32 {
    table[i] + (table[i + 1] - table[i]) * f
}

/// The share of the grain at light `y`.
pub(crate) fn share(y: f32, k: &Grain) -> f32 {
    between(&k.share, place(y))
}

/// `∫ B` from −2 to `u`, `B` the cubic B-spline the lattice is interpolated with.
pub(crate) fn ramp(u: f32) -> f32 {
    let a = u.abs().min(2.0);
    let far = 2.0 - a;
    let tail = if a >= 1.0 { far * far * far * far / 24.0 } else { 0.5 - (4.0 * a - 2.0 * a * a * a + 0.75 * a * a * a * a) / 6.0 };
    if u >= 0.0 { 1.0 - tail } else { tail }
}

/// Four uniforms at lattice node `i`: PCG-2D's hash, exact on the GPU too.
pub(crate) fn hash(i: [i32; 2]) -> V4 {
    let mut v = [i[0] as u32, i[1] as u32].map(|x| x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223));
    v[0] = v[0].wrapping_add(v[1].wrapping_mul(1_664_525));
    v[1] = v[1].wrapping_add(v[0].wrapping_mul(1_664_525));
    v = v.map(|x| x ^ (x >> 16));
    v[0] = v[0].wrapping_add(v[1].wrapping_mul(1_664_525));
    v[1] = v[1].wrapping_add(v[0].wrapping_mul(1_664_525));
    v = v.map(|x| x ^ (x >> 16));
    [v[0] & 0xffff, v[0] >> 16, v[1] & 0xffff, v[1] >> 16].map(|n| (n as f32 + 0.5) * (1.0 / 65536.0))
}

/// The light's standard normal: the four uniforms summed, so the GPU needs no transcendental.
pub(crate) fn normal(u: V4) -> f32 {
    (u[0] + u[1] + u[2] + u[3] - 2.0) * 1.732_050_8
}

/// Three more normals of the same uniforms, the light's orthogonal Hadamard combinations.
pub(crate) fn chroma(u: V4) -> V3 {
    [u[0] + u[1] - u[2] - u[3], u[0] - u[1] + u[2] - u[3], u[0] - u[1] - u[2] + u[3]].map(|v| v * 1.732_050_8)
}

/// Each channel's colour offset of the field's chroma, unit spread and zero sum, so the log mean stays the light's.
pub(crate) fn offsets(c: V3) -> V3 {
    let mean = (c[0] + c[1] + c[2]) / 3.0;
    c.map(|v| (v - mean) * 1.224_744_9)
}

/// The first node whose spline reaches `[lo, hi]` and each node's share of the mean over it.
fn weights(lo: f32, hi: f32) -> (i32, Vec<f32>) {
    let first = lo.floor() as i32 - 1;
    let count = (hi.ceil() as i32 + 2 - first).min(8);
    let span = hi - lo;
    (first, (0..count).map(|j| (ramp(hi - (first + j) as f32) - ramp(lo - (first + j) as f32)) / span).collect())
}

/// The spread of the field's mean over `footprint` cells, both axes, averaged over where it falls.
pub fn mean_spread(footprint: f32) -> f32 {
    const PLACES: usize = 64;
    let f = f64::from(footprint);
    let per_axis: f64 = (0..PLACES)
        .map(|k| {
            let t = (k as f64 + 0.5) / PLACES as f64;
            let (lo, hi) = (t - f / 2.0, t + f / 2.0);
            let first = lo.floor() as i64 - 1;
            (first..=hi.ceil() as i64 + 1)
                .map(|i| {
                    let w = (f64::from(ramp((hi - i as f64) as f32)) - f64::from(ramp((lo - i as f64) as f32))) / f;
                    w * w
                })
                .sum::<f64>()
        })
        .sum::<f64>()
        / PLACES as f64;
    per_axis as f32
}

/// The field's mean over the footprint of the pixel whose centre is `at`: the light, then its chroma.
pub(crate) fn field(at: V2, k: &Grain) -> V4 {
    let [x, y] = k.scan;
    let q = [(x[0] * at[0] + x[1] * at[1] + x[2]) * k.per_cell, (y[0] * at[0] + y[1] * at[1] + y[2]) * k.per_cell];
    let (fx, wx) = weights(q[0] - k.reach, q[0] + k.reach);
    let (fy, wy) = weights(q[1] - k.reach, q[1] + k.reach);
    let mut field = [0.0; 4];
    for (r, wr) in wy.iter().enumerate() {
        let mut row = [0.0; 4];
        for (c, wc) in wx.iter().enumerate() {
            let u = hash([fx + c as i32, fy + r as i32]);
            let [a, b, d] = chroma(u);
            for (lane, n) in row.iter_mut().zip([normal(u), a, b, d]) {
                *lane += wc * n;
            }
        }
        for (lane, v) in field.iter_mut().zip(row) {
            *lane += wr * v;
        }
    }
    field
}

/// `s` under `field` at its light's share, its colour offsets `d` scaled by the colour at its light.
pub(crate) fn laid(s: V4, field: V4, d: V3, k: &Grain) -> V4 {
    let at = place(dot(rgb(s), LUMA));
    let (share, colour) = (between(&k.share, at), between(&k.colour, at));
    with_alpha(map(rgb(s), |c, v| v * (k.stops * share * (field[0] + colour * d[c])).exp2()), s[3])
}

/// The grain at the pixel whose centre is `at`, in the graph's pixels; a light given no share is kept.
pub fn grain(s: V4, at: V2, k: &Grain) -> V4 {
    if share(dot(rgb(s), LUMA), k) <= 0.0 {
        return s;
    }
    let f = field(at, k);
    laid(s, f, offsets([f[1], f[2], f[3]]), k)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHOLE: [f32; GRAIN_NODES] = [1.0; GRAIN_NODES];

    /// A colour on a straight line over the table's nodes, from `first` to `last`.
    fn line(first: f32, last: f32) -> [f32; GRAIN_NODES] {
        std::array::from_fn(|i| first + (last - first) * i as f32 / (GRAIN_NODES - 1) as f32)
    }

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

    /// The correlation of two of a node's normals over 256² nodes.
    fn correlation(a: impl Fn(V4) -> f32, b: impl Fn(V4) -> f32) -> f64 {
        let n = 256;
        let nodes = (0..n).flat_map(|y| (0..n).map(move |x| hash([x - n / 2, y - n / 2])));
        let (mut ab, mut aa, mut bb) = (0.0f64, 0.0f64, 0.0f64);
        for u in nodes {
            let (x, y) = (f64::from(a(u)), f64::from(b(u)));
            (ab, aa, bb) = (ab + x * y, aa + x * x, bb + y * y);
        }
        ab / (aa * bb).sqrt()
    }

    #[test]
    fn the_chroma_nodes_are_standard_normals_uncorrelated_with_each_other_and_the_light() {
        for lane in 0..3 {
            let (mean, var, next) = moments(|i| chroma(hash(i))[lane]);
            assert!(mean.abs() < 0.01 && (var - 1.0).abs() < 0.02 && next < 0.01, "{lane}: mean {mean}, variance {var}, neighbours {next}");
            let light = correlation(normal, |u| chroma(u)[lane]);
            let other = correlation(|u| chroma(u)[lane], |u| chroma(u)[(lane + 1) % 3]);
            assert!(light.abs() < 0.01 && other.abs() < 0.01, "{lane}: {light} with the light, {other} with the next");
        }
        // Twin: a chroma lane that is the light itself.
        assert!(correlation(normal, normal) > 0.99);
    }

    #[test]
    fn the_colour_moves_no_log_mean_and_rests_monochrome_at_zero() {
        let k = Grain::on([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], 0.7, 1.0, 0.4, WHOLE).coloured(line(2.0, 0.5));
        let mono = Grain { colour: [0.0; GRAIN_NODES], ..k };
        let s = [0.05, 0.02, 0.2, 1.0];
        let sum = |p: V4| (0..3).map(|c| (p[c] / s[c]).log2()).sum::<f32>();
        let mut skewed = 0.0f32;
        for at in [[3.25f32, 10.5], [40.0, 7.75], [-12.5, 3.0], [101.5, 55.5]] {
            let (coloured, plain) = (grain(s, at, &k), grain(s, at, &mono));
            assert!((sum(coloured) - sum(plain)).abs() < 1e-4, "at {at:?}: {} against {}", sum(coloured), sum(plain));
            let f = (plain[0] / s[0]).log2();
            assert!((0..3).all(|c| ((plain[c] / s[c]).log2() - f).abs() < 1e-5), "at {at:?}: at zero every channel moves as one");
            assert!((0..3).any(|c| ((coloured[c] / s[c]).log2() - f).abs() > 1e-3), "at {at:?}: the colour moved no channel");
            // Twin: an offset that keeps the luma instead, whose channels do not sum to nothing.
            let v = field(at, &k);
            let c = [v[1], v[2], v[3]];
            skewed = skewed.max((sum(laid(s, v, map(c, |_, x| x - dot(c, LUMA)), &k)) - sum(plain)).abs());
        }
        assert!(skewed > 1e-2, "the twin's log mean moved {skewed}");
    }

    #[test]
    fn three_channels_of_equal_spread_cast_no_colour_on_a_flat_grey() {
        let k = Grain::on([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], 1.0, 1.0, 0.3, WHOLE).coloured([2.0; GRAIN_NODES]);
        let side = 1024;
        let grey = [0.1, 0.1, 0.1, 1.0];
        let (mut kept, mut luma) = ([0.0f64; 3], [0.0f64; 3]);
        for y in 0..side {
            for x in 0..side {
                let v = field([x as f32 + 0.5, y as f32 + 0.5], &k);
                let c = [v[1], v[2], v[3]];
                let (a, b) = (laid(grey, v, offsets(c), &k), laid(grey, v, map(c, |_, x| x - dot(c, LUMA)), &k));
                for ch in 0..3 {
                    kept[ch] += f64::from(a[ch]);
                    luma[ch] += f64::from(b[ch]);
                }
            }
        }
        let cast = |m: [f64; 3]| {
            let mean = m.iter().sum::<f64>() / 3.0;
            m.iter().map(|v| (v / mean - 1.0).abs()).fold(0.0, f64::max)
        };
        println!("the channels' means part by {:.5}, the twin's by {:.5}", cast(kept), cast(luma));
        assert!(cast(kept) < 0.005, "the channels' means part by {:.4}", cast(kept));
        // Twin: the luma-keeping offset, blue's variance eight times green's.
        assert!(cast(luma) > 0.05, "the twin casts {:.4}", cast(luma));
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

    #[test]
    fn the_share_scales_the_colour_with_the_light() {
        let whole = Grain::on([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], 0.7, 1.0, 0.4, WHOLE).coloured([2.0; GRAIN_NODES]);
        let half = Grain { share: [0.5; GRAIN_NODES], ..whole };
        let s = [0.05, 0.02, 0.2, 1.0];
        for at in [[3.25f32, 10.5], [40.0, 7.75]] {
            let (a, b) = (grain(s, at, &whole), grain(s, at, &half));
            assert!((0..3).all(|c| ((b[c] / s[c]).log2() * 2.0 - (a[c] / s[c]).log2()).abs() < 1e-5), "at {at:?}: {a:?} against {b:?}");
            // Twin: the colour laid whole on the half share.
            let v = field(at, &whole);
            let d = offsets([v[1], v[2], v[3]]);
            let unshared = map(rgb(s), |c, x| x * (half.stops * (0.5 * v[0] + 2.0 * d[c])).exp2());
            assert!((0..3).any(|c| ((unshared[c] / s[c]).log2() - (b[c] / s[c]).log2()).abs() > 1e-3), "at {at:?}");
        }
        // A light given no share is kept to the bit.
        assert_eq!(grain(s, [3.25, 10.5], &Grain { share: [0.0; GRAIN_NODES], ..whole }), s);
    }
}
