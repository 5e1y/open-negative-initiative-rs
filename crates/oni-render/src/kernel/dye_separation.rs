//! The dye separation: a scan's densities above the bare film turned into the negative's dye amounts, the
//! stock's measured dyes read through the scan's camera, on its own channels before its matrix, and solved by
//! Newton's steps from the dyes' matrix.

use super::*;
use super::stage47::Stage47;

/// Newton's steps from the matrix: three bring every dye mixture a negative holds within 1e-6 of its amounts.
pub const DYE_STEPS: usize = 3;
/// The largest move a step makes: a reading no dyes make, a camera's matrix having pushed a channel under zero,
/// then ends near the matrix's amounts instead of running away.
pub const DYE_STEP_BOUND: f32 = 0.5;

/// The nodes of each dye's tone, the density a grey holding that amount of it reads on the frame's own windows.
pub const DYE_TONE_NODES: usize = 128;

/// The least light a camera channel reads: far under any scan's guard, so a dense grey still solves.
pub const DYE_LIGHT_FLOOR: f32 = 1e-12;

const LOG2_10: f32 = std::f32::consts::LOG2_10;
const LOG10_E: f32 = std::f32::consts::LOG10_E;

/// A stock's dyes read by the scan's camera, and how far this frame unmixes them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DyeSeparation {
    /// Each channel's density of the bare film as the frame reads it, through its gains: what the dyes lie above.
    pub black: V3,
    /// The stock's own bare film as the scan reads it, in Rec. 2020 light: where the frame's lands.
    pub bare: V3,
    /// Rec. 2020 to the camera's own channels, the inverse of its matrix, by rows.
    pub to_camera: [V3; 3],
    /// The stock's bare film on the camera's channels, in density.
    pub base: V3,
    /// Camera densities above the base to dye amounts, by rows, at a unit of each dye: the first step.
    pub matrix: [V3; 3],
    /// The share of the unmix laid: none reads the scan's densities as they are, one the dye amounts.
    pub dose: f32,
    /// Per wavelength the camera reads: the three dyes' density at unit amount and the base's.
    pub dyes: &'static [V4],
    /// The camera's own weight of the light passed, per wavelength, never negative, a clear film's Y reading one.
    pub weights: &'static [V3],
    /// Each dye's amount to the density a neutral grey holding it reads on the frame's windows, from `tone_low` by
    /// `tone_step`, straight on past either end: the frame's greys print as on the scan's densities, its colours unmixed.
    pub tone: [V3; DYE_TONE_NODES],
    pub tone_low: V3,
    pub tone_step: V3,
}

impl DyeSeparation {
    /// The uniform: the frame's gains, pedestal and guard as stage 4 reads them, then the dyes.
    pub fn rows(&self, gain: V3, pedestal: f32, log_guard: f32) -> Vec<V4> {
        let m = &self.matrix;
        let c = &self.to_camera;
        let mut rows = vec![
            v3(gain, pedestal),
            v3(self.black, log_guard),
            v3(self.base, self.dose),
            v3(self.bare, 0.0),
            v3(c[0], 0.0),
            v3(c[1], 0.0),
            v3(c[2], 0.0),
            v3(m[0], 0.0),
            v3(m[1], 0.0),
            v3(m[2], 0.0),
            v3(self.tone_low, 0.0),
            v3(self.tone_step, 0.0),
        ];
        let pad = |n: usize| std::iter::repeat_n([0.0; 4], super::virtual_negative::VIRTUAL_NEGATIVE_BANDS - n);
        rows.extend(self.dyes.iter().copied().chain(pad(self.dyes.len())));
        rows.extend(self.weights.iter().map(|w| v3(*w, 0.0)).chain(pad(self.weights.len())));
        rows.extend(self.tone.iter().map(|t| v3(*t, 0.0)));
        rows
    }

    pub fn head(&self) -> [u32; 4] {
        [self.dyes.len() as u32, 0, 0, 0]
    }

    /// The tone reading each dye's amount as a density, unmoved: what a reader of the dyes alone takes.
    pub fn as_amounts(self) -> Self {
        let (low, high) = (-1.0, 5.0);
        let step = (high - low) / (DYE_TONE_NODES - 1) as f32;
        let tone = std::array::from_fn(|i| [low + i as f32 * step; 3]);
        Self { tone, tone_low: [low; 3], tone_step: [step; 3], ..self }
    }

