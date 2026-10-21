//! RAW decoding through LibRaw, pinned: demosaiced once, then turned into scene-linear Rec. 2020 in floating point.
//! `route` sorts a file onto its branch from its bytes; `container` reads a linear scan's TIFF.

pub mod camera;
pub mod rendition;
pub mod container;
pub mod embedded;
pub mod plain;
pub mod cache;
mod linear;

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
    half: c_int,
    inset: [c_int; 4],
    flip: c_int,
    make: [c_char; 64],
    model: [c_char; 64],
}

unsafe extern "C" {
    fn oni_open(path: *const c_char, exposure_gain: c_float, min_side: c_int, out: *mut Decoded) -> c_int;
    fn oni_unpack(d: *mut Decoded) -> c_int;
    fn oni_inset(d: *const Decoded, out: *mut c_int);
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
pub(crate) static DNG_UNPACK: Mutex<()> = Mutex::new(());

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

/// How a RAW's channels reach the working space: LibRaw demosaics on the same multipliers in each,
/// only the matrix after it differs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Colours {
    /// Through the camera's matrix to XYZ, Bradford to D65, then Rec. 2020.
    #[default]
    Matrix,
    /// The sensor's own channels, each scaled by the 5000 K balance alone: no channel feeds another.
    Sensor,
    /// The sensor's own channels as the photosites counted them, no balance at all.
    SensorUnbalanced,
}

/// The 5000 K balance of a camera whose `cam_xyz` maps XYZ to its channels: the neutral there reads
/// (1, 1, 1) when its most sensitive channel reaches the white level, times `2^stops`.
fn converted_by(cam_xyz: &M3, stops: f64, colours: Colours) -> Option<Conversion> {
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
    let diagonal = |gain: [f64; 3]| [0, 1, 2].map(|r| [0, 1, 2].map(|c| if r == c { gain[c] as f32 } else { 0.0 }));
    let matrix = match colours {
        Colours::Matrix => [0, 1, 2].map(|r| [0, 1, 2].map(|c| (m[r][c] * undo[c]) as f32)),
        // The neutral reads (1, 1, 1) at the white level, as through the matrix, so grey keeps its level.
        Colours::Sensor => diagonal([hi / lo * stops.exp2(); 3]),
        // The most sensitive channel reads 1 at the white level; the others read their share of it.
        Colours::SensorUnbalanced => diagonal(undo.map(|u| u / hi)),
    };
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

/// The branch a file decodes through, as `RawDecode.alone` sorts it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Branch {
    /// A camera mosaic, through LibRaw.
    Mosaic,
    /// A demosaiced linear scan in a TIFF container, through `container`.
    LinearRgb,
    /// Anything else, read by an image reader in its declared or chosen space.
    Plain,
}

/// Camera RAW extensions (`UTType.rawImage`), the fallback for a file whose bytes no probe claims.
const RAW_EXTENSIONS: &[&str] = &[
    "3fr", "ari", "arw", "bay", "braw", "cap", "cr2", "cr3", "crw", "dcr", "dcs", "dng", "drf", "eip", "erf", "fff", "gpr",
    "iiq", "k25", "kdc", "mdc", "mef", "mos", "mrw", "nef", "nrw", "orf", "pef", "ptx", "pxn", "r3d", "raf", "raw", "rw2",
    "rwl", "rwz", "sr2", "srf", "srw", "x3f",
];

