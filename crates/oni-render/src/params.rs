//! Each kernel's arguments, in the Metal kernel's order and under its names, and their uniform rows.
//! Every block opens on a `vec4<u32>` head (count, width, height, extra) then `vec4<f32>` rows.

use bytemuck::{Pod, Zeroable};

type V2 = [f32; 2];
type V3 = [f32; 3];
type V4 = [f32; 4];

fn v3(a: V3, w: f32) -> V4 {
    [a[0], a[1], a[2], w]
}

fn v22(a: V2, b: V2) -> V4 {
    [a[0], a[1], b[0], b[1]]
}

/// The uniform block a shader reads: the head, then the rows.
pub(crate) fn uniform(head: [u32; 4], rows: &[V4]) -> Vec<u8> {
    let mut out = bytemuck::bytes_of(&head).to_vec();
    out.extend_from_slice(bytemuck::cast_slice(rows));
    out
}

/// The fused kernel's arguments, named as the Metal kernel names them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stage47 {
    pub gain: [f32; 3],
    pub red: [f32; 3],
    pub green: [f32; 3],
    pub blue: [f32; 3],
    pub invert: f32,
    pub density: f32,
    pub pedestal: f32,
    pub log_guard: f32,
    pub window: f32,
    pub low_points: [f32; 3],
    pub high_points: [f32; 3],
    pub sep_r: [f32; 3],
    pub sep_g: [f32; 3],
    pub sep_b: [f32; 3],
    pub print_on: f32,
    pub print_stops: f32,
    pub print_contrast: f32,
    pub print_zones: [f32; 2],
    pub own_stops: [f32; 3],
    pub hunt: f32,
    pub base_stops: f32,
    pub base_contrast: f32,
    pub base_zones: [f32; 2],
    pub decompression: f32,
    pub balance_low: [f32; 3],
    pub balance_span: [f32; 3],
    pub film_share: f32,
    pub follow: f32,
}

impl Stage47 {
    /// A negative at rest: no print, identity separation, no decompression.
    pub fn rest() -> Self {
        Self {
            gain: [1.0; 3],
            red: [0.0, 2.0, 1.0],
            green: [0.0, 2.0, 1.0],
            blue: [0.0, 2.0, 1.0],
            invert: 1.0,
            density: 1.0,
            pedestal: 0.0,
            log_guard: 1e-6,
            window: 3.0,
            low_points: [0.25; 3],
            high_points: [0.75; 3],
            sep_r: [1.0, 0.0, 0.0],
            sep_g: [0.0, 1.0, 0.0],
            sep_b: [0.0, 0.0, 1.0],
            print_on: 0.0,
            print_stops: 0.0,
            print_contrast: 0.0,
            print_zones: [0.0; 2],
            own_stops: [0.0; 3],
            hunt: 0.0,
            base_stops: 0.0,
            base_contrast: 0.0,
            base_zones: [0.0; 2],
            decompression: 1.0,
            balance_low: [0.0; 3],
            balance_span: [1.0; 3],
            film_share: 0.0,
            follow: 1.0,
        }
    }

