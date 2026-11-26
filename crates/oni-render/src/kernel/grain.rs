//! Film grain made of grains: on a jittered lattice anchored on the scan's full-resolution grid, each grain the soft union
//! of a few small clouds. A pixel reads the film where the grains over it are centred, so detail finer than a grain goes;
//! each layer has its own grains, each grain a deviation of its window's share, the layers' mixed by the reader's gaze.

use super::*;

/// The lattice's pitch, in grain radii.
pub const GRAIN_STEP: f32 = 0.7;
/// A cloud's radius, in grain radii: a grain is the union of four.
pub const GRAIN_CLOUD: f32 = 0.62;
/// The deviation of a cloud's place about its grain's centre, in grain radii; its draw is bounded at 2.45 of it.
pub const GRAIN_SCATTER: f32 = 0.32;
/// A cloud's profile reaches this many of its radii.
pub const GRAIN_PROFILE: f32 = 1.3;
/// The weight under which a point counts as covered by no grain.
pub const GRAIN_COVERED: f32 = 0.05;
/// The most lattice cells read along each axis, which bounds a pixel's work at any scale.
pub const GRAIN_CELLS: i32 = 8;

/// The farthest a cloud's centre lies from its grain's, in grain radii: the triangular draw's bound.
const SCATTER_BOUND: f32 = GRAIN_SCATTER * 2.449_489_8;

/// A negative under its grain: each layer's light on the bare film, its reading's pedestal `veil` added, and its window in
/// density, so a light reads as its share of its own window above the bare film.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrainedNegative {
    pub bare: [f32; 3],
    pub veil: f32,
    pub window: [f32; 3],
}

/// A grain laid on a render.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grain {
    /// An output pixel's centre to its point on the scan's full-resolution grid, in rows.
    pub scan: [[f32; 3]; 2],
    /// A point of that grid back to this render's pixels.
    pub render: [[f32; 3]; 2],
    /// A grain's radius and its clouds', in full-resolution pixels: the clouds widened by the render's footprint.
    pub radius: f32,
    pub cloud: f32,
    /// Each layer's grain deviation, in shares of its window.
    pub deviation: [f32; 3],
    /// Each channel's mix of the layers' deviations, rows summing to one.
    pub gaze: [V3; 3],
    /// One layer on the three channels: a silver film's.
    pub silver: bool,
    /// The negative it lies on; none on a positive, whose share is the screen's code of its light.
    pub negative: Option<GrainedNegative>,
    /// The photo's own arrangement, added to every lattice index.
    pub seed: [u32; 2],
    /// The scan's grid onto the level of the layers' field a render reads (twice), then the box it reads there: its half
    /// width in texels, zero for a bilinear read, and the level's spread, the deviation its clouds keep against the
    /// clouds it wanted.
    pub plate: [[[f32; 3]; 2]; 2],
    pub blend: [f32; 3],
    /// This render's own spread: `deviation` is the wanted deviation times it.
    pub spread: f32,
    /// A render coarser than the field's finest level, which reads the mean of the texels under its pixel's box.
    pub reduced: bool,
}

impl Grain {
    /// Grains of `radius` full-resolution pixels on a render whose pixel spans `footprint` of them, each layer's grain
    /// `deviation`: the clouds widened by the footprint; past the lattice's bound no wider, the deviation scaled as their mean.
    pub fn on(scan: [[f32; 3]; 2], radius: f32, footprint: f32, deviation: [f32; 3], gaze: [V3; 3]) -> Self {
        let wanted = (GRAIN_CLOUD * radius).hypot(0.5 * footprint);
        let bound = ((GRAIN_CELLS - 1) as f32 * 0.5 * GRAIN_STEP - SCATTER_BOUND) * radius / GRAIN_PROFILE;
        let cloud = wanted.min(bound);
        let spread = cloud / wanted;
        let render = inverse(scan);
        Grain {
            scan,
            render,
            radius,
            cloud,
            deviation: deviation.map(|d| d * spread),
            gaze,
            silver: false,
            negative: None,
            seed: [0, 0],
            plate: [render, render],
            blend: [0.0, spread, spread],
            spread,
            reduced: false,
        }
    }

