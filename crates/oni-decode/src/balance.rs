//! The 5000 K balance and the matrix from a camera's channels to scene-linear Rec. 2020, derived from its own `cam_xyz`.

use crate::m3::{apply, inverse, mul};
use crate::{DEFAULT_BASELINE_EV, M3, Neutral};

/// The neutral of the Swift app's `CIRAWFilter`, 5000 K and tint 0: Robertson's isotemperature node at
/// 200 mired, on the Planckian locus, in CIE 1960 (u, v), the table DNG temperature and tint are read on.
pub(crate) const NEUTRAL_UV: (f64, f64) = (0.21142, 0.32312);

pub(crate) const D65_XY: (f64, f64) = (0.3127, 0.3290);
pub(crate) const REC2020_XY: [(f64, f64); 3] = [(0.708, 0.292), (0.170, 0.797), (0.131, 0.046)];

pub(crate) const BRADFORD: M3 = [
    [0.8951, 0.2664, -0.1614],
    [-0.7502, 1.7135, 0.0367],
    [0.0389, -0.0685, 1.0296],
];

/// The multipliers LibRaw demosaics on and the matrix from its 16-bit output to Rec. 2020.
#[derive(Debug)]
pub(crate) struct Conversion {
    pub(crate) mul: [f32; 3],
    pub(crate) matrix: [[f32; 3]; 3],
}

pub(crate) fn xyz((x, y): (f64, f64)) -> [f64; 3] {
    [x / y, 1.0, (1.0 - x - y) / y]
}

pub(crate) fn white_xyz(neutral: Neutral) -> [f64; 3] {
    let (u, v) = neutral.uv();
    let d = u - 4.0 * v + 2.0;
    xyz((1.5 * u / d, v / d))
}

/// XYZ to linear Rec. 2020, a D65 white of Y = 1 landing on (1, 1, 1).
pub(crate) fn rec2020_from_xyz() -> M3 {
    let p = REC2020_XY.map(xyz);
    let primaries = [0, 1, 2].map(|r| [p[0][r], p[1][r], p[2][r]]);
    let to_xyz_unscaled = inverse(&primaries).expect("Rec. 2020 primaries are independent");
    let s = apply(&to_xyz_unscaled, xyz(D65_XY));
    let to_xyz = [0, 1, 2].map(|r| [0, 1, 2].map(|c| primaries[r][c] * s[c]));
    inverse(&to_xyz).expect("scaled primaries are independent")
}

/// Bradford's von Kries adaptation from white `from` to white `to`.
pub(crate) fn bradford(from: [f64; 3], to: [f64; 3]) -> M3 {
    let (a, b) = (apply(&BRADFORD, from), apply(&BRADFORD, to));
    let gains = [
        [b[0] / a[0], 0.0, 0.0],
        [0.0, b[1] / a[1], 0.0],
        [0.0, 0.0, b[2] / a[2]],
    ];
    mul(
        &inverse(&BRADFORD).expect("Bradford is invertible"),
        &mul(&gains, &BRADFORD),
    )
}

/// A camera's channels to working Rec. 2020 as a decode lays them, less its scale: `cam_xyz` inverted, the light of
/// `neutral` adapted by Bradford to D65; none where the matrix has no inverse.
pub(crate) fn working_from(cam_xyz: &M3, neutral: Neutral) -> Option<M3> {
    let to_2020 = mul(&rec2020_from_xyz(), &bradford(white_xyz(neutral), xyz(D65_XY)));
    Some(mul(&to_2020, &inverse(cam_xyz)?))
}

/// `working_from` for a negative's 5000 K, as a scanned negative's RAW is decoded: what ONI's own scanner reads through.
pub fn working_from_camera(cam_xyz: &[[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    working_from(cam_xyz, Neutral::Swift)
}

/// The 5000 K balance of a camera whose `cam_xyz` maps XYZ to its channels: the neutral there reads
/// (1, 1, 1) when its most sensitive channel reaches the white level, times `2^stops`.
pub(crate) fn converted_on(cam_xyz: &M3, stops: f64, neutral: Neutral) -> Option<Conversion> {
    let white = white_xyz(neutral);
    let n = apply(cam_xyz, white);
    if n.iter().any(|v| !v.is_finite() || *v <= 0.0) {
        return None;
    }
    let (lo, hi) = (
        n.iter().cloned().fold(f64::MAX, f64::min),
        n.iter().cloned().fold(0.0, f64::max),
    );
    // LibRaw scales channel c by (1 / n_c) / max(1 / n), so its output o_c is r_c * lo / n_c.
    let undo = [0, 1, 2].map(|c| hi * n[c] / lo * stops.exp2());
    let m = working_from(cam_xyz, neutral)?;
    Some(Conversion {
        mul: n.map(|v| (1.0 / v) as f32),
        matrix: [0, 1, 2].map(|r| [0, 1, 2].map(|c| (m[r][c] * undo[c]) as f32)),
    })
}

/// The stops a decode adds: the file's own BaselineExposure, else the camera RAW default.
pub(crate) fn baseline(tag: f32) -> f64 {
    if tag > -999.0 {
        tag as f64
    } else {
        DEFAULT_BASELINE_EV
    }
}
