//! RAW decoding through LibRaw, pinned: demosaiced once, then turned into scene-linear Rec. 2020 in floating point.
//! `route` sorts a file onto its branch from its bytes; `container` reads a linear scan's TIFF.

pub mod camera;
pub mod container;
pub mod embedded;
pub mod plain;
pub mod cache;
mod linear;
mod source;
mod neutral;
mod m3;
mod size;
#[cfg(test)]
mod stamp;
mod ffi;
mod balance;
mod branch;
mod mosaic;
// Links LibRaw's archive; that crate holds no Rust to name.
use oni_libraw as _;
pub use size::full_size;
pub use neutral::{Neutral, SLIDE_KELVIN};

pub use balance::working_from_camera;
pub(crate) use balance::NEUTRAL_UV;
use balance::{baseline, converted_on};
#[cfg(test)]
use balance::{Conversion, D65_XY, bradford, rec2020_from_xyz, white_xyz, xyz};
#[cfg(test)]
use m3::apply;
pub use branch::{Branch, PLAIN_EXTENSIONS, decodes_via_mosaic, is_low_precision, is_raw, named_decodable, route, unsupported_format};
pub(crate) use ffi::DNG_UNPACK;
use ffi::{Session, oni_inset, oni_open, oni_process};
pub use mosaic::{Mosaic, is_nikon_he, mosaic, reads};

use source::{Route, Source};
use std::os::raw::c_int;
use std::path::Path;

type M3 = [[f64; 3]; 3];

pub use oni_settings::white::DEFAULT_BASELINE_EV;

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
/// The file ends before its pixels do: a copy interrupted or still running.
pub const CUT_SHORT: i32 = -5;
/// Nikon's High Efficiency RAW (HE, HE*), which LibRaw identifies but cannot unpack.
pub const NIKON_HE: i32 = -6;

impl DecodeError {
    pub fn is_cut_short(&self) -> bool {
        self.0 == CUT_SHORT
    }

    pub fn is_nikon_he(&self) -> bool {
        self.0 == NIKON_HE
    }
}

/// Decodes `path`; `exposure_gain` is LibRaw's linear exposure shift, 1 for none. Safe from several
/// threads at once: every decode owns its LibRaw instance.
pub fn decode(path: &Path, exposure_gain: f32) -> Result<Linear, DecodeError> {
    decode_for(path, exposure_gain, 0).map(|(linear, _)| linear)
}

/// The `side` that reads any sensor at LibRaw's half size, kept on disk as a whole decode is.
pub const HALF: usize = 1;

/// `decode` for a copy no longer than `side`: a mosaic twice as long is read at half size, a sensor
/// pixel per 2x2 block with no demosaic; the factor back to the full frame beside it.
pub fn decode_for(path: &Path, exposure_gain: f32, side: usize) -> Result<(Linear, usize), DecodeError> {
    decoded(path, exposure_gain, side, Neutral::Swift)
}

/// `decode_for` balanced on `neutral`.
pub fn decode_on(path: &Path, exposure_gain: f32, side: usize, neutral: Neutral) -> Result<(Linear, usize), DecodeError> {
    decoded(path, exposure_gain, side, neutral)
}

/// `decode_on` with or without the camera's colour matrix: without it, the camera's own channels balanced at `neutral`
/// are the working space.
pub fn decode_with(path: &Path, exposure_gain: f32, side: usize, neutral: Neutral, matrix: bool) -> Result<(Linear, usize), DecodeError> {
    decoded_by(path, exposure_gain, side, neutral, Route::PLATFORM, matrix)
}

fn decoded(path: &Path, exposure_gain: f32, side: usize, neutral: Neutral) -> Result<(Linear, usize), DecodeError> {
    decoded_by(path, exposure_gain, side, neutral, Route::PLATFORM, true)
}

