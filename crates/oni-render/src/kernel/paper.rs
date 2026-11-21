//! The paper: an enlarger's light through the negative's dyes exposes a colour paper's three layers by their
//! spectral sensitivities, the paper develops on its characteristic curves, and its dyes are read under D50.

use super::*;

/// Most wavelengths either sum runs over: every sample of 380 to 780 nm by 5.
pub const PAPER_BANDS: usize = 81;

/// The nodes of each layer's characteristic curve, on a uniform log exposure axis.
pub const PAPER_CURVE_NODES: usize = 256;

/// The paper's raw exposure floor in its log, as its curves read a layer no light reached.
pub const PAPER_EXPOSURE_FLOOR: f32 = 1e-10;

const LOG2_10: f32 = std::f32::consts::LOG2_10;
const LOG10_E: f32 = std::f32::consts::LOG10_E;

/// A negative printed on a paper: how its pixel is read as dye amounts, the enlarger's light, the paper's
/// curves and its print seen under D50, in Rec. 2020.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paper {
    /// The input read as density above the bare film: `−log10((s + pedestal)·gain) − black`, held at the guard.
    pub gain: V3,
    pub pedestal: f32,
    pub log_guard: f32,
    pub black: V3,
    /// Per wavelength the negative passes: its three dyes' density at unit amount, then its base's.
    pub negative: &'static [V4],
    /// Per wavelength the negative passes: the lamp's light read by each of the paper's three layers.
    pub exposure: &'static [V3],
    /// Per wavelength the negative passes: the cyan, magenta and yellow dichroic filters' transmittance.
    pub dichroic: &'static [V3],
    /// Each dichroic filter's transmittance where it absorbs, `10^(−CC/100)`: the enlarger's filtration.
    pub filters: V3,
    /// The print's exposure as a factor of the light: the reference grey's normalisation and Exposure's stops.
    pub scale: f32,
    /// Each layer's log exposure the contrast turns about, and its slope's factor there.
    pub pivot: V3,
    pub contrast: f32,
    /// Each layer's density along the log exposure from `curve_low` by `curve_step`, held at either end.
    pub curves: &'static [V3; PAPER_CURVE_NODES],
    pub curve_low: f32,
    pub curve_step: f32,
    /// Per wavelength the print reflects: its three dyes' density at unit amount, then its base's.
    pub print: &'static [V4],
    /// Per wavelength the print reflects: D50 read by the 1931 observer, adapted to D65, in Rec. 2020.
    pub view: &'static [V3],
}

impl Paper {
    pub fn head(&self) -> [u32; 4] {
        [self.negative.len() as u32, self.print.len() as u32, 0, 0]
    }

    /// The uniform: the reading, the enlarger, the curves' axis, then the bands and the curves.
    pub fn rows(&self) -> Vec<V4> {
        let mut rows = vec![
            v3(self.gain, self.pedestal),
            v3(self.black, self.log_guard),
            v3(self.filters, self.scale),
            v3(self.pivot, self.contrast),
            [self.curve_low, self.curve_step, 0.0, 0.0],
        ];
        let pad = |n: usize| std::iter::repeat_n([0.0; 4], PAPER_BANDS - n);
        rows.extend(self.negative.iter().copied().chain(pad(self.negative.len())));
        rows.extend(self.exposure.iter().map(|w| v3(*w, 0.0)).chain(pad(self.exposure.len())));
        rows.extend(self.dichroic.iter().map(|w| v3(*w, 0.0)).chain(pad(self.dichroic.len())));
        rows.extend(self.print.iter().copied().chain(pad(self.print.len())));
        rows.extend(self.view.iter().map(|w| v3(*w, 0.0)).chain(pad(self.view.len())));
        rows.extend(self.curves.iter().map(|c| v3(*c, 0.0)));
        rows
    }

    /// The dye amounts above the bare film a pixel holds, as the paper reads it.
    pub fn amounts(&self, s: V4) -> V3 {
        map(rgb(s), |c, v| -((v + self.pedestal) * self.gain[c]).max(self.log_guard).ln() * LOG10_E - self.black[c])
    }

    /// Each layer's log exposure under the negative holding `amounts`, before the contrast turns it.
    pub fn exposed(&self, amounts: V3) -> V3 {
        let mut raw = [0.0f32; 3];
        for ((dye, weight), filter) in self.negative.iter().zip(self.exposure).zip(self.dichroic) {
            let dimmed = map(*filter, |c, f| 1.0 - (1.0 - f) * (1.0 - self.filters[c]));
            let passed = (-LOG2_10 * (dot(amounts, rgb(*dye)) + dye[3])).exp2() * dimmed[0] * dimmed[1] * dimmed[2];
            raw = map(raw, |c, v| v + passed * weight[c]);
        }
        map(raw, |_, v| (v.max(0.0) * self.scale + PAPER_EXPOSURE_FLOOR).ln() * LOG10_E)
    }

    /// Each layer's density at log exposure `log`, turned about its pivot by the contrast.
    pub fn developed(&self, log: V3) -> V3 {
        map(log, |c, v| {
            let turned = self.pivot[c] + (v - self.pivot[c]) * self.contrast;
            let f = clamp((turned - self.curve_low) / self.curve_step, 0.0, (PAPER_CURVE_NODES - 1) as f32);
            let i = f.floor().min((PAPER_CURVE_NODES - 2) as f32);
            let at = i as usize;
            mix(self.curves[at][c], self.curves[at + 1][c], f - i)
        })
    }