    /// The uniform block, every vector padded to four lanes as WGSL lays it out.
    pub fn packed(&self, count: u32) -> Packed {
        Packed {
            rows: [
                v3(self.gain, self.invert),
                v3(self.red, self.density),
                v3(self.green, self.pedestal),
                v3(self.blue, self.log_guard),
                v3(self.low_points, self.window),
                v3(self.high_points, self.print_on),
                v3(self.sep_r, self.print_stops),
                v3(self.sep_g, self.print_contrast),
                v3(self.sep_b, self.hunt),
                v3(self.own_stops, self.base_stops),
                v3(self.balance_low, self.base_contrast),
                v3(self.balance_span, self.decompression),
                v22(self.print_zones, self.base_zones),
                [self.film_share, self.follow, 0.0, 0.0],
            ],
            count: [count, 0, 0, 0],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Packed {
    rows: [[f32; 4]; 14],
    count: [u32; 4],
}

// MARK: - pipeline.metal

/// `flatField(s, f, amount, exponent, perChannel)`: the mask `f` is laid on the frame's own extent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlatField {
    pub amount: f32,
    pub exponent: V3,
    pub per_channel: f32,
}

impl FlatField {
    pub(crate) fn rows(&self) -> Vec<V4> {
        vec![[self.amount, self.per_channel, 0.0, 0.0], v3(self.exponent, 0.0)]
    }
}

// MARK: - curves.metal

/// The curves' table length, `Curve.lutSize`.
pub const LUT_SIZE: usize = 1024;

/// `curves(src, rgbLUT, lumaLUT, bent)`. `rgbLUT` packs linked, red, green, blue in R, G, B, A;
/// `lumaLUT` carries luma in R. `bent` flags the channels', the linked and the luma tables.
#[derive(Clone, Debug, PartialEq)]
pub struct Curves {
    pub rgb_lut: Vec<V4>,
    pub luma_lut: Vec<V4>,
    pub bent: V3,
}

// MARK: - chroma.metal

/// `saturationCurve(s, knots, lows, high, bends)`, as `SaturationCurve.arguments` packs them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SaturationCurve {
    pub knots: V3,
    pub lows: V4,
    pub high: f32,
    pub bends: V3,
}

impl SaturationCurve {
    pub(crate) fn rows(&self) -> Vec<V4> {
        vec![v3(self.knots, self.high), self.lows, v3(self.bends, 0.0)]
    }
}

/// `textureCoeff(moments, relative)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextureCoeff {
    pub relative: f32,
}

/// `texture(src, narrowLuma, coeff, gain)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Texture {
    pub gain: f32,
}

/// The arguments `detailRows` and `detailColumns` share, built as `Pipeline.applyDetail` builds them.
/// `.x` of `sigma` and `reach` is the chroma denoise, `.y` the sharpening; radii are pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detail {
    pub sigma: V2,
    pub reach: V2,
    pub disk: f32,
    pub force: f32,
    pub amount: f32,
    pub halo: f32,
}

/// `Pipeline.blurFloor`: a radius at or under it leaves its half of the stage out.
pub const BLUR_FLOOR: f64 = 0.1614;

/// `Pipeline.detailReach`: the taps each side a blur of deviation `sigma` reads, out to 3.5 deviations.
pub fn detail_reach(sigma: f64) -> i32 {
    (3.5 * sigma).ceil() as i32
}

impl Detail {
    /// `applyDetail`'s arguments from the pixel radius and force of the chroma denoise and the pixel
    /// radius, amount and halo of the sharpening; `None` where neither half acts.
    pub fn dosed(chroma: Option<(f64, f32)>, sharpen: Option<(f64, f32, f32)>) -> Option<Self> {
        let chroma = chroma.filter(|c| c.0 > BLUR_FLOOR);
        let sharpen = sharpen.filter(|s| s.0 > BLUR_FLOOR);
        if chroma.is_none() && sharpen.is_none() {
            return None;
        }
        let halo = sharpen.map_or(0.0, |s| s.2);
        let disk = if halo > 0.0 { sharpen.map_or(1.0, |s| s.0).max(1.0).floor() } else { 0.0 };
        Some(Self {
            sigma: [chroma.map_or(0.0, |c| c.0 as f32), sharpen.map_or(0.0, |s| s.0 as f32)],
            reach: [
                chroma.map_or(0, |c| detail_reach(c.0)) as f32,
                sharpen.map_or(0, |s| detail_reach(s.0)) as f32,
            ],
            disk: disk as f32,
            force: chroma.map_or(0.0, |c| c.1),
            amount: sharpen.map_or(0.0, |s| s.1),
            halo,
        })
    }

    /// How far the columns read the rows vertically: `max(reach, disk)`, the rows pass's vertical margin.
    pub fn margin(&self) -> usize {
        self.reach[0].max(self.reach[1]).max(self.disk) as usize
    }

    pub(crate) fn rows(&self) -> Vec<V4> {
        vec![v22(self.sigma, self.reach), [self.disk, self.force, self.amount, self.halo]]
    }
}

/// `spectrogram(src, hueLow, hueHigh, satLow, satHigh, lumaLow, lumaHigh)`: the eight bands' doses,
/// each already divided by its band's authority as `Spectrogram.packed` does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spectrogram {
    pub hue_low: V4,
    pub hue_high: V4,
    pub sat_low: V4,
    pub sat_high: V4,
    pub luma_low: V4,
    pub luma_high: V4,
}