/// The camera's own channels in place of `matrix`'s conversion: each balanced as `matrix` balances its neutral, its row
/// sums, all one, so the white still reads (1, 1, 1) times the baseline's gain.
fn unconverted(matrix: &[[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let gain = matrix.iter().map(|r| r.iter().sum::<f32>()).sum::<f32>() / 3.0;
    [[gain, 0.0, 0.0], [0.0, gain, 0.0], [0.0, 0.0, gain]]
}

fn decoded_by(path: &Path, exposure_gain: f32, side: usize, neutral: Neutral, route: Route, converted: bool) -> Result<(Linear, usize), DecodeError> {
    let laid = |m: [[f32; 3]; 3]| if converted { m } else { unconverted(&m) };
    // A whole or a `HALF` decode is kept on disk, and one kept is read back in place of LibRaw's;
    // its key keeps the word `Matrix`, so the decodes already kept stay valid.
    let slide = if neutral == Neutral::Slide { "|slide" } else { "" };
    let keyed = |side: usize| cache::key(path, &format!("{:08x}|Matrix|{side}{slide}", exposure_gain.to_bits()));
    let key = (side == 0 || side == HALF).then(|| keyed(side)).flatten();
    // A shorter copy LibRaw reads at half size is the `HALF` decode pixel for pixel: one kept serves it.
    let half = (side > HALF).then(|| keyed(HALF)).flatten();
    // The same decode running on another thread, a photo's ahead of its opening, is waited for and read back.
    let _flight = key.as_deref().map(cache::Flight::take);
    let kept = key.as_deref().and_then(cache::read).or_else(|| half.as_deref().and_then(cache::read).filter(|k| k.factor == 2 && k.width.max(k.height) >= side));
    if let Some(k) = kept {
        let pixels = linear::converted(&k.raw, k.width, (0, 0, k.width, k.height), k.flip, &laid(k.matrix));
        let (width, height) = linear::upright(k.flip, (k.width, k.height));
        return Ok((Linear { width, height, pixels }, k.factor));
    }
    let mut source = Source::open(path, route)?;
    let mut s = Session(unsafe { std::mem::zeroed() });
    let min_side = c_int::try_from(side).unwrap_or(0);
    let rc = unsafe { oni_open(source.name(), source.take(), exposure_gain, min_side, &mut s.0) };
    if rc != 0 {
        return Err(DecodeError(rc));
    }
    let rc = s.unpack();
    if rc != 0 {
        return Err(DecodeError(rc));
    }
    let d = &s.0;
    let cam_xyz: M3 = [0, 1, 2].map(|r| d.cam_xyz[r].map(f64::from));
    let conv = (d.colors == 3)
        .then(|| converted_on(&cam_xyz, baseline(d.baseline_exposure), neutral))
        .flatten();
    let conv = conv.ok_or(DecodeError(NO_COLOUR_MATRIX))?;
    let rc = unsafe { oni_process(&mut s.0, conv.mul.as_ptr()) };
    if rc != 0 {
        return Err(DecodeError(rc));
    }
    let d = &s.0;
    let (w, h) = (d.width as usize, d.height as usize);
    let raw = unsafe { std::slice::from_raw_parts(d.pixels, w * h * 3) };
    // The maker's default crop, the frame `CIRAWFilter` hands out: 6048 × 4032 of a D750's 6064 × 4040,
    // so a sidecar's fractions fall on the same pixels in either app.
    let mut inset = [0 as c_int; 4];
    unsafe { oni_inset(d, inset.as_mut_ptr()) };
    let factor = if d.half != 0 { 2 } else { 1 };
    let [cx, cy, cw, ch] = inset.map(|v| v.max(0) as usize / factor);
    let (x0, y0, w_out, h_out) = if cw > 0 && ch > 0 && cx + cw <= w && cy + ch <= h { (cx, cy, cw, ch) } else { (0, 0, w, h) };
    let matrix = conv.matrix;
    if let Some(key) = key {
        let head = cache::Kept { width: w_out, height: h_out, flip: d.flip, factor, matrix, raw: Vec::new() };
        cache::write(&key, &head, (y0..y0 + h_out).map(|row| &raw[(row * w + x0) * 3..(row * w + x0 + w_out) * 3]));
    }
    let pixels = linear::converted(raw, w, (x0, y0, w_out, h_out), d.flip, &laid(matrix));
    let (w_out, h_out) = linear::upright(d.flip, (w_out, h_out));
    if std::env::var_os("ONI_DECODE_TRACE").is_some() {
        eprintln!(
            "mul {:?} baseline {} cam_xyz {:?}\nmatrix {:?}",
            conv.mul, d.baseline_exposure, d.cam_xyz, conv.matrix
        );
    }
    Ok((Linear { width: w_out, height: h_out, pixels }, factor))
}

#[cfg(test)]
mod tests;
