//! A scanned slide read as its film's dyes: each pixel's light over the backlight and the bare base solved for the three
//! dyes' amounts whose light, through the film's base and dyes band by band under the scanner's lamp, is that light.

use super::*;

/// The model's evaluations after the first guess: on the owner's slide P1073157 eight lay 99.9 % of the pixels within
/// 0.02 ΔE of the screen sixty-four lay, the worst 1.2.
pub const DYE_STEPS: usize = 8;

/// The first step's damping, a share of the curvature: Levenberg–Marquardt's, tenfold on a step refused, a tenth on one
/// kept, so a light past the dyes' reach settles on the amounts of the nearest light they reach.
pub const DYE_DAMPING: f32 = 1e-3;

/// The amounts a solve keeps to: a hair under the bare film for a scan's noise, past any film's D-max for its black.
pub const DYE_LOW: f32 = -0.5;
pub const DYE_HIGH: f32 = 6.0;

/// The least light a solve reads: under it a sensor's black or noise would read as an endless density.
pub const DYE_FLOOR: f32 = 1e-6;

/// The film's dyes and the scanner's lamp, beside the input as two rows of `bands`: each wavelength's cyan, magenta and
/// yellow at unit amount and the base's density, then the lamp's light as the observer sees it, the bare film's (1, 1, 1).
#[derive(Clone, Debug, PartialEq)]
pub struct DyeAmounts {
    /// Each channel's scan to the model's light: the frame's gains, which put the backlight on 1, times the lamp's light
    /// through no film; the bare film reads (1, 1, 1) in working Rec. 2020.
    pub gain: V3,
    /// The model's light in working Rec. 2020 to the observer's, by rows: the observer sees no band as negative, so the
    /// model's light stays positive and falls as any dye grows, where a dense blue's red in Rec. 2020 reads zero or less.
    pub to_seen: [V3; 3],
    /// The first guess, by rows: the amounts are this times each of the observer's densities over the bare film.
    pub start: [V3; 3],
    pub bands: usize,
}

impl DyeAmounts {
    pub fn head(&self) -> [u32; 4] {
        [self.bands as u32, DYE_STEPS as u32, 0, 0]
    }

    pub fn rows(&self) -> Vec<V4> {
        let (t, s) = (&self.to_seen, &self.start);
        vec![v3(self.gain, DYE_DAMPING), v3(t[0], 0.0), v3(t[1], 0.0), v3(t[2], 0.0), v3(s[0], 0.0), v3(s[1], 0.0), v3(s[2], 0.0)]
    }
}

/// `m⁻¹ v` by cofactors; none where `m` is singular, as a pixel whose light no dye moves reads no step.
fn solved(m: &[V3; 3], v: V3) -> Option<V3> {
    let [[a, b, c], [d, e, f], [g, h, i]] = *m;
    let (ca, cb, cc) = (e * i - f * h, f * g - d * i, d * h - e * g);
    let det = a * ca + b * cb + c * cc;
    if det.abs() < 1e-30 {
        return None;
    }
    let inverse = [[ca, c * h - b * i, b * f - c * e], [cb, a * i - c * g, c * d - a * f], [cc, b * g - a * h, a * e - b * d]];
    Some(map([dot(inverse[0], v), dot(inverse[1], v), dot(inverse[2], v)], |_, x| x / det))
}

/// The log miss of amounts `a`'s light against `target`, and its Jacobian by rows.
fn missed(a: V3, target: V3, k: &DyeAmounts, table: &[V4]) -> (V3, [V3; 3]) {
    let mut f = [0.0f32; 3];
    let mut j = [[0.0f32; 3]; 3];
    for i in 0..k.bands {
        let (dye, seen) = (table[i], table[k.bands + i]);
        let t = 10f32.powf(-(dye[3] + a[0] * dye[0] + a[1] * dye[1] + a[2] * dye[2]));
        for c in 0..3 {
            f[c] += t * seen[c];
            for m in 0..3 {
                j[c][m] -= t * seen[c] * dye[m];
            }
        }
    }
    let f = f.map(|v| v.max(1e-12));
    (map(f, |c, v| v.log10() - target[c]), std::array::from_fn(|c| j[c].map(|v| v / f[c])))
}

/// The squared miss's gradient, halved: the Jacobian's transpose on the miss.
fn sloped(jacobian: &[V3; 3], miss: V3) -> V3 {
    std::array::from_fn(|r| (0..3).map(|m| jacobian[m][r] * miss[m]).sum())
}

/// The dyes' amounts whose light under the lamp is `light`, the model's units in working Rec. 2020, by damped
/// Gauss–Newton steps on its log; a light past the dyes' reach takes the amounts of the nearest light they reach.
pub fn amounts(light: V3, k: &DyeAmounts, table: &[V4]) -> V3 {
    let seen = map([0, 1, 2].map(|r| dot(k.to_seen[r], light)), |_, v| v.max(DYE_FLOOR));
    let target = seen.map(f32::log10);
    let mut a = map([0, 1, 2].map(|r| -dot(k.start[r], target)), |_, v| clamp(v, DYE_LOW, DYE_HIGH));
    let (mut miss, mut jacobian) = missed(a, target, k, table);
    let mut damping = DYE_DAMPING;
    for _ in 0..DYE_STEPS {
        // An amount on a bound the descent pushes past is held there, the others solved without it.
        let slope = sloped(&jacobian, miss);
        let free: [bool; 3] = std::array::from_fn(|c| !((a[c] <= DYE_LOW && slope[c] > 0.0) || (a[c] >= DYE_HIGH && slope[c] < 0.0)));
        let normal: [V3; 3] = std::array::from_fn(|r| {
            std::array::from_fn(|c| match (free[r] && free[c], r == c) {
                (true, diagonal) => (0..3).map(|m| jacobian[m][r] * jacobian[m][c]).sum::<f32>() * if diagonal { 1.0 + damping } else { 1.0 },
                (false, diagonal) => f32::from(u8::from(diagonal)),
            })
        });
        let gradient = map(slope, |c, v| if free[c] { v } else { 0.0 });
        let Some(step) = solved(&normal, gradient) else { break };
        let next = map(a, |c, v| clamp(v - step[c], DYE_LOW, DYE_HIGH));
        let (next_miss, next_jacobian) = missed(next, target, k, table);
        // Kept where it misses less, or where its slopes at both ends say so: near the root the misses differ under f32's
        // grain, while the slopes' trapezoid still reads the fall.
        let ends = sloped(&next_jacobian, next_miss);
        let falling = dot(map(slope, |c, v| v + ends[c]), map(a, |c, v| v - next[c])) > 0.0;
        if dot(next_miss, next_miss) < dot(miss, miss) || falling {
            (a, miss, jacobian, damping) = (next, next_miss, next_jacobian, damping * 0.1);
        } else {
            damping *= 10.0;
        }
    }
    a
}

/// One scanned pixel to its dyes' amounts, each coded on its own channel as `10^-amount`, as a slide's projection reads
/// them; alpha kept.
pub fn dye_amounts(s: V4, k: &DyeAmounts, table: &[V4]) -> V4 {
    let a = amounts(map(rgb(s), |c, v| v * k.gain[c]), k, table);
    with_alpha(a.map(|v| 10f32.powf(-v)), s[3])
}
