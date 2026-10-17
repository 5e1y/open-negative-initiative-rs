//! RAW decoding through LibRaw, pinned: demosaiced once, then turned into scene-linear Rec. 2020 in floating point.

use std::ffi::CString;
use std::os::raw::{c_char, c_float, c_int, c_ushort, c_void};
use std::path::Path;

#[repr(C)]
struct Decoded {
    width: c_int,
    height: c_int,
    pre_mul: [c_float; 4],
    rgb_cam: [[c_float; 4]; 3],
    pixels: *const c_ushort,
    image: *mut c_void,
    raw: *mut c_void,
}

unsafe extern "C" {
    fn oni_decode(path: *const c_char, exposure_gain: c_float, out: *mut Decoded) -> c_int;
    fn oni_release(d: *mut Decoded);
}

/// Linear sRGB to linear Rec. 2020, both on D65.
const SRGB_TO_2020: [[f32; 3]; 3] = [
    [0.627_404, 0.329_282, 0.043_314],
    [0.069_097, 0.919_540, 0.011_361],
    [0.016_392, 0.088_013, 0.895_595],
];

pub struct Linear {
    pub width: usize,
    pub height: usize,
    /// Row-major RGBA, scene-linear Rec. 2020, alpha 1; unclamped above 1.
    pub pixels: Vec<[f32; 4]>,
}

#[derive(Debug)]
pub struct DecodeError(pub i32);

/// Decodes `path`; `exposure_gain` is LibRaw's linear exposure shift, 1 for none, used by the linearity check.
pub fn decode(path: &Path, exposure_gain: f32) -> Result<Linear, DecodeError> {
    let c_path = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| DecodeError(-3))?;
    let mut d: Decoded = unsafe { std::mem::zeroed() };
    let rc = unsafe { oni_decode(c_path.as_ptr(), exposure_gain, &mut d) };
    if rc != 0 {
        return Err(DecodeError(rc));
    }
    let (w, h) = (d.width as usize, d.height as usize);
    let raw = unsafe { std::slice::from_raw_parts(d.pixels, w * h * 3) };
    let mut m = [[0f32; 3]; 3];
    for (r, row) in m.iter_mut().enumerate() {
        for (c, cell) in row.iter_mut().enumerate() {
            *cell = (0..3).map(|k| SRGB_TO_2020[r][k] * d.rgb_cam[k][c]).sum();
        }
    }
    let pixels = raw
        .chunks_exact(3)
        .map(|p| {
            let cam = [p[0] as f32 / 65535.0, p[1] as f32 / 65535.0, p[2] as f32 / 65535.0];
            let t = |r: usize| m[r][0] * cam[0] + m[r][1] * cam[1] + m[r][2] * cam[2];
            [t(0), t(1), t(2), 1.0]
        })
        .collect();
    unsafe { oni_release(&mut d) };
    Ok(Linear { width: w, height: h, pixels })
}
