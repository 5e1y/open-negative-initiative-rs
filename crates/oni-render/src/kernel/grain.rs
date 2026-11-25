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
}

impl Grain {
    /// Grains of `radius` full-resolution pixels on a render whose pixel spans `footprint` of them, each layer's grain
    /// `deviation`: the clouds widened by the footprint; past the lattice's bound no wider, the deviation scaled as their mean.
    pub fn on(scan: [[f32; 3]; 2], radius: f32, footprint: f32, deviation: [f32; 3], gaze: [V3; 3]) -> Self {
        let wanted = (GRAIN_CLOUD * radius).hypot(0.5 * footprint);
        let bound = ((GRAIN_CELLS - 1) as f32 * 0.5 * GRAIN_STEP - SCATTER_BOUND) * radius / GRAIN_PROFILE;
        let cloud = wanted.min(bound);
        Grain {
            scan,
            render: inverse(scan),
            radius,
            cloud,
            deviation: deviation.map(|d| d * cloud / wanted),
            gaze,
            silver: false,
            negative: None,
            seed: [0, 0],
        }
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

    pub fn rows(&self) -> Vec<V4> {
        let [x, y] = self.scan;
        let [rx, ry] = self.render;
        let (bare, window) = match self.negative {
            Some(n) => (v3(n.bare, n.veil), v3(n.window, 1.0)),
            None => ([1.0, 1.0, 1.0, 0.0], [1.0, 1.0, 1.0, 0.0]),
        };
        let mut rows = vec![v3(x, self.radius), v3(y, self.cloud), v3(rx, self.reach()), v3(ry, 0.0), v3(self.deviation, 0.0), bare, window];
        rows.extend(self.gaze.map(|g| v3(g, 0.0)));
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

/// One pixel of `input`, at `at` in the graph's pixels, under the grain `k`; alpha kept.
pub fn grain(input: &Grid, at: V2, k: &Grain) -> V4 {
    let p = [at[0].floor() as i32, at[1].floor() as i32];
    let own = input.load(p);
    let [x, y] = k.scan;
    let q = [x[0] * at[0] + x[1] * at[1] + x[2], y[0] * at[0] + y[1] * at[1] + y[2]];
    let step = GRAIN_STEP * k.radius;
    let reach = k.reach();
    let lo = q.map(|v| ((v - reach) / step).floor() as i32);
    let count = std::array::from_fn::<i32, 2, _>(|i| (((q[i] + reach) / step).floor() as i32 - lo[i] + 1).min(GRAIN_CELLS));
    let fields = k.head()[2];
    let mut sample = ([0.0f32; 3], 0.0f32);
    let mut noise = [(0.0f32, 0.0f32); 3];
    for field in 0..fields {
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
                if w <= 0.0 {
                    continue;
                }
                if field == 0 {
                    let [rx, ry] = k.render;
                    let r = [rx[0] * centre[0] + rx[1] * centre[1] + rx[2], ry[0] * centre[0] + ry[1] * centre[1] + ry[2]];
                    let v = shared(input.load([r[0].floor() as i32, r[1].floor() as i32]), k);
                    sample = (std::array::from_fn(|c| sample.0[c] + w * v[c]), sample.1 + w);
                } else {
                    let layer = (field - 1) as usize;
                    let v = normal(a[1]) * k.deviation[layer];
                    noise[layer] = (noise[layer].0 + w * v, noise[layer].1 + w);
                }
            }
        }
    }
    let base = if sample.1 > GRAIN_COVERED { sample.0.map(|v| v / sample.1) } else { shared(own, k) };
    let mean = |(v, w): (f32, f32)| if w > GRAIN_COVERED { v / w } else { 0.0 };
    let delta = if k.silver { [mean(noise[0]); 3] } else { noise.map(mean) };
    let laid = if k.silver { delta } else { k.gaze.map(|g| dot(g, delta)) };
    with_alpha(coded(map(base, |c, v| v + laid[c]), k), own[3])
}

#[cfg(test)]
mod tests;
