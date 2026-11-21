//! Film grain: a texture in stops on a lattice anchored on the scan's full-resolution grid, each output
//! pixel the field's mean over its own footprint; its share read off the pixel's light, its colour the pixel's own.
//! Inside a clump the scene gives way to its blur under a grain, so detail finer than a grain follows it;
//! between clumps it takes a haze of that blur, so no detail finer than a grain reads sharp through it.

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
    /// The colour at each node's light: a grain `d` stops darker raises each channel's ratio to the luma
    /// to the power 2^(colour·d), deepening the pixel's own hue.
    pub colour: [f32; GRAIN_NODES],
    /// The photo's own arrangement, added to every lattice index: a translation of one field, so each
    /// photo keeps the same statistics. `[0, 0]` is the lattice as it is.
    pub seed: [u32; 2],
    /// The share of the blurred scene a clump takes at the whole share, from 0 to 1.
    pub dissolve: f32,
    /// The share of `dissolve` laid between clumps too, from 0 to 1.
    pub haze: f32,
}

impl Grain {
    /// `spread` stops at full resolution on cells of `cell` full-resolution pixels, rendered at `scale`,
    /// each light taking its `share`; past the cap the cap's mean stands in, scaled to the whole footprint's spread. No colour.
    pub fn on(scan: [[f32; 3]; 2], cell: f32, scale: f32, spread: f32, share: [f32; GRAIN_NODES]) -> Self {
        let full = 1.0 / cell;
        let here = full / scale;
        let kept = here.min(GRAIN_FOOTPRINT_CAP);
        let stops = spread * mean_spread(here) / (mean_spread(full) * mean_spread(kept));
        Grain { scan, per_cell: 1.0 / cell, reach: kept / 2.0, stops, share, colour: [0.0; GRAIN_NODES], seed: [0, 0], dissolve: 0.0, haze: 0.0 }
    }

    /// This grain coloured by `colour` at each node's light.
    pub fn coloured(self, colour: [f32; GRAIN_NODES]) -> Self {
        Grain { colour, ..self }
    }

    /// This grain on the photo of `seed`.
    pub fn seeded(self, seed: [u32; 2]) -> Self {
        Grain { seed, ..self }
    }

    /// This grain taking `dissolve` of the blurred scene in its clumps.
    pub fn dissolving(self, dissolve: f32) -> Self {
        Grain { dissolve, ..self }
    }

    /// This grain laying `haze` of its dissolve between its clumps too.
    pub fn hazed(self, haze: f32) -> Self {
        Grain { haze, ..self }
    }

    /// The uniform's head: the seed.
    pub fn head(&self) -> [u32; 4] {
        [self.seed[0], self.seed[1], 0, 0]
    }

    /// The lattice's rows, the dose, the mix, then a row a node holding its share, the next one's, its
    /// colour and the next one's, so the shader reads one row a pixel and indexes no vector's lane.
    pub fn rows(&self) -> Vec<V4> {
        let [x, y] = self.scan;
        let dose = [self.stops, GRAIN_LOW_STOPS, nodes_per_stop(), 0.0];
        let mix = [self.dissolve, 1.0 / mean_spread(self.per_cell), self.haze, 0.0];
        let mut rows = vec![[x[0], x[1], x[2], self.per_cell], [y[0], y[1], y[2], self.reach], dose, mix];
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

/// Four uniforms at lattice node `i` of the photo of `seed`: PCG-2D's hash, exact on the GPU too.
pub(crate) fn hash(i: [i32; 2], seed: [u32; 2]) -> V4 {
    let mut v = [(i[0] as u32).wrapping_add(seed[0]), (i[1] as u32).wrapping_add(seed[1])].map(|x| x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223));
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

/// The field's mean over the footprint of the pixel whose centre is `at`.
pub(crate) fn field(at: V2, k: &Grain) -> f32 {
    let [x, y] = k.scan;
    let q = [(x[0] * at[0] + x[1] * at[1] + x[2]) * k.per_cell, (y[0] * at[0] + y[1] * at[1] + y[2]) * k.per_cell];
    let (fx, wx) = weights(q[0] - k.reach, q[0] + k.reach);
    let (fy, wy) = weights(q[1] - k.reach, q[1] + k.reach);
    let mut field = 0.0;
    for (r, wr) in wy.iter().enumerate() {
        let mut row = 0.0;
        for (c, wc) in wx.iter().enumerate() {
            row += wc * normal(hash([fx + c as i32, fy + r as i32], k.seed));
        }
        field += wr * row;
    }
    field
}

/// `s` moved `a` stops, each channel's ratio to the luma raised to 2^(colour × the stops it darkens): its
/// own hue deepened, a grey kept grey, its luma the grain's; a channel at or under zero is kept.
pub(crate) fn laid(s: V4, a: f32, colour: f32) -> V4 {
    let k = a.exp2();
    let darker = colour * (-a).max(0.0);
    let y = dot(rgb(s), LUMA);
    let plain = with_alpha(map(rgb(s), |_, v| v * k), s[3]);
    if darker <= 0.0 || y <= 0.0 {
        return plain;
    }
    let gamma = darker.exp2();
    let p = map(rgb(s), |_, v| {
        let r = v / y;
        if r > 0.0 { (gamma * r.log2()).exp2() } else { r }
    });
    let q = dot(p, LUMA);
    if q <= 0.0 {
        return plain;
    }
    with_alpha(map(p, |_, v| v * (k * y / q)), s[3])
}

/// How much of the blurred scene the pixel of `part` takes where the field is `f`: `haze` of `dissolve × part`
/// under half the field's spread at full resolution, the whole of it past one and a half, light or dark alike.
pub(crate) fn mixed(f: f32, part: f32, k: &Grain) -> f32 {
    let clump = smoothstep(0.5, 1.5, f.abs() * (1.0 / mean_spread(k.per_cell)));
    k.dissolve * part * (k.haze + (1.0 - k.haze) * clump)
}

/// The grain at the pixel whose centre is `at`, in the graph's pixels, over `b`, the scene blurred under
/// a grain; a light given no share is kept.
pub fn grain(s: V4, b: V4, at: V2, k: &Grain) -> V4 {
    let y = dot(rgb(s), LUMA);
    let part = share(y, k);
    if part <= 0.0 {
        return s;
    }
    let f = field(at, k);
    let w = mixed(f, part, k);
    let base = with_alpha(map(rgb(s), |c, v| v + w * (b[c] - v)), s[3]);
    laid(base, k.stops * part * f, between(&k.colour, place(y)))
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod dissolve_tests;