    /// A level of the layers' field on the scan's grid, one texel `pitch` full-resolution pixels a side, its clouds
    /// widened by that pitch: drawn once a photo and grain size, read at every scale.
    pub fn level(radius: f32, pitch: f32) -> Self {
        Grain::on([[pitch, 0.0, 0.0], [0.0, pitch, 0.0]], radius, pitch, [1.0; 3], [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]])
    }

    /// This grain reading its layers' field off two levels of pitches `pitch` and spreads `spread`, the second's share
    /// `share`.
    pub fn plated(self, pitch: f32, gain: f32, half: f32) -> Self {
        let onto = [[1.0 / pitch, 0.0, 0.0], [0.0, 1.0 / pitch, 0.0]];
        Grain { plate: [onto, onto], blend: [half, gain, gain], reduced: half > 0.0, ..self }
    }

    /// How many full-resolution pixels one of this render's spans.
    pub fn footprint(&self) -> f32 {
        let [x, y] = self.scan;
        (x[0] * y[1] - x[1] * y[0]).abs().sqrt()
    }

    /// This grain on `negative`.
    pub fn on_negative(self, negative: GrainedNegative) -> Self {
        Grain { negative: Some(negative), ..self }
    }

    /// This grain on one layer, a silver film's.
    pub fn on_silver(self) -> Self {
        Grain { silver: true, ..self }
    }

    /// This grain on the photo of `seed`.
    pub fn seeded(self, seed: [u32; 2]) -> Self {
        Grain { seed, ..self }
    }

    /// How far around a pixel a grain covering it is centred, in full-resolution pixels.
    pub fn reach(&self) -> f32 {
        SCATTER_BOUND * self.radius + GRAIN_PROFILE * self.cloud
    }

    /// The uniform's head: the seed and the fields laid, the sampling one and a field a layer.
    pub fn head(&self) -> [u32; 4] {
        [self.seed[0], self.seed[1], if self.silver { 2 } else { 4 }, 0]
    }

    /// `rows` with what the field reads alone: no deviation, negative or gaze, so a notch on them keeps it.
    pub fn field_rows(&self) -> Vec<V4> {
        let mut rows = Grain { deviation: [0.0; 3], gaze: [[0.0; 3]; 3], negative: None, ..*self }.rows();
        rows[5] = [0.0; 4];
        rows[6] = [0.0; 4];
        for row in &mut rows[10..] {
            *row = [0.0; 4];
        }
        rows
    }

    pub fn rows(&self) -> Vec<V4> {
        let [x, y] = self.scan;
        let [rx, ry] = self.render;
        let (bare, window) = match self.negative {
            Some(n) => (v3(n.bare, n.veil), v3(n.window, 1.0)),
            None => ([1.0, 1.0, 1.0, 0.0], [1.0, 1.0, 1.0, 0.0]),
        };
        let wanted = self.deviation.map(|d| d / self.spread);
        let mut rows = vec![v3(x, self.radius), v3(y, self.cloud), v3(rx, self.reach()), v3(ry, 0.0), v3(wanted, 0.0), bare, window];
        rows.extend(self.gaze.map(|g| v3(g, 0.0)));
        rows.extend(self.plate.iter().flatten().map(|r| v3(*r, 0.0)));
        rows.push(v3(self.blend, f32::from(u8::from(self.reduced))));
        rows
    }
}

/// The inverse of an affine map's two rows.
fn inverse(m: [[f32; 3]; 2]) -> [[f32; 3]; 2] {
    let [[a, b, c], [d, e, f]] = m.map(|r| r.map(f64::from));
    let det = a * e - b * d;
    let rows = [[e / det, -b / det, (b * f - c * e) / det], [-d / det, a / det, (c * d - a * f) / det]];
    rows.map(|r| r.map(|v| v as f32))
}

/// PCG-4D on a lattice cell, its field and the photo's seed: four words, exact on every device.
fn hash(v: [u32; 4]) -> [u32; 4] {
    let mut v = v.map(|x| x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223));
    let mix = |v: &mut [u32; 4]| {
        v[0] = v[0].wrapping_add(v[1].wrapping_mul(v[3]));
        v[1] = v[1].wrapping_add(v[2].wrapping_mul(v[0]));
        v[2] = v[2].wrapping_add(v[0].wrapping_mul(v[1]));
        v[3] = v[3].wrapping_add(v[1].wrapping_mul(v[2]));
    };
    mix(&mut v);
    v = v.map(|x| x ^ (x >> 16));
    mix(&mut v);
    v
}

/// A word's byte `i` as a uniform on 0…1.
fn byte(w: u32, i: u32) -> f32 {
    (((w >> (8 * i)) & 0xff) as f32 + 0.5) / 256.0
}

/// A word's half `i` as a uniform on 0…1.
fn half(w: u32, i: u32) -> f32 {
    (((w >> (16 * i)) & 0xffff) as f32 + 0.5) / 65536.0
}

/// A standard normal from a word's four bytes, their sum's.
fn normal(w: u32) -> f32 {
    (byte(w, 0) + byte(w, 1) + byte(w, 2) + byte(w, 3) - 2.0) * 1.732_050_8
}