impl Spectrogram {
    pub(crate) fn rows(&self) -> Vec<V4> {
        vec![self.hue_low, self.hue_high, self.sat_low, self.sat_high, self.luma_low, self.luma_high]
    }
}

/// `Vignette.midpoint`: flat inside this share of the half-diagonal.
pub const VIGNETTE_MIDPOINT: f32 = 0.3;

/// `vignette(s, centre, halfSide, dose, hunt)`, in Core Image coordinates; `dose` is (stops, midpoint).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vignette {
    pub centre: V2,
    pub half_side: V2,
    pub dose: V2,
    pub hunt: f32,
}

impl Vignette {
    /// `Pipeline.applyVignette` on an extent: centred, the half sides its own, hunt 0 in monochrome.
    pub fn on_extent(origin: V2, width: usize, height: usize, stops: f32, hunt: f32) -> Self {
        let (w, h) = (width as f32, height as f32);
        Self {
            centre: [origin[0] + w / 2.0, origin[1] + h / 2.0],
            half_side: [w / 2.0, h / 2.0],
            dose: [stops, VIGNETTE_MIDPOINT],
            hunt,
        }
    }

    pub(crate) fn rows(&self) -> Vec<V4> {
        vec![v22(self.centre, self.half_side), [self.dose[0], self.dose[1], self.hunt, 0.0]]
    }
}

// MARK: - gamut.metal

/// `screenGamutZebra(picture, shown, tint, period, duty, phase, tolerance)`; `tint` in the working space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenGamutZebra {
    pub tint: V4,
    pub period: f32,
    pub duty: f32,
    pub phase: f32,
    pub tolerance: f32,
}

impl ScreenGamutZebra {
    pub(crate) fn rows(&self) -> Vec<V4> {
        vec![self.tint, [self.period, self.duty, self.phase, self.tolerance]]
    }
}

// MARK: - scanner.metal

/// `scannerDensity(s, gain, pedestal, logGuard, median, sepR, sepG, sepB, scale, shift, reachedLow,
/// reachedHigh, low, span, invert, dmax)`, as `ScannerRender.apply` passes them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScannerDensity {
    pub gain: V3,
    pub pedestal: f32,
    pub log_guard: f32,
    pub median: V3,
    pub sep_r: V3,
    pub sep_g: V3,
    pub sep_b: V3,
    pub scale: f32,
    pub shift: V3,
    pub reached_low: V3,
    pub reached_high: V3,
    pub low: f32,
    pub span: f32,
    pub invert: f32,
    pub dmax: f32,
}

impl ScannerDensity {
    pub(crate) fn rows(&self) -> Vec<V4> {
        vec![
            v3(self.gain, self.pedestal),
            v3(self.median, self.log_guard),
            v3(self.sep_r, self.scale),
            v3(self.sep_g, self.low),
            v3(self.sep_b, self.span),
            v3(self.shift, self.invert),
            v3(self.reached_low, self.dmax),
            v3(self.reached_high, 0.0),
        ]
    }
}

/// `scannerOutput(code, warmth, warmthShare, lift, contrast)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScannerOutput {
    pub warmth_share: f32,
    pub lift: f32,
    pub contrast: f32,
}

/// `ScannerRender.lift`: the warmth looks' moves stored raised by it.
pub const SCANNER_LIFT: f32 = 0.5;

/// `scannerEncode(s, root)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScannerEncode {
    pub root: f32,
}

// MARK: - correctionSearch.metal

/// `correctionCost(src, destCentre, halfSize, windowOrigin, step, gridN, minRadius, extentMin,
/// extentMax, hole)`, every point in the source's Core Image coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CorrectionCost {
    pub dest_centre: V2,
    pub half_size: V2,
    pub window_origin: V2,
    pub step: f32,
    pub grid_n: f32,
    pub min_radius: f32,
    pub extent_min: V2,
    pub extent_max: V2,
    pub hole: V4,
}

