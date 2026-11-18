//! A scene's white in kelvin and tint, as DNG reads them on Robertson's isotemperature lines, and the
//! relative balance that renders a decode made on one white as if made on another.

use serde::{Deserialize, Serialize};

/// A white on DNG's scale: kelvin along the Planckian locus, tint across it, + toward magenta.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct White {
    pub kelvin: f32,
    pub tint: f32,
}

/// CIE 1960 (u, v).
pub type Uv = (f64, f64);
pub type M3 = [[f64; 3]; 3];

/// The kelvin slider's travel, its middle the mosaic's own white.
pub const KELVIN_RANGE: (f32, f32) = (2000.0, 25000.0);
pub const KELVIN_CENTRE: f32 = 5000.0;
pub const TINT_RANGE: (f32, f32) = (-150.0, 150.0);

/// The white every mosaic is decoded on: Robertson's node at 200 mired, 5000 K and tint 0.
pub const MOSAIC_WHITE: Uv = (0.21142, 0.32312);

/// The white every other file's pixels already stand on: D65.
pub const FILE_WHITE: Uv = uv_of_xy(0.3127, 0.3290);

/// DNG's `kTintScale`: a tint of 1 is 1/3000 of a (u, v) unit across the isotherm.
const TINT_SCALE: f64 = -3000.0;

/// Robertson's isotemperature lines as the DNG SDK tabulates them: mired, u, v, slope.
const NODES: [(f64, f64, f64, f64); 31] = [
    (0.0, 0.18006, 0.26352, -0.24341),
    (10.0, 0.18066, 0.26589, -0.25479),
    (20.0, 0.18133, 0.26846, -0.26876),
    (30.0, 0.18208, 0.27119, -0.28539),
    (40.0, 0.18293, 0.27407, -0.30470),
    (50.0, 0.18388, 0.27709, -0.32675),
    (60.0, 0.18494, 0.28021, -0.35156),
    (70.0, 0.18611, 0.28342, -0.37915),
    (80.0, 0.18740, 0.28668, -0.40955),
    (90.0, 0.18880, 0.28997, -0.44278),
    (100.0, 0.19032, 0.29326, -0.47888),
    (125.0, 0.19462, 0.30141, -0.58204),
    (150.0, 0.19962, 0.30921, -0.70471),
    (175.0, 0.20525, 0.31647, -0.84901),
    (200.0, 0.21142, 0.32312, -1.0182),
    (225.0, 0.21807, 0.32909, -1.2168),
    (250.0, 0.22511, 0.33439, -1.4512),
    (275.0, 0.23247, 0.33904, -1.7298),
    (300.0, 0.24010, 0.34308, -2.0637),
    (325.0, 0.24702, 0.34655, -2.4681),
    (350.0, 0.25591, 0.34951, -2.9641),
    (375.0, 0.26400, 0.35200, -3.5814),
    (400.0, 0.27218, 0.35407, -4.3633),
    (425.0, 0.28039, 0.35577, -5.3762),
    (450.0, 0.28863, 0.35714, -6.7262),
    (475.0, 0.29685, 0.35823, -8.5955),
    (500.0, 0.30505, 0.35907, -11.324),
    (525.0, 0.31320, 0.35968, -15.628),
    (550.0, 0.32129, 0.36011, -23.325),
    (575.0, 0.32931, 0.36038, -40.770),
    (600.0, 0.33724, 0.36051, -116.45),
];

pub const fn uv_of_xy(x: f64, y: f64) -> Uv {
    let d = 1.5 - x + 6.0 * y;
    (2.0 * x / d, 3.0 * y / d)
}

/// The decode's own white: a mosaic's, else the file's.
pub fn anchor(mosaic: bool) -> Uv {
    if mosaic { MOSAIC_WHITE } else { FILE_WHITE }
}

/// The isotherm's unit direction from a node's slope.
fn across(slope: f64) -> (f64, f64) {
    let len = (1.0 + slope * slope).sqrt();
    (1.0 / len, slope / len)
}

/// `dng_temperature::Get_xy_coord`, in (u, v): the locus between the two nodes the mired falls
/// between, then the tint along the isotherm interpolated there.
pub fn uv(w: White) -> Uv {
    let r = 1e6 / f64::from(w.kelvin);
    let offset = f64::from(w.tint) / TINT_SCALE;
    let i = (0..30).find(|&i| r < NODES[i + 1].0).unwrap_or(29);
    let ((r0, u0, v0, t0), (r1, u1, v1, t1)) = (NODES[i], NODES[i + 1]);
    let f = (r1 - r) / (r1 - r0);
    let (u, v) = (u0 * f + u1 * (1.0 - f), v0 * f + v1 * (1.0 - f));
    let ((a0, b0), (a1, b1)) = (across(t0), across(t1));
    let (du, dv) = (a0 * f + a1 * (1.0 - f), b0 * f + b1 * (1.0 - f));
    let len = (du * du + dv * dv).sqrt();
    (u + du / len * offset, v + dv / len * offset)
}