    /// Each dye's amount through its tone, straight on past the table's ends.
    pub fn toned(&self, amounts: V3) -> V3 {
        map(amounts, |c, v| {
            let f = (v - self.tone_low[c]) / self.tone_step[c];
            let i = clamp(f.floor(), 0.0, (DYE_TONE_NODES - 2) as f32);
            let at = i as usize;
            mix(self.tone[at][c], self.tone[at + 1][c], f - i)
        })
    }

    /// The camera's density of the film holding `amounts` of each dye over its base, and its change with each.
    fn read(&self, amounts: V3) -> (V3, [V3; 3]) {
        let mut light = [0.0f32; 3];
        let mut slope = [[0.0f32; 3]; 3];
        for (dye, weight) in self.dyes.iter().zip(self.weights) {
            let t = (-LOG2_10 * (dot(amounts, rgb(*dye)) + dye[3])).exp2();
            for c in 0..3 {
                let passed = t * weight[c];
                light[c] += passed;
                slope[c] = map(slope[c], |k, v| v + passed * dye[k]);
            }
        }
        let held = map(light, |_, v| v.max(DYE_LIGHT_FLOOR));
        (map(held, |_, v| -v.ln() * LOG10_E), std::array::from_fn(|c| slope[c].map(|v| v / held[c])))
    }

    /// The dye amounts over the base for the scan densities `above` it, dosed.
    pub fn separated(&self, above: V3) -> V3 {
        let light = map(above, |c, v| self.bare[c] * (-LOG2_10 * v).exp2());
        let target = map(mul(&self.to_camera, light), |_, v| -v.max(DYE_LIGHT_FLOOR).ln() * LOG10_E);
        let mut amounts = mul(&self.matrix, map(target, |c, v| v - self.base[c]));
        for _ in 0..DYE_STEPS {
            let (density, slope) = self.read(amounts);
            let miss = map(density, |c, v| v - target[c]);
            let step = solved(&slope, miss).unwrap_or_else(|| mul(&self.matrix, miss));
            amounts = map(amounts, |c, v| v - clamp(step[c], -DYE_STEP_BOUND, DYE_STEP_BOUND));
        }
        map(above, |c, v| v + self.dose * (amounts[c] - v))
    }
}

fn mul(m: &[V3; 3], v: V3) -> V3 {
    [dot(m[0], v), dot(m[1], v), dot(m[2], v)]
}

/// `m⁻¹ v` by cofactors, `None` where `m` is singular.
fn solved(m: &[V3; 3], v: V3) -> Option<V3> {
    let [[a, b, c], [d, e, f], [g, h, i]] = *m;
    let (ca, cb, cc) = (e * i - f * h, f * g - d * i, d * h - e * g);
    let det = a * ca + b * cb + c * cc;
    // A NaN determinant fails the comparison too, as the shader's does.
    if det.abs().partial_cmp(&1e-12) != Some(std::cmp::Ordering::Greater) {
        return None;
    }
    let inverse = [[ca, c * h - b * i, b * f - c * e], [cb, a * i - c * g, c * d - a * f], [cc, b * g - a * h, a * e - b * d]];
    Some(map(mul(&inverse, v), |_, x| x / det))
}

