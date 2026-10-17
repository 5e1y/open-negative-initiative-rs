//! RAW decoding through LibRaw, pinned: demosaiced once, then turned into scene-linear Rec. 2020 in floating point.

use std::ffi::CString;
use std::os::raw::{c_char, c_float, c_int, c_ushort, c_void};
use std::path::Path;
use std::sync::Mutex;

#[repr(C)]
struct Decoded {
    width: c_int,
    height: c_int,
    colors: c_int,
    dng: c_int,
    cam_xyz: [[c_float; 3]; 4],
    baseline_exposure: c_float,
    pixels: *const c_ushort,
    image: *mut c_void,
    raw: *mut c_void,
}

unsafe extern "C" {
    fn oni_open(path: *const c_char, exposure_gain: c_float, out: *mut Decoded) -> c_int;
    fn oni_unpack(d: *mut Decoded) -> c_int;
    fn oni_process(d: *mut Decoded, mul: *const c_float) -> c_int;
    fn oni_release(d: *mut Decoded);
}

/// Frees LibRaw's buffers on every exit path.
struct Session(Decoded);

impl Drop for Session {
    fn drop(&mut self) {
        unsafe { oni_release(&mut self.0) };
    }
}

/// LibRaw's DNG unpacker fills a static cosine table on first use, unguarded (`ljpeg_idct`): DNG
/// unpacks take this lock, every other stage of every decode runs concurrently.
static DNG_UNPACK: Mutex<()> = Mutex::new(());

type M3 = [[f64; 3]; 3];

/// The neutral of the Swift app's `CIRAWFilter`, 5000 K and tint 0: Robertson's isotemperature node at
/// 200 mired, on the Planckian locus, in CIE 1960 (u, v), the table DNG temperature and tint are read on.
const NEUTRAL_UV: (f64, f64) = (0.21142, 0.32312);

const D65_XY: (f64, f64) = (0.3127, 0.3290);
const REC2020_XY: [(f64, f64); 3] = [(0.708, 0.292), (0.170, 0.797), (0.131, 0.046)];

const BRADFORD: M3 = [
    [0.8951, 0.2664, -0.1614],
    [-0.7502, 1.7135, 0.0367],
    [0.0389, -0.0685, 1.0296],
];

/// Stops added to a file that names no BaselineExposure: without them `CIRAWFilter` sits 0.48 (Z f, roll
/// #8) and 0.47 (S1R) stop over this decode in Rec. 2020 luminance; one value for every camera.
pub const DEFAULT_BASELINE_EV: f64 = 0.48;

pub struct Linear {
    pub width: usize,
    pub height: usize,
    /// Row-major RGBA, scene-linear Rec. 2020, alpha 1; unclamped above 1.
    pub pixels: Vec<[f32; 4]>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct DecodeError(pub i32);

/// The file is not a three-colour sensor with a colour matrix, so no balance can be derived.
pub const NO_COLOUR_MATRIX: i32 = -4;

/// The multipliers LibRaw demosaics on and the matrix from its 16-bit output to Rec. 2020.
#[derive(Debug)]
struct Conversion {
    mul: [f32; 3],
    matrix: [[f32; 3]; 3],
}

fn mul(a: &M3, b: &M3) -> M3 {
    let mut m = [[0.0; 3]; 3];
    for (r, row) in m.iter_mut().enumerate() {
        for (c, cell) in row.iter_mut().enumerate() {
            *cell = (0..3).map(|k| a[r][k] * b[k][c]).sum();
        }
    }
    m
}

fn apply(m: &M3, v: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|r| m[r][0] * v[0] + m[r][1] * v[1] + m[r][2] * v[2])
}

fn inverse(m: &M3) -> Option<M3> {
    let cof = |r: usize, c: usize| {
        let (r0, r1, c0, c1) = ((r + 1) % 3, (r + 2) % 3, (c + 1) % 3, (c + 2) % 3);
        m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0]
    };
    let det: f64 = (0..3).map(|c| m[0][c] * cof(0, c)).sum();
    if det.abs() < 1e-12 || !det.is_finite() {
        return None;
    }
    let mut inv = [[0.0; 3]; 3];
    for (r, row) in inv.iter_mut().enumerate() {
        for (c, cell) in row.iter_mut().enumerate() {
            *cell = cof(c, r) / det;
        }
    }
    Some(inv)
}