    /// The print holding `density` of each dye, seen under D50, in linear Rec. 2020.
    pub fn viewed(&self, density: V3) -> V3 {
        let mut out = [0.0f32; 3];
        for (dye, weight) in self.print.iter().zip(self.view) {
            let t = (-LOG2_10 * (dot(density, rgb(*dye)) + dye[3])).exp2();
            out = map(out, |c, v| v + t * weight[c]);
        }
        out
    }
}

/// One negative pixel printed on the paper, in linear Rec. 2020; alpha kept.
pub fn paper(s: V4, k: &Paper) -> V4 {
    with_alpha(k.viewed(k.developed(k.exposed(k.amounts(s)))), s[3])
}

#[cfg(test)]
mod tests {
    use super::*;

    static NEGATIVE: [V4; 3] = [[1.0, 0.2, 0.05, 0.1], [0.1, 1.0, 0.2, 0.2], [0.02, 0.1, 1.0, 0.4]];
    static EXPOSURE: [V3; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    static DICHROIC: [V3; 3] = [[0.0, 1.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 0.0]];
    static PRINT: [V4; 3] = [[1.0, 0.0, 0.0, 0.05], [0.0, 1.0, 0.0, 0.05], [0.0, 0.0, 1.0, 0.05]];
    static VIEW: [V3; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    static CURVES: std::sync::LazyLock<[V3; PAPER_CURVE_NODES]> =
        std::sync::LazyLock::new(|| std::array::from_fn(|i| { let d = 0.01 * i as f32; [d, 2.0 * d, 3.0 * d] }));

    /// Three narrow bands, each alone in a layer and in a viewing channel, a linear curve on each layer.
    fn narrow() -> Paper {
        Paper {
            gain: [1.0; 3],
            pedestal: 0.0,
            log_guard: 1e-12,
            black: [0.0; 3],
            negative: &NEGATIVE,
            exposure: &EXPOSURE,
            dichroic: &DICHROIC,
            filters: [1.0; 3],
            scale: 1.0,
            pivot: [0.0; 3],
            contrast: 1.0,
            curves: &CURVES,
            curve_low: -2.0,
            curve_step: 0.01,
            print: &PRINT,
            view: &VIEW,
        }
    }

    #[test]
    fn a_narrow_paper_exposes_each_layer_through_its_band_and_a_filter_dims_its_own() {
        let k = narrow();
        let log = k.exposed([0.3, 0.6, 0.9]);
        let expected = [-(0.3 + 0.12 + 0.045 + 0.1), -(0.03 + 0.6 + 0.18 + 0.2), -(0.006 + 0.06 + 0.9 + 0.4)];
        assert!((0..3).all(|c| (log[c] - expected[c]).abs() < 1e-5), "{log:?}");
        // A yellow filter at 30 CC dims the blue band by 0.3 in density, the others not at all.
        let yellow = Paper { filters: [1.0, 1.0, 10f32.powf(-0.3)], ..k }.exposed([0.3, 0.6, 0.9]);
        assert!((yellow[2] - (expected[2] - 0.3)).abs() < 1e-5 && (yellow[0] - expected[0]).abs() < 1e-6, "{yellow:?}");
        // Twin: the same filter read as a magenta one dims green instead.
        let magenta = Paper { filters: [1.0, 10f32.powf(-0.3), 1.0], ..k }.exposed([0.3, 0.6, 0.9]);
        assert!((magenta[2] - expected[2]).abs() < 1e-6 && (magenta[1] - expected[1]).abs() > 0.29);
    }

    #[test]
    fn the_curves_hold_their_ends_and_the_contrast_turns_about_the_pivot() {
        let k = Paper { pivot: [-1.0; 3], ..narrow() };
        assert_eq!(k.developed([-5.0, -5.0, -5.0]), [0.0; 3], "held under the first node");
        let top = k.developed([5.0; 3]);
        assert!((top[2] - 3.0 * 0.01 * (PAPER_CURVE_NODES - 1) as f32).abs() < 1e-4, "held past the last: {top:?}");
        let mid = k.developed([-1.5; 3]);
        assert!((mid[0] - 0.5).abs() < 1e-5, "{mid:?}");
        let steep = Paper { contrast: 2.0, ..k }.developed([-1.5, -1.0, -0.5]);
        assert!((steep[0] - 0.0).abs() < 1e-5 && (steep[1] - 2.0).abs() < 1e-4 && (steep[2] - 6.0).abs() < 1e-4, "{steep:?}");
        // Twin: the plain contrast leaves the slope as it is, away from the pivot.
        assert!((k.developed([-0.5; 3])[2] - 4.5).abs() < 1e-4);
    }

    #[test]
    fn the_print_reads_its_dyes_and_base_and_the_reading_lies_above_the_black() {
        let k = narrow();
        let out = k.viewed([0.5, 1.0, 2.0]);
        let expected = [0.55, 1.05, 2.05].map(|d: f32| 10f32.powf(-d));
        assert!((0..3).all(|c| (out[c] - expected[c]).abs() < 1e-6), "{out:?}");
        let read = Paper { black: [0.2; 3], gain: [2.0, 1.0, 1.0], ..k }.amounts([0.01, 0.01, 1e-20, 1.0]);
        assert!((read[0] - (-(0.02f32).log10() - 0.2)).abs() < 1e-5 && (read[2] - 11.8).abs() < 1e-4, "held at the guard: {read:?}");
        // Twin: no black reads the full density.
        assert!((k.amounts([0.01, 0.01, 0.01, 1.0])[1] - 2.0).abs() < 1e-5);
    }
}