/// A cloud's offset from its grain's centre, in grain radii: on each axis a triangular draw of the scatter's deviation.
fn offset(w: u32) -> V2 {
    let tri = |i: u32| (byte(w, i) + byte(w, i + 1) - 1.0) * GRAIN_SCATTER / 0.408_248_3;
    [tri(0), tri(2)]
}

/// A cloud's weight at `d` full-resolution pixels from its centre, of radius `rc`.
fn profile(d: f32, rc: f32) -> f32 {
    let w = clamp((GRAIN_PROFILE - d / rc) / 0.6, 0.0, 1.0);
    w * w
}

/// A light's share of each window, or its screen code on a positive.
fn shared(s: V4, k: &Grain) -> V3 {
    match k.negative {
        Some(n) => map(rgb(s), |c, v| -((v + n.veil) / n.bare[c]).max(1e-30).log10() / n.window[c]),
        None => map(rgb(s), |_, v| super::neutral_negative::screen_code(v)),
    }
}

/// The light holding shares `s`.
fn coded(s: V3, k: &Grain) -> V3 {
    match k.negative {
        Some(n) => map(s, |c, v| n.bare[c] * (-std::f32::consts::LOG2_10 * n.window[c] * v).exp2() - n.veil),
        None => map(s, |_, v| screen_light(v)),
    }
}

/// The light whose screen code is `code`: sRGB's, carried straight on under zero and by its own power past 1.
fn screen_light(code: f32) -> f32 {
    if code <= 0.040_45 { code / 12.92 } else { ((code + 0.055) / 1.055).powf(2.4) }
}

/// The lattice cells whose grains may cover `at`, in the graph's pixels: where `at` lies on the scan's grid, the first
/// cell and how many along each axis.
fn cells(at: V2, k: &Grain) -> (V2, [i32; 2], [i32; 2]) {
    let [x, y] = k.scan;
    let q = [x[0] * at[0] + x[1] * at[1] + x[2], y[0] * at[0] + y[1] * at[1] + y[2]];
    let step = GRAIN_STEP * k.radius;
    let reach = k.reach();
    let lo = q.map(|v| ((v - reach) / step).floor() as i32);
    let count = std::array::from_fn::<i32, 2, _>(|i| (((q[i] + reach) / step).floor() as i32 - lo[i] + 1).min(GRAIN_CELLS));
    (q, lo, count)
}

/// Field `field`'s grains over `q` on the scan's grid: each covering grain's centre and its weight there.
fn covering(q: V2, lo: [i32; 2], count: [i32; 2], field: u32, k: &Grain, mut each: impl FnMut([u32; 4], V2, f32)) {
    let step = GRAIN_STEP * k.radius;
    let reach = k.reach();
    for j in 0..count[1] {
        for i in 0..count[0] {
            let cell = [lo[0] + i, lo[1] + j];
            let a = hash([(cell[0] as u32).wrapping_add(k.seed[0]), (cell[1] as u32).wrapping_add(k.seed[1]), field, 0]);
            let centre = [(cell[0] as f32 + half(a[0], 0)) * step, (cell[1] as f32 + half(a[0], 1)) * step];
            let to = [q[0] - centre[0], q[1] - centre[1]];
            if to[0].hypot(to[1]) > reach {
                continue;
            }
            let b = hash(a);
            let w: f32 = [a[2], a[3], b[0], b[1]]
                .iter()
                .map(|&c| {
                    let o = offset(c);
                    profile((to[0] - o[0] * k.radius).hypot(to[1] - o[1] * k.radius), k.cloud)
                })
                .sum();
            if w > 0.0 {
                each(a, centre, w);
            }
        }
    }
}

/// Each layer's grains at `at`, in the graph's pixels: their draws' mean through the grains over it, in standard
/// normals, zero where no grain covers it enough; the film beneath, the deviations and the gaze play no part, so the
/// field holds while a slider moves the film or the grain's Amount.
pub fn grain_field(at: V2, k: &Grain) -> V3 {
    let (q, lo, count) = cells(at, k);
    let layers = if k.silver { 1 } else { 3 };
    let mut out = [0.0; 3];
    for (layer, value) in out.iter_mut().enumerate().take(layers) {
        let mut noise = (0.0f32, 0.0f32);
        covering(q, lo, count, layer as u32 + 1, k, |a, _, w| noise = (noise.0 + w * normal(a[1]), noise.1 + w));
        *value = if noise.1 > GRAIN_COVERED { noise.0 / noise.1 } else { 0.0 };
    }
    out
}