impl CorrectionCost {
    pub(crate) fn rows(&self, source_origin: V2) -> Vec<V4> {
        vec![
            v22(self.dest_centre, self.half_size),
            [self.window_origin[0], self.window_origin[1], self.step, self.grid_n],
            v22(self.extent_min, self.extent_max),
            self.hole,
            [self.min_radius, source_origin[0], source_origin[1], 0.0],
        ]
    }
}

/// `correctionBiasField(patch, base, gradX, gradY, origin)`; `origin` in Core Image coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CorrectionBiasField {
    pub base: V3,
    pub grad_x: V3,
    pub grad_y: V3,
    pub origin: V2,
}

impl CorrectionBiasField {
    pub(crate) fn rows(&self, patch_origin: V2) -> Vec<V4> {
        vec![v3(self.base, 0.0), v3(self.grad_x, 0.0), v3(self.grad_y, 0.0), v22(self.origin, patch_origin)]
    }
}

// MARK: - Core Image built-ins

/// `CIColorMatrix`: each output channel a dot of the input with its vector, plus the bias.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorMatrix {
    pub r_vector: V4,
    pub g_vector: V4,
    pub b_vector: V4,
    pub a_vector: V4,
    pub bias_vector: V4,
}

impl Default for ColorMatrix {
    /// Core Image's defaults: the identity, no bias.
    fn default() -> Self {
        Self {
            r_vector: [1.0, 0.0, 0.0, 0.0],
            g_vector: [0.0, 1.0, 0.0, 0.0],
            b_vector: [0.0, 0.0, 1.0, 0.0],
            a_vector: [0.0, 0.0, 0.0, 1.0],
            bias_vector: [0.0; 4],
        }
    }
}

impl ColorMatrix {
    pub(crate) fn rows(&self) -> Vec<V4> {
        vec![self.r_vector, self.g_vector, self.b_vector, self.a_vector, self.bias_vector]
    }
}

/// `CIColorClamp`: every component held inside its pair.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorClamp {
    pub min_components: V4,
    pub max_components: V4,
}

impl Default for ColorClamp {
    fn default() -> Self {
        Self { min_components: [0.0; 4], max_components: [1.0; 4] }
    }
}

/// `CIExposureAdjust`: the colour times `2^ev`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExposureAdjust {
    pub ev: f32,
}

/// `CILinearGradient`, in Core Image coordinates; colours in the working space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearGradient {
    pub point0: V2,
    pub point1: V2,
    pub color0: V4,
    pub color1: V4,
}

impl LinearGradient {
    pub(crate) fn rows(&self) -> Vec<V4> {
        vec![v22(self.point0, self.point1), self.color0, self.color1]
    }
}

/// `CIRadialGradient`, in Core Image coordinates; colours in the working space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadialGradient {
    pub center: V2,
    pub radius0: f32,
    pub radius1: f32,
    pub color0: V4,
    pub color1: V4,
}

impl RadialGradient {
    pub(crate) fn rows(&self) -> Vec<V4> {
        vec![[self.center[0], self.center[1], self.radius0, self.radius1], self.color0, self.color1]
    }
}

/// `CIColorCube`: `cube_dimension` nodes a side, red running fastest, then green, then blue.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorCube {
    pub cube_dimension: usize,
    pub cube_data: Vec<V4>,
}

impl ColorCube {
    /// A cube from the little-endian `f32` RGBA bytes `inputCubeData` carries.
    pub fn from_bytes(cube_dimension: usize, bytes: &[u8]) -> Option<Self> {
        let n = cube_dimension * cube_dimension * cube_dimension;
        if cube_dimension < 2 || bytes.len() != n * 16 {
            return None;
        }
        let cube_data = bytes
            .chunks_exact(16)
            .map(|c| std::array::from_fn(|k| f32::from_le_bytes([c[4 * k], c[4 * k + 1], c[4 * k + 2], c[4 * k + 3]])))
            .collect();
        Some(Self { cube_dimension, cube_data })
    }
}

/// `CIAreaHistogram`: `count` bins over 0…1, each the share of the pixels it holds times `scale`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AreaHistogram {
    pub count: usize,
    pub scale: f32,
}

/// A rectangle of whole pixels in the buffer's own frame, top row first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}
