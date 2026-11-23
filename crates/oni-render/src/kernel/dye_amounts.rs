//! A scanned slide read as its film's dyes: each pixel's light over the backlight and the bare base solved for the three
//! dyes' amounts whose light, through the film's base and dyes band by band under the scanner's lamp, is that light.

use super::*;

/// Newton's steps from the first guess: on the owner's slides eight bring 99.9 % of the pixels within 4e-4 log of the
/// projected light the root gives.
pub const DYE_STEPS: usize = 8;

/// The most one step moves an amount: a dense pixel's Jacobian is steep, and a full step there overshoots.
pub const DYE_STEP_LIMIT: f32 = 0.5;

/// The amounts a solve keeps to: a hair under the bare film for a scan's noise, past any film's D-max for its black.
pub const DYE_LOW: f32 = -0.5;
pub const DYE_HIGH: f32 = 6.0;

/// The least light a solve reads: under it a sensor's black or noise would read as an endless density.
pub const DYE_FLOOR: f32 = 1e-6;

/// The film's dyes and the scanner's lamp as the camera sees it, beside the input as two rows of `bands`: each wavelength's
/// cyan, magenta and yellow at unit amount and the base's density, then the lamp's light in working Rec. 2020, the bare
/// film's light adding to (1, 1, 1).
#[derive(Clone, Debug, PartialEq)]
pub struct DyeAmounts {
    /// Each channel's scan to the model's light: the frame's gains, which put the backlight on 1, times the lamp's light
    /// through no film.
    pub gain: V3,
    /// The first guess, by rows: the amounts are this times each channel's density over the bare film.
    pub start: [V3; 3],
    pub bands: usize,
}

impl DyeAmounts {
    pub fn head(&self) -> [u32; 4] {
        [self.bands as u32, DYE_STEPS as u32, 0, 0]
    }

    pub fn rows(&self) -> Vec<V4> {
        let s = &self.start;
        vec![v3(self.gain, 0.0), v3(s[0], 0.0), v3(s[1], 0.0), v3(s[2], 0.0)]
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

/// The dyes' amounts whose light under the lamp is `light`, the model's units, by Newton's steps on its log.
pub fn amounts(light: V3, k: &DyeAmounts, table: &[V4]) -> V3 {
    let light = light.map(|v| v.max(DYE_FLOOR));
    let target = light.map(f32::log10);
    let mut a = map([0, 1, 2].map(|r| -dot(k.start[r], target)), |_, v| clamp(v, DYE_LOW, DYE_HIGH));
    for _ in 0..DYE_STEPS {
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
        let jacobian: [V3; 3] = std::array::from_fn(|c| j[c].map(|v| v / f[c]));
        let miss = map(f, |c, v| v.log10() - target[c]);
        let Some(step) = solved(&jacobian, miss) else { break };
        a = map(a, |c, v| clamp(v - clamp(step[c], -DYE_STEP_LIMIT, DYE_STEP_LIMIT), DYE_LOW, DYE_HIGH));
    }
    a
}

/// One scanned pixel to its dyes' amounts, each coded on its own channel as `10^-amount`, as a slide's projection reads
/// them; alpha kept.
pub fn dye_amounts(s: V4, k: &DyeAmounts, table: &[V4]) -> V4 {
    let a = amounts(map(rgb(s), |c, v| v * k.gain[c]), k, table);
    with_alpha(a.map(|v| 10f32.powf(-v)), s[3])
}