/// One pixel of `input`, at `at` in the graph's pixels, under the grain `k` whose layers' field there is `field`;
/// alpha kept.
pub fn grain_on(input: &Grid, at: V2, k: &Grain, field: V3) -> V4 {
    let p = [at[0].floor() as i32, at[1].floor() as i32];
    let own = input.load(p);
    let (q, lo, count) = cells(at, k);
    let mut sample = ([0.0f32; 3], 0.0f32);
    covering(q, lo, count, 0, k, |_, centre, w| {
        let [rx, ry] = k.render;
        let r = [rx[0] * centre[0] + rx[1] * centre[1] + rx[2], ry[0] * centre[0] + ry[1] * centre[1] + ry[2]];
        let v = shared(input.load([r[0].floor() as i32, r[1].floor() as i32]), k);
        sample = (std::array::from_fn(|c| sample.0[c] + w * v[c]), sample.1 + w);
    });
    let base = if sample.1 > GRAIN_COVERED { sample.0.map(|v| v / sample.1) } else { shared(own, k) };
    let laid = if k.silver { [k.deviation[0] * field[0]; 3] } else { k.gaze.map(|g| dot(g, map(field, |c, v| v * k.deviation[c]))) };
    with_alpha(coded(map(base, |c, v| v + laid[c]), k), own[3])
}

/// One pixel of `input`, at `at` in the graph's pixels, under the grain `k`; alpha kept.
pub fn grain(input: &Grid, at: V2, k: &Grain) -> V4 {
    grain_on(input, at, k, grain_field(at, k))
}

/// A level of the layers' field where `map` takes `q`, a point of the scan's grid: the mean of its texels over a box
/// `half` texels each side of it, each weighted by how much of it the box covers, or bilinearly where `half` is zero.
pub fn plate_read(level: &Grid, map: [[f32; 3]; 2], q: V2, half: f32) -> V3 {
    let t = [map[0][0] * q[0] + map[0][1] * q[1] + map[0][2] - 0.5, map[1][0] * q[0] + map[1][1] * q[1] + map[1][2] - 0.5];
    if half == 0.0 {
        let f = t.map(f32::floor);
        let w = [t[0] - f[0], t[1] - f[1]];
        let (i, j) = (f[0] as i32, f[1] as i32);
        let mix = |a: V4, b: V4, s: f32| std::array::from_fn::<f32, 3, _>(|c| a[c] + (b[c] - a[c]) * s);
        let top = mix(level.load([i, j]), level.load([i + 1, j]), w[0]);
        let bottom = mix(level.load([i, j + 1]), level.load([i + 1, j + 1]), w[0]);
        return std::array::from_fn(|c| top[c] + (bottom[c] - top[c]) * w[1]);
    }
    let span = |c: f32| {
        let (lo, hi) = (c - half, c + half);
        let first = (lo + 0.5).floor() as i32;
        std::array::from_fn::<(i32, f32), 5, _>(|n| {
            let i = first + n as i32;
            (i, ((hi.min(i as f32 + 0.5) - lo.max(i as f32 - 0.5)).max(0.0)) / (2.0 * half))
        })
    };
    let (xs, ys) = (span(t[0]), span(t[1]));
    let mut out = [0.0f32; 3];
    for (j, wy) in ys {
        for (i, wx) in xs {
            let v = level.load([i, j]);
            for c in 0..3 {
                out[c] += wx * wy * v[c];
            }
        }
    }
    out
}

/// The mean of each 2 × 2 texels of `level`: the next level of a plate.
pub fn grain_halved(level: &Grid) -> Grid {
    let (w, h) = (level.width.div_ceil(2), level.height.div_ceil(2));
    Grid::generated(0, 0, w, h, |p| {
        let q = [2 * p[0] + level.x, 2 * p[1] + level.y];
        let s = [level.load(q), level.load([q[0] + 1, q[1]]), level.load([q[0], q[1] + 1]), level.load([q[0] + 1, q[1] + 1])];
        std::array::from_fn(|c| 0.25 * (s[0][c] + s[1][c] + s[2][c] + s[3][c]))
    })
}

/// One pixel of `input` at `at` under the grain `k`, its layers' field read off `level` through `k.plate` and `k.blend`;
/// alpha kept.
pub fn grain_plated(input: &Grid, at: V2, k: &Grain, level: &Grid) -> V4 {
    let (q, _, _) = cells(at, k);
    let read = plate_read(level, k.plate[0], q, k.blend[0]);
    let field = read.map(|v| v * k.blend[1]);
    grain_on(input, at, &Grain { deviation: k.deviation.map(|d| d / k.spread), ..*k }, field)
}

#[cfg(test)]
mod tests;