/// `RawDecode.isRaw`: the extension, consulted only when the bytes claim nothing.
pub fn is_raw(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| RAW_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Sorted on the bytes: a CFA is a mosaic, LinearRaw a linear scan; a file neither claims falls back
/// on the extension, so the probe only ever adds files to the linear branch.
pub fn route(path: &Path) -> Branch {
    match container::probe(path) {
        Some(container::Kind::Mosaic) => Branch::Mosaic,
        Some(container::Kind::LinearRgb) => Branch::LinearRgb,
        _ if is_raw(path) => Branch::Mosaic,
        _ => Branch::Plain,
    }
}

/// `RawDecode.decodesViaMosaic`, which the settings' migration shares with the decode.
pub fn decodes_via_mosaic(path: &Path) -> bool {
    route(path) == Branch::Mosaic
}

/// `RawDecode.isLowPrecision`: 8 bits per channel or fewer, a warning and never a refusal, since
/// the density inversion bands low-code shadows. A RAW name is never low.
pub fn is_low_precision(path: &Path) -> bool {
    if container::probe(path) == Some(container::Kind::LinearRgb)
        && let Some(bits) = container::bits_per_sample(path)
    {
        return bits <= 8;
    }
    !is_raw(path) && image_depth(path).is_some_and(|bits| bits <= 8)
}

/// The depth an image reader reports: a TIFF's directory, a PNG's header, 8 for a JPEG.
fn image_depth(path: &Path) -> Option<usize> {
    use std::io::Read;
    let mut head = [0u8; 26];
    let n = std::fs::File::open(path).ok()?.read(&mut head).ok()?;
    match &head[..n.min(8)] {
        [0xFF, 0xD8, 0xFF, ..] => Some(8),
        [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] if n >= 25 => Some(head[24] as usize),
        [b'I', b'I', ..] | [b'M', b'M', ..] => container::bits_per_sample(path),
        _ => None,
    }
}

/// Decodes `path`; `exposure_gain` is LibRaw's linear exposure shift, 1 for none. Safe from several
/// threads at once: every decode owns its LibRaw instance.
pub fn decode(path: &Path, exposure_gain: f32) -> Result<Linear, DecodeError> {
    decode_for(path, exposure_gain, 0).map(|(linear, _)| linear)
}

/// `decode` for a copy no longer than `side`: read at half size, a sensor pixel per 2x2 block with
/// no demosaic, when the sensor is twice as long; the factor back to the full frame beside it.
pub fn decode_for(path: &Path, exposure_gain: f32, side: usize) -> Result<(Linear, usize), DecodeError> {
    decode_in(path, exposure_gain, side, Colours::Matrix)
}

/// `decode` in LibRaw's own colours, the camera's rendition matrix left out: what it is measured on.
pub fn decode_native(path: &Path, exposure_gain: f32) -> Result<Linear, DecodeError> {
    decoded(path, exposure_gain, 0, Colours::Matrix, false).map(|(linear, _)| linear)
}

/// `decode_for` in the given colours; the sensor's own never take a rendition matrix.
pub fn decode_in(path: &Path, exposure_gain: f32, side: usize, colours: Colours) -> Result<(Linear, usize), DecodeError> {
    decoded(path, exposure_gain, side, colours, true)
}

fn decoded(path: &Path, exposure_gain: f32, side: usize, colours: Colours, matched: bool) -> Result<(Linear, usize), DecodeError> {
    // A whole decode is kept on disk, and one kept is read back in place of LibRaw's.
    let key = (side == 0).then(|| cache::key(path, &format!("{:08x}|{colours:?}|{matched}", exposure_gain.to_bits()))).flatten();
    if let Some(k) = key.as_deref().and_then(cache::read) {
        let pixels = linear::converted(&k.raw, k.width, (0, 0, k.width, k.height), k.flip, &k.matrix);
        let (width, height) = linear::upright(k.flip, (k.width, k.height));
        return Ok((Linear { width, height, pixels }, k.factor));
    }
    let c_path = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| DecodeError(-3))?;
    let mut s = Session(unsafe { std::mem::zeroed() });
    let min_side = c_int::try_from(side).unwrap_or(0);
    let rc = unsafe { oni_open(c_path.as_ptr(), exposure_gain, min_side, &mut s.0) };
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
        .then(|| converted_by(&cam_xyz, baseline(d.baseline_exposure), colours))
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
    let matrix = if matched && colours == Colours::Matrix { rendition::for_decode(&d.make, &d.model, d.dng != 0, conv.matrix) } else { conv.matrix };
    if let Some(key) = key.filter(|_| d.half == 0) {
        let head = cache::Kept { width: w_out, height: h_out, flip: d.flip, factor, matrix, raw: Vec::new() };
        cache::write(&key, &head, (y0..y0 + h_out).map(|row| &raw[(row * w + x0) * 3..(row * w + x0 + w_out) * 3]));
    }
    let pixels = linear::converted(raw, w, (x0, y0, w_out, h_out), d.flip, &matrix);
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
