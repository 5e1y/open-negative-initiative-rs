//! The working space, linear Rec. 2020, and the PCS-side profile the ICC legs go through.

use std::sync::LazyLock;

use moxcms::{ColorProfile, DataColorSpace, ProfileClass, ToneReprCurve, Xyzd};

use crate::pcs::{self, Mat3};

/// Linear Rec. 2020 to D50 XYZ: the primaries on D65, Bradford-adapted to the ICC's own D50,
/// so the working white lands on the PCS white exactly.
pub static WORKING_TO_PCS: LazyLock<Mat3> = LazyLock::new(|| {
    let primaries = [(0.708, 0.292), (0.170, 0.797), (0.131, 0.046)];
    let xyz = |(x, y): (f64, f64)| [x / y, 1.0, (1.0 - x - y) / y];
    let columns = primaries.map(xyz);
    let m: Mat3 = std::array::from_fn(|r| std::array::from_fn(|c| columns[c][r]));
    let d65 = xyz((0.3127, 0.3290));
    let s = pcs::mul(&pcs::inverse(&m).expect("independent primaries"), d65);
    let native: Mat3 = std::array::from_fn(|r| std::array::from_fn(|c| m[r][c] * s[c]));
    pcs::matmul(&bradford(d65, [0.9642, 1.0, 0.8249]), &native)
});

/// The Bradford adaptation from one white to another.
pub fn bradford(from: [f64; 3], to: [f64; 3]) -> Mat3 {
    let b: Mat3 = [[0.8951, 0.2664, -0.1614], [-0.7502, 1.7135, 0.0367], [0.0389, -0.0685, 1.0296]];
    let (cf, ct) = (pcs::mul(&b, from), pcs::mul(&b, to));
    let scale: Mat3 = std::array::from_fn(|r| std::array::from_fn(|c| if r == c { ct[r] / cf[r] } else { 0.0 }));
    pcs::matmul(&pcs::inverse(&b).expect("Bradford is invertible"), &pcs::matmul(&scale, &b))
}

pub static PCS_TO_WORKING: LazyLock<Mat3> =
    LazyLock::new(|| pcs::inverse(&WORKING_TO_PCS).expect("Rec. 2020 primaries are independent"));

/// The working space as an ICC profile: its own colorants on a linear curve.
pub fn working_profile() -> ColorProfile {
    let mut profile = ColorProfile::new_bt2020();
    let m = &*WORKING_TO_PCS;
    let column = |c: usize| Xyzd { x: m[0][c], y: m[1][c], z: m[2][c] };
    (profile.red_colorant, profile.green_colorant, profile.blue_colorant) = (column(0), column(1), column(2));
    let linear = ToneReprCurve::Lut(vec![]);
    profile.red_trc = Some(linear.clone());
    profile.green_trc = Some(linear.clone());
    profile.blue_trc = Some(linear);
    profile
}

/// D50 XYZ carried as linear RGB through identity colorants, so a CMM converts a matrix
/// profile's values to or from XYZ.
pub static PCS_PROFILE: LazyLock<ColorProfile> = LazyLock::new(|| {
    let d50 = Xyzd { x: 0.9642, y: 1.0, z: 0.8249 };
    let linear = ToneReprCurve::Lut(vec![]);
    let mut profile = ColorProfile::default();
    profile.profile_class = ProfileClass::ColorSpace;
    profile.color_space = DataColorSpace::Rgb;
    profile.pcs = DataColorSpace::Xyz;
    profile.white_point = d50;
    profile.media_white_point = Some(d50);
    profile.red_colorant = Xyzd { x: 1.0, y: 0.0, z: 0.0 };
    profile.green_colorant = Xyzd { x: 0.0, y: 1.0, z: 0.0 };
    profile.blue_colorant = Xyzd { x: 0.0, y: 0.0, z: 1.0 };
    profile.red_trc = Some(linear.clone());
    profile.green_trc = Some(linear.clone());
    profile.blue_trc = Some(linear);
    profile
});

/// The Rec. 2020 transfer of the cube space: the grid spends its points where the eye does.
const A: f64 = 1.099_296_826_809_44;
const BETA: f64 = 0.018_053_968_510_807;

pub fn rec2020_encode(linear: f32) -> f32 {
    let l = (linear as f64).abs();
    let v = if l < BETA { 4.5 * l } else { A * l.powf(0.45) - (A - 1.0) };
    (v as f32).copysign(linear)
}

pub fn rec2020_decode(encoded: f32) -> f32 {
    let v = (encoded as f64).abs();
    let l = if v < 4.5 * BETA { v / 4.5 } else { ((v + (A - 1.0)) / A).powf(1.0 / 0.45) };
    (l as f32).copysign(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn working_white_lands_on_d50() {
        let white = pcs::mul(&WORKING_TO_PCS, [1.0, 1.0, 1.0]);
        let d50 = [0.9642, 1.0, 0.8249];
        assert!((0..3).all(|i| (white[i] - d50[i]).abs() < 1e-9), "{white:?}");
        // Twin: the primaries left on D65, unadapted, put the white a fifth off D50's in Z.
        let native = pcs::matmul(&bradford(d50, [0.3127 / 0.3290, 1.0, (1.0 - 0.3127 - 0.3290) / 0.3290]), &WORKING_TO_PCS);
        let unadapted = pcs::mul(&native, [1.0, 1.0, 1.0]);
        assert!((unadapted[2] - d50[2]).abs() > 0.2, "{unadapted:?}");
    }

    #[test]
    fn rec2020_transfer_round_trips() {
        for x in [-0.5f32, 0.0, 0.001, 0.0180, 0.0181, 0.2, 0.9, 1.0, 1.4] {
            let back = rec2020_decode(rec2020_encode(x));
            assert!((back - x).abs() < 1e-5 * x.abs().max(1.0), "{x} -> {back}");
        }
        assert!((rec2020_encode(0.18) - 0.409).abs() < 1e-3);
        // Twin: the sRGB curve puts 18 % grey elsewhere.
        assert!((rec2020_encode(0.18) - 0.461).abs() > 0.03);
    }
}