fn xyz((x, y): (f64, f64)) -> [f64; 3] {
    [x / y, 1.0, (1.0 - x - y) / y]
}

fn neutral_xyz() -> [f64; 3] {
    let (u, v) = NEUTRAL_UV;
    let d = u - 4.0 * v + 2.0;
    xyz((1.5 * u / d, v / d))
}

/// XYZ to linear Rec. 2020, a D65 white of Y = 1 landing on (1, 1, 1).
fn rec2020_from_xyz() -> M3 {
    let p = REC2020_XY.map(xyz);
    let primaries = [0, 1, 2].map(|r| [p[0][r], p[1][r], p[2][r]]);
    let to_xyz_unscaled = inverse(&primaries).expect("Rec. 2020 primaries are independent");
    let s = apply(&to_xyz_unscaled, xyz(D65_XY));
    let to_xyz = [0, 1, 2].map(|r| [0, 1, 2].map(|c| primaries[r][c] * s[c]));
    inverse(&to_xyz).expect("scaled primaries are independent")
}

/// Bradford's von Kries adaptation from white `from` to white `to`.
fn bradford(from: [f64; 3], to: [f64; 3]) -> M3 {
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

/// The 5000 K balance of a camera whose `cam_xyz` maps XYZ to its channels: the neutral there reads
/// (1, 1, 1) when its most sensitive channel reaches the white level, times `2^stops`.
fn conversion(cam_xyz: &M3, stops: f64) -> Option<Conversion> {
    let white = neutral_xyz();
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
    let to_2020 = mul(&rec2020_from_xyz(), &bradford(white, xyz(D65_XY)));
    let m = mul(&to_2020, &inverse(cam_xyz)?);
    let matrix = [0, 1, 2].map(|r| [0, 1, 2].map(|c| (m[r][c] * undo[c]) as f32));
    Some(Conversion {
        mul: n.map(|v| (1.0 / v) as f32),
        matrix,
    })
}

/// The stops a decode adds: the file's own BaselineExposure, else the camera RAW default.
fn baseline(tag: f32) -> f64 {
    if tag > -999.0 {
        tag as f64
    } else {
        DEFAULT_BASELINE_EV
    }
}

/// Decodes `path`; `exposure_gain` is LibRaw's linear exposure shift, 1 for none, used by the linearity check.
/// Safe to call from several threads at once: every decode owns its LibRaw instance.
pub fn decode(path: &Path, exposure_gain: f32) -> Result<Linear, DecodeError> {
    let c_path = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| DecodeError(-3))?;
    let mut s = Session(unsafe { std::mem::zeroed() });
    let rc = unsafe { oni_open(c_path.as_ptr(), exposure_gain, &mut s.0) };
    if rc != 0 {
        return Err(DecodeError(rc));
    }
    let rc = if s.0.dng != 0 {
        let _held = DNG_UNPACK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { oni_unpack(&mut s.0) }
    } else {
        unsafe { oni_unpack(&mut s.0) }
    };
    if rc != 0 {
        return Err(DecodeError(rc));
    }
    let d = &s.0;
    let cam_xyz: M3 = [0, 1, 2].map(|r| d.cam_xyz[r].map(f64::from));
    let conv = (d.colors == 3)
        .then(|| conversion(&cam_xyz, baseline(d.baseline_exposure)))
        .flatten();
    let conv = conv.ok_or(DecodeError(NO_COLOUR_MATRIX))?;
    let rc = unsafe { oni_process(&mut s.0, conv.mul.as_ptr()) };
    if rc != 0 {
        return Err(DecodeError(rc));
    }
    let d = &s.0;
    let (w, h) = (d.width as usize, d.height as usize);
    let raw = unsafe { std::slice::from_raw_parts(d.pixels, w * h * 3) };
    let m = conv.matrix;
    let pixels = raw
        .chunks_exact(3)
        .map(|p| {
            let cam = [
                p[0] as f32 / 65535.0,
                p[1] as f32 / 65535.0,
                p[2] as f32 / 65535.0,
            ];
            let t = |r: usize| m[r][0] * cam[0] + m[r][1] * cam[1] + m[r][2] * cam[2];
            [t(0), t(1), t(2), 1.0]
        })
        .collect();
    if std::env::var_os("ONI_DECODE_TRACE").is_some() {
        eprintln!(
            "mul {:?} baseline {} cam_xyz {:?}\nmatrix {:?}",
            conv.mul, d.baseline_exposure, d.cam_xyz, m
        );
    }
    Ok(Linear {
        width: w,
        height: h,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Adobe's matrix for the Nikon Z f, as LibRaw holds it.
    const Z_F: M3 = [
        [1.1607, -0.4491, -0.0977],
        [-0.4522, 1.246, 0.2304],
        [-0.0458, 0.1519, 0.7616],
    ];

    /// What LibRaw hands out for a white `w` whose most sensitive channel reaches the white level.
    fn rendered(conv: &Conversion, w: [f64; 3]) -> [f64; 3] {
        let n = apply(&Z_F, w);
        let hi = n.iter().cloned().fold(0.0, f64::max);
        let top = conv.mul.iter().cloned().fold(0.0, f32::max) as f64;
        let o = [0, 1, 2].map(|c| n[c] / hi * conv.mul[c] as f64 / top);
        let m = conv.matrix.map(|r| r.map(f64::from));
        apply(&m, o)
    }

    #[test]
    fn the_5000_k_white_reads_neutral_at_the_baseline() {
        let conv = conversion(&Z_F, DEFAULT_BASELINE_EV).unwrap();
        let level = DEFAULT_BASELINE_EV.exp2();
        let out = rendered(&conv, neutral_xyz());
        for v in out {
            assert!((v - level).abs() < 1e-5, "{out:?}");
        }
        // Twin: a D65 white, the balance of LibRaw's own pre_mul, comes out visibly blue.
        let d65 = rendered(&conv, xyz(D65_XY));
        assert!(d65[2] / d65[0] > 1.15, "{d65:?}");
    }

    /// True when `w` has the chromaticity of a 5000 K Planckian radiator, (0.3451, 0.3516).
    fn planckian_5000(w: [f64; 3]) -> bool {
        let (x, y) = (w[0] / w.iter().sum::<f64>(), w[1] / w.iter().sum::<f64>());
        (x - 0.3451).abs() < 2e-4 && (y - 0.3516).abs() < 2e-4
    }

    #[test]
    fn the_neutral_is_the_planckian_5000_k() {
        assert!(planckian_5000(neutral_xyz()), "{:?}", neutral_xyz());
        // Twin: D50 sits 0.007 above the locus in y and must not pass.
        assert!(!planckian_5000(xyz((0.3457, 0.3585))));
    }

    #[test]
    fn rec2020_and_bradford_land_whites_on_one() {
        let d65 = xyz(D65_XY);
        let to_2020 = rec2020_from_xyz();
        for v in apply(&to_2020, d65) {
            assert!((v - 1.0).abs() < 1e-9);
        }
        let adapted = apply(&bradford(neutral_xyz(), d65), neutral_xyz());
        for c in 0..3 {
            assert!((adapted[c] - d65[c]).abs() < 1e-9);
        }
        // Twin: without adaptation the 5000 K white is not grey in Rec. 2020.
        let raw = apply(&to_2020, neutral_xyz());
        assert!(raw[0] / raw[2] > 1.2, "{raw:?}");
    }

    #[test]
    fn a_dng_tag_replaces_the_default() {
        assert_eq!(baseline(-999.0), DEFAULT_BASELINE_EV);
        assert_eq!(baseline(0.35), 0.35f32 as f64);
        // Twin: a tag of zero is a value, not an absence.
        assert_ne!(baseline(0.0), DEFAULT_BASELINE_EV);
    }

    #[test]
    fn a_singular_matrix_is_refused() {
        assert!(conversion(&[[0.0; 3]; 3], 0.0).is_none());
        // Twin: the Z f matrix is accepted.
        assert!(conversion(&Z_F, 0.0).is_some());
    }
}
