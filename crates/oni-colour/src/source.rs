//! A source read outside the RAW engine into the working space as Core Image reads it: its curve
//! undone, then its primaries kept (`channelsHeld`) or converted to linear Rec. 2020.

use moxcms::{ColorProfile, DataColorSpace, ToneReprCurve};
use oni_settings::SourceEncoding;

use crate::curve::Curve;
use crate::pcs::{self, Mat3};
use crate::working::{PCS_TO_WORKING, bradford};

/// A matrix-shaper source: one curve per channel, and its linear RGB onto D50 XYZ.
pub struct Source {
    curves: [Curve; 3],
    to_pcs: Mat3,
}

impl Source {
    /// The space a file is read in: the profile it embeds, else the chosen encoding, else sRGB.
    /// A grey file chosen Linear reads linear; any other grey choice keeps Core Image's default.
    pub fn of(icc: Option<&[u8]>, chosen: Option<SourceEncoding>, grey: bool) -> Source {
        if let Some(found) = icc.and_then(|d| ColorProfile::new_from_slice(d).ok()).and_then(|p| Self::from_profile(&p)) {
            return found;
        }
        let profile = match chosen {
            Some(SourceEncoding::Linear) => {
                let mut p = ColorProfile::new_srgb();
                let linear = ToneReprCurve::Lut(vec![]);
                (p.red_trc, p.green_trc, p.blue_trc) = (Some(linear.clone()), Some(linear.clone()), Some(linear));
                p
            }
            Some(SourceEncoding::DisplayP3) if !grey => ColorProfile::new_display_p3(),
            _ => ColorProfile::new_srgb(),
        };
        Self::from_profile(&profile).expect("the built-in spaces are matrix-shaper profiles")
    }

    /// `None` for a profile carrying no matrix and curves, such as a LUT-only one.
    pub fn from_profile(p: &ColorProfile) -> Option<Source> {
        match p.color_space {
            DataColorSpace::Rgb => {
                let curves = [&p.red_trc, &p.green_trc, &p.blue_trc].map(|t| t.as_ref().and_then(Curve::new));
                let [Some(r), Some(g), Some(b)] = curves else { return None };
                Some(Source { curves: [r, g, b], to_pcs: p.rgb_to_xyz_matrix().v })
            }
            // A grey is neutral in every primaries: its D50 white lands on the working white.
            DataColorSpace::Gray => {
                let curve = Curve::new(p.gray_trc.as_ref()?)?;
                Some(Source { curves: [curve.clone(), curve.clone(), curve], to_pcs: pcs::inverse(&PCS_TO_WORKING)? })
            }
            _ => None,
        }
    }

    /// Code values to linear light in place: on the working primaries, or the file's own when `held`.
    pub fn linearise(&self, pixels: &mut [[f32; 4]], held: bool) {
        let m = (!held).then(|| pcs::matmul(&PCS_TO_WORKING, &self.to_pcs));
        for p in pixels.iter_mut() {
            let lin: [f32; 3] = std::array::from_fn(|c| self.curves[c].eval(f64::from(p[c])) as f32);
            let out = m.as_ref().map_or(lin, |m| pcs::mul_f32(m, lin));
            p[..3].copy_from_slice(&out);
        }
    }
}

/// CalibrationIlluminant1's whites Core Image knows: A, D55, D65, D50.
fn illuminant(code: i64) -> Option<[f64; 3]> {
    Some(match code {
        17 => [1.09850, 1.0, 0.35585],
        20 => [0.95682, 1.0, 0.92149],
        21 => [0.95047, 1.0, 1.08883],
        23 => [0.96422, 1.0, 0.82521],
        _ => return None,
    })
}

/// `ContainerRead.calibratedSpace` converted to the working space: the inverse of ColorMatrix1
/// (XYZ to the file's RGB), its white the named illuminant, else where RGB (1, 1, 1) lands.
pub fn calibrated_to_working(color_matrix: &[f64], illuminant_code: i64) -> Option<Mat3> {
    let m: Mat3 = std::array::from_fn(|r| std::array::from_fn(|c| color_matrix.get(r * 3 + c).copied().unwrap_or(f64::NAN)));
    if color_matrix.len() != 9 {
        return None;
    }
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if det.abs() <= 1e-9 {
        return None;
    }
    let to_xyz = pcs::inverse(&m)?;
    let own = pcs::mul(&to_xyz, [1.0, 1.0, 1.0]);
    if own[1] <= 0.0 {
        return None;
    }
    let white = illuminant(illuminant_code).unwrap_or([own[0] / own[1], 1.0, own[2] / own[1]]);
    Some(pcs::matmul(&PCS_TO_WORKING, &pcs::matmul(&bradford(white, [0.9642, 1.0, 0.8249]), &to_xyz)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_srgb_code_linearises_and_converts_only_when_not_held() {
        let s = Source::of(None, None, false);
        let mut held = vec![[0.5, 0.5, 0.5, 1.0], [1.0, 0.0, 0.0, 1.0]];
        s.linearise(&mut held, true);
        // sRGB's 0.5 is 0.214 in light, whatever the primaries.
        assert!((held[0][0] - 0.2140).abs() < 1e-3, "{:?}", held[0]);
        assert!((held[1][0] - 1.0).abs() < 1e-5 && held[1][1] == 0.0 && held[1][2] == 0.0, "held, a primary stays the file's own");
        let mut converted = vec![[0.5, 0.5, 0.5, 1.0], [1.0, 0.0, 0.0, 1.0]];
        s.linearise(&mut converted, false);
        assert!((converted[0][0] - converted[0][2]).abs() < 1e-3, "a grey stays grey: {:?}", converted[0]);
        // Twin: converted, sRGB's red lies inside Rec. 2020, mixed with some green and blue.
        assert!(converted[1][1] > 0.02 && converted[1][0] < 0.7, "{:?}", converted[1]);
        // A grey file chosen Linear reads its codes as light.
        let mut linear = vec![[0.5, 0.5, 0.5, 1.0]];
        Source::of(None, Some(SourceEncoding::Linear), true).linearise(&mut linear, true);
        assert!((linear[0][0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_dng_matrix_converts_its_own_white_to_the_working_white() {
        // ColorMatrix1 of the sRGB primaries on D65: XYZ to linear sRGB.
        let cm = [3.2406, -1.5372, -0.4986, -0.9689, 1.8758, 0.0415, 0.0557, -0.2040, 1.0570];
        let m = calibrated_to_working(&cm, 21).unwrap();
        let white = pcs::mul(&m, [1.0, 1.0, 1.0]);
        assert!(white.iter().all(|v| (v - 1.0).abs() < 2e-3), "{white:?}");
        // Twin: a singular matrix names no space.
        assert!(calibrated_to_working(&[1.0, 2.0, 3.0, 2.0, 4.0, 6.0, 0.0, 0.0, 1.0], 21).is_none());
    }
}