/// `dng_temperature::Set_xy_coord` from (u, v): the first isotherm the point lies below, the
/// mired interpolated on its distances to that line and the one before.
pub fn of_uv((u, v): Uv) -> White {
    let mut last = (0.0, 0.0, 0.0);
    for i in 1..=30 {
        let (r, nu, nv, t) = NODES[i];
        let (du, dv) = across(t);
        let dt = -(u - nu) * dv + (v - nv) * du;
        if dt <= 0.0 || i == 30 {
            let dt = -dt.min(0.0);
            let f = if i == 1 { 0.0 } else { dt / (last.0 + dt) };
            let (rp, up, vp, _) = NODES[i - 1];
            let kelvin = 1e6 / (rp * f + r * (1.0 - f));
            let (uu, vv) = (u - (up * f + nu * (1.0 - f)), v - (vp * f + nv * (1.0 - f)));
            let (du, dv) = (du * (1.0 - f) + last.1 * f, dv * (1.0 - f) + last.2 * f);
            let len = (du * du + dv * dv).sqrt();
            let tint = (uu * du / len + vv * dv / len) * TINT_SCALE;
            return White { kelvin: kelvin as f32, tint: tint as f32 };
        }
        last = (dt, du, dv);
    }
    unreachable!("the last isotherm answers")
}

/// The white a look carries onto `target`: a digital frame takes the donor's, else rests on its own
/// As Shot; a film frame keeps its own, which it does not render.
pub fn carried(donor: Option<White>, target: &crate::PipelineSettings) -> Option<White> {
    if target.mode.is_film() { target.white } else { donor.or(target.shot) }
}

/// XYZ of a (u, v) white, Y = 1.
pub fn xyz((u, v): Uv) -> [f64; 3] {
    let d = u - 4.0 * v + 2.0;
    let (x, y) = (1.5 * u / d, v / d);
    [x / y, 1.0, (1.0 - x - y) / y]
}

const BRADFORD: M3 = [[0.8951, 0.2664, -0.1614], [-0.7502, 1.7135, 0.0367], [0.0389, -0.0685, 1.0296]];
const REC2020_XY: [(f64, f64); 3] = [(0.708, 0.292), (0.170, 0.797), (0.131, 0.046)];

/// The balance that renders a decode made on white `to` as if made on `from`, on linear Rec. 2020:
/// Bradford from `from` to `to` in XYZ, so a scene lit by `from` lands neutral.
pub fn relative(from: Uv, to: Uv) -> [[f32; 3]; 3] {
    let to_2020 = rec2020_from_xyz();
    let (a, b) = (apply(&BRADFORD, xyz(from)), apply(&BRADFORD, xyz(to)));
    let gains = [[b[0] / a[0], 0.0, 0.0], [0.0, b[1] / a[1], 0.0], [0.0, 0.0, b[2] / a[2]]];
    let adapted = mul(&inverse(&BRADFORD), &mul(&gains, &BRADFORD));
    let m = mul(&to_2020, &mul(&adapted, &inverse(&to_2020)));
    m.map(|row| row.map(|x| x as f32))
}

/// XYZ to linear Rec. 2020, D65 at Y = 1 landing on (1, 1, 1).
pub fn rec2020_from_xyz() -> M3 {
    let p = REC2020_XY.map(|(x, y)| [x / y, 1.0, (1.0 - x - y) / y]);
    let primaries = [0, 1, 2].map(|r| [p[0][r], p[1][r], p[2][r]]);
    let s = apply(&inverse(&primaries), xyz(FILE_WHITE));
    inverse(&[0, 1, 2].map(|r| [0, 1, 2].map(|c| primaries[r][c] * s[c])))
}

pub fn apply(m: &M3, v: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|r| m[r][0] * v[0] + m[r][1] * v[1] + m[r][2] * v[2])
}

fn mul(a: &M3, b: &M3) -> M3 {
    [0, 1, 2].map(|r| [0, 1, 2].map(|c| (0..3).map(|k| a[r][k] * b[k][c]).sum()))
}

/// The inverse of a matrix whose rows are independent, as every one here is by construction.
pub fn inverse(m: &M3) -> M3 {
    let c = |r: usize, k: usize| m[(r + 1) % 3][(k + 1) % 3] * m[(r + 2) % 3][(k + 2) % 3] - m[(r + 1) % 3][(k + 2) % 3] * m[(r + 2) % 3][(k + 1) % 3];
    let det: f64 = (0..3).map(|k| m[0][k] * c(0, k)).sum();
    [0, 1, 2].map(|r| [0, 1, 2].map(|k| c(k, r) / det))
}

#[cfg(test)]
mod tests;