/// One scan pixel through the frame's gains and pedestal to its dye amounts, then their tones, as light: stage 4
/// reads it at unit gain and no pedestal. Alpha kept.
pub fn dye_separation(s: V4, k: &Stage47, d: &DyeSeparation) -> V4 {
    let t = map(rgb(s), |c, v| (v + k.pedestal) * k.gain[c]);
    let density = map(t, |_, v| -v.max(k.log_guard).ln() * LOG10_E);
    let amounts = d.separated(map(density, |c, v| v - d.black[c]));
    with_alpha(map(d.toned(amounts), |_, v| (-LOG2_10 * v).exp2()), s[3])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three narrow dyes, each alone in a channel but for a share of crosstalk, read on three wavelengths.
    fn narrow() -> DyeSeparation {
        static DYES: [V4; 3] = [[1.0, 0.3, 0.05, 0.1], [0.2, 1.0, 0.1, 0.2], [0.02, 0.15, 1.0, 0.4]];
        static WEIGHTS: [V3; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let base = map([0.1, 0.2, 0.4], |_, v| v);
        // Each wavelength alone in its channel: the densities are linear in the dyes and the matrix their inverse.
        let a = [[1.0, 0.2, 0.02], [0.3, 1.0, 0.15], [0.05, 0.1, 1.0]];
        let inverse = solved(&a, [1.0, 0.0, 0.0]).zip(solved(&a, [0.0, 1.0, 0.0])).zip(solved(&a, [0.0, 0.0, 1.0])).unwrap();
        let ((x, y), z) = inverse;
        let matrix = [[x[0], y[0], z[0]], [x[1], y[1], z[1]], [x[2], y[2], z[2]]];
        let bare = base.map(|v| (-LOG2_10 * v).exp2());
        let to_camera = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let tone = [[0.0; 3]; DYE_TONE_NODES];
        DyeSeparation { black: [0.0; 3], bare, to_camera, base, matrix, dose: 1.0, dyes: &DYES, weights: &WEIGHTS, tone, tone_low: [0.0; 3], tone_step: [1.0; 3] }
            .as_amounts()
    }

    #[test]
    fn a_narrow_camera_reads_each_dye_through_its_inverse_and_the_dose_mixes_toward_the_scan() {
        let d = narrow();
        let amounts = [0.8, 1.3, 0.4];
        let (density, _) = d.read(amounts);
        let above = map(density, |c, v| v - d.base[c]);
        let back = d.separated(above);
        assert!((0..3).all(|c| (back[c] - amounts[c]).abs() < 1e-5), "{back:?}");
        let half = DyeSeparation { dose: 0.5, ..d }.separated(above);
        assert!((0..3).all(|c| (half[c] - 0.5 * (above[c] + amounts[c])).abs() < 1e-5), "{half:?}");
        // Twin: the scan's own densities are not the dye amounts, crosstalk and all.
        assert!((0..3).any(|c| (above[c] - amounts[c]).abs() > 0.05), "{above:?}");
        assert_eq!(DyeSeparation { dose: 0.0, ..d }.separated(above), above);
    }

    #[test]
    fn a_tone_reads_between_its_nodes_and_straight_on_past_its_ends() {
        let d = narrow();
        let mut tone = [[0.0; 3]; DYE_TONE_NODES];
        for (i, t) in tone.iter_mut().enumerate() {
            *t = [i as f32 * i as f32, 2.0 * i as f32, 1.0];
        }
        let shaped = DyeSeparation { tone, tone_low: [0.0; 3], tone_step: [0.5; 3], ..d };
        assert_eq!(shaped.toned([0.75, 0.75, 0.75]), [2.5, 3.0, 1.0], "halfway between the second and third nodes");
        let past = (DYE_TONE_NODES - 1) as f32 * 0.5 + 0.5;
        let last = (DYE_TONE_NODES - 1) as f32;
        assert_eq!(shaped.toned([-0.5, past, past])[..2], [-1.0, 2.0 * last + 2.0], "straight on past either end");
        // Twin: the plain tone reads each amount as it is.
        let plain = d.toned([0.75, 1.5, -0.25]);
        assert!((0..3).all(|c| (plain[c] - [0.75, 1.5, -0.25][c]).abs() < 1e-6), "{plain:?}");
    }

    #[test]
    fn the_bare_film_holds_no_dye_and_a_singular_slope_falls_back_on_the_matrix() {
        let d = narrow();
        // Zero to the round trip of the bare film's light through its density, a float's.
        assert!(d.separated([0.0; 3]).iter().all(|v| v.abs() < 1e-6), "{:?}", d.separated([0.0; 3]));
        assert!(d.separated([0.1, 0.0, 0.0])[0] > 0.05, "twin: a density above the bare film holds a dye");
        assert_eq!(solved(&[[1.0, 2.0, 3.0], [2.0, 4.0, 6.0], [0.0, 0.0, 1.0]], [1.0, 1.0, 1.0]), None);
        // Twin: an invertible matrix solves.
        assert_eq!(solved(&[[2.0, 0.0, 0.0], [0.0, 4.0, 0.0], [0.0, 0.0, 1.0]], [1.0, 1.0, 1.0]), Some([0.5, 0.25, 1.0]));
    }
}
