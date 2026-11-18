//! Film grain: a monochrome texture in stops on a lattice anchored on the scan's full-resolution grid,
//! each output pixel the field's mean over its own footprint.

use super::*;

/// The widest footprint averaged node by node, in cells: seven nodes an axis at most.
pub const GRAIN_FOOTPRINT_CAP: f32 = 3.0;

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
}

impl Grain {
    /// `spread` stops at full resolution on cells of `cell` full-resolution pixels, rendered at `scale`;
    /// past the cap the cap's mean stands in, scaled to the whole footprint's spread.
    pub fn on(scan: [[f32; 3]; 2], cell: f32, scale: f32, spread: f32) -> Self {
        let full = 1.0 / cell;
        let here = full / scale;
        let kept = here.min(GRAIN_FOOTPRINT_CAP);
        let stops = spread * mean_spread(here) / (mean_spread(full) * mean_spread(kept));
        Grain { scan, per_cell: 1.0 / cell, reach: kept / 2.0, stops }
    }

    pub fn rows(&self) -> Vec<V4> {
        let [x, y] = self.scan;
        vec![[x[0], x[1], x[2], self.per_cell], [y[0], y[1], y[2], self.reach], [self.stops, 0.0, 0.0, 0.0]]
    }
}

/// `∫ B` from −2 to `u`, `B` the cubic B-spline the lattice is interpolated with.
pub(crate) fn ramp(u: f32) -> f32 {
    let a = u.abs().min(2.0);
    let far = 2.0 - a;
    let tail = if a >= 1.0 { far * far * far * far / 24.0 } else { 0.5 - (4.0 * a - 2.0 * a * a * a + 0.75 * a * a * a * a) / 6.0 };
    if u >= 0.0 { 1.0 - tail } else { tail }
}

/// A standard normal at lattice node `i`: PCG-2D's hash, four 16-bit uniforms summed, so the GPU
/// reaches the same value with no transcendental.
pub(crate) fn normal(i: [i32; 2]) -> f32 {
    let mut v = [i[0] as u32, i[1] as u32].map(|x| x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223));
    v[0] = v[0].wrapping_add(v[1].wrapping_mul(1_664_525));
    v[1] = v[1].wrapping_add(v[0].wrapping_mul(1_664_525));
    v = v.map(|x| x ^ (x >> 16));
    v[0] = v[0].wrapping_add(v[1].wrapping_mul(1_664_525));
    v[1] = v[1].wrapping_add(v[0].wrapping_mul(1_664_525));
    v = v.map(|x| x ^ (x >> 16));
    let u = [v[0] & 0xffff, v[0] >> 16, v[1] & 0xffff, v[1] >> 16].map(|n| (n as f32 + 0.5) * (1.0 / 65536.0));
    (u[0] + u[1] + u[2] + u[3] - 2.0) * 1.732_050_8
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

/// The grain at the pixel whose centre is `at`, in the graph's pixels.
pub fn grain(s: V4, at: V2, k: &Grain) -> V4 {
    let [x, y] = k.scan;
    let q = [(x[0] * at[0] + x[1] * at[1] + x[2]) * k.per_cell, (y[0] * at[0] + y[1] * at[1] + y[2]) * k.per_cell];
    let (fx, wx) = weights(q[0] - k.reach, q[0] + k.reach);
    let (fy, wy) = weights(q[1] - k.reach, q[1] + k.reach);
    let mut field = 0.0;
    for (r, wr) in wy.iter().enumerate() {
        let mut row = 0.0;
        for (c, wc) in wx.iter().enumerate() {
            row += wc * normal([fx + c as i32, fy + r as i32]);
        }
        field += wr * row;
    }
    let f = (k.stops * field).exp2();
    with_alpha(map(rgb(s), |_, v| v * f), s[3])
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let (mean, var, next) = moments(normal);
        assert!(mean.abs() < 0.01 && (var - 1.0).abs() < 0.02 && next < 0.01, "mean {mean}, variance {var}, neighbours {next}");
        // Twin: a hash of one coordinate alone repeats down the other.
        let (_, _, striped) = moments(|i| normal([i[0], 0]));
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
        let k = |reach: f32| Grain { scan: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], per_cell: 0.7, reach, stops: 1.0 };
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
}
