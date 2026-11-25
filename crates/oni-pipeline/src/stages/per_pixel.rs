//! Kernels that read their input, and any auxiliary image, at the pixel they write.

use super::*;
use oni_graph::Image;
use oni_render::{HeldBlack, Negative, NeutralNegative, Paper, Shown};

/// A kernel's composed shader; for one reading only its input, its source and its call on `v`, the
/// input at `p`, so `laid` may run several in one pass.
pub struct Kernel {
    pub(super) shader: LazyLock<String>,
    pub(super) alone: Option<(&'static str, &'static str)>,
}

/// A kernel reading its input, and any auxiliary image, at the pixel it writes.
pub struct PerPixel {
    pub(super) name: &'static str,
    pub(super) kernel: &'static Kernel,
    pub(super) params: Vec<u8>,
}

impl Stage for PerPixel {
    fn name(&self) -> &'static str {
        self.name
    }

    fn shader(&self) -> &'static str {
        self.kernel.shader.as_str()
    }

    fn params(&self) -> Vec<u8> {
        self.params.clone()
    }
}

kernel!(STAGE47, wgsl::STAGE47, call "stage47_px(v)");
kernel!(NEGATIVE, wgsl::NEGATIVE, call "negative_px(v)");
kernel!(NEGATIVE_COLOURED, wgsl::NEGATIVE_COLOURED, call "negative_px(v)");
kernel!(PAPER, wgsl::PAPER, call "paper_px(v)");
kernel!(INVERSION, wgsl::INVERSION, call "inversion_px(v)");
kernel!(EXPOSURE, wgsl::EXPOSURE_ADJUST, call "exposure_adjust_px(v)");
kernel!(RECIPROCAL, wgsl::RECIPROCAL, call "reciprocal_px(v)");
kernel!(NEUTRAL_NEGATIVE, wgsl::NEUTRAL_NEGATIVE, call "neutral_negative_px(v)");
kernel!(OUTPUT_CLIP, wgsl::OUTPUT_CLIP, call "output_clip_px(v)");
kernel!(FLAT_FIELD, [wgsl::FLAT_FIELD], "fn pixel(p: vec2<i32>) -> vec4<f32> { return flat_field_px(load(p), load1(p)); }");
kernel!(HELD_BLACK, [wgsl::HELD_BLACK], "fn pixel(p: vec2<i32>) -> vec4<f32> { return held_black_px(load(p), load1(p)); }");
kernel!(SATURATION, wgsl::SATURATION_CURVE, call "saturation_curve_px(v)");
kernel!(MATRIX, wgsl::COLOR_MATRIX, call "color_matrix_px(v)");
kernel!(TEXTURE, [wgsl::TEXTURE], "fn pixel(p: vec2<i32>) -> vec4<f32> { return texture_px(load(p), load1(p), load2(p)); }");
kernel!(SCANNER_ENCODE, wgsl::SCANNER_ENCODE, call "scanner_encode_px(v)");
kernel!(SCANNER_OUTPUT, [wgsl::SCANNER_OUTPUT], "fn pixel(p: vec2<i32>) -> vec4<f32> { return scanner_output_px(load(p), load1(p)); }");
kernel!(VIGNETTE, wgsl::VIGNETTE, call "vignette_px(v, vec2<f32>(p) + 0.5)");
kernel!(HALATION_SOURCE, wgsl::HALATION_SOURCE, call "halation_source_px(v)");
kernel!(HALATION, [wgsl::HALATION], "fn pixel(p: vec2<i32>) -> vec4<f32> { return halation_px(load(p), load1(p)); }");

/// The fused stages 4 to 7 past Lighting on the negative, which `converted` lays before them.
pub fn stage47(k: &Stage47) -> PerPixel {
    PerPixel { name: "stage47", kernel: &STAGE47, params: bytemuck::bytes_of(&k.after_negative().packed(0)).to_vec() }
}

/// Lighting and the colour tools on the negative, in the pass stage 4-7 reads.
pub fn negative(k: &Negative) -> PerPixel {
    // Lighting alone takes the pass without the colour tools, its bits as they were.
    let (name, kernel) = if k.colours() { ("negative-coloured", &NEGATIVE_COLOURED) } else { ("negative", &NEGATIVE) };
    PerPixel { name, kernel, params: uniform(k.head(), &k.rows()) }
}

/// The window's shares printed on the paper, in the pass past stage 4-7's window.
pub fn paper(k: &Paper) -> PerPixel {
    PerPixel { name: "paper", kernel: &PAPER, params: uniform(HEAD, &k.rows()) }
}

/// The window's shares shown with no paper, the pure inversion, in the pass past stage 4-7's window.
pub fn inversion() -> PerPixel {
    PerPixel { name: "inversion", kernel: &INVERSION, params: uniform(HEAD, &[]) }
}

/// Stages 4 to 7 on `image` as `cpu::stage47` runs them, in one pass: Lighting on the negative, the windows, then the
/// pure inversion, the paper, or a legacy print's curve in the windows' own kernel.
pub fn converted(image: Image, k: &Stage47) -> Image {
    let lit = k.negative.as_ref().map(negative);
    let shown = match &k.shown {
        Shown::Inversion => Some(inversion()),
        Shown::Paper(p) => Some(paper(p)),
        Shown::Curve => None,
    };
    laid(image, lit.into_iter().chain([stage47(k)]).chain(shown).collect())
}

/// `CIExposureAdjust` by `ev` stops.
pub fn exposure(ev: f32) -> PerPixel {
    PerPixel { name: "exposure-adjust", kernel: &EXPOSURE, params: uniform(HEAD, &[[ev, 0.0, 0.0, 0.0]]) }
}

/// A positive read as its own negative, its light inverted about `k`, held at `floor` below.
pub fn reciprocal(k: f32, floor: f32) -> PerPixel {
    PerPixel { name: "reciprocal", kernel: &RECIPROCAL, params: uniform(HEAD, &[[k, floor, 0.0, 0.0]]) }
}

/// The neutral negative: a scene exposed and developed as the screen's code of its light.
pub fn neutral_negative(k: &NeutralNegative) -> PerPixel {
    PerPixel { name: "neutral-negative", kernel: &NEUTRAL_NEGATIVE, params: uniform(HEAD, &k.rows()) }
}

pub fn output_clip() -> PerPixel {
    PerPixel { name: "output-clip", kernel: &OUTPUT_CLIP, params: uniform(HEAD, &[]) }
}

/// The division by the mask its first auxiliary image holds, laid on the frame's extent.
pub fn flat_field(k: &FlatField) -> PerPixel {
    PerPixel { name: "flat-field", kernel: &FLAT_FIELD, params: uniform(HEAD, &k.rows()) }
}

/// The lit slide laid back onto its rest, the auxiliary image, over the deep shadows.
pub fn held_black(k: &HeldBlack) -> PerPixel {
    PerPixel { name: "held-black", kernel: &HELD_BLACK, params: uniform(HEAD, &k.rows()) }
}

pub fn saturation(k: &SaturationCurve) -> PerPixel {
    PerPixel { name: "saturation-curve", kernel: &SATURATION, params: uniform(HEAD, &k.rows()) }
}

pub fn color_matrix(k: &ColorMatrix) -> PerPixel {
    PerPixel { name: "color-matrix", kernel: &MATRIX, params: uniform(HEAD, &k.rows()) }
}

/// `Pipeline.lumaOnly`: every channel the Rec. 2020 luminance, alpha kept.
pub fn luma_only() -> PerPixel {
    let w = oni_settings::axis::pipeline::LUMA_WEIGHTS;
    let row = [w[0], w[1], w[2], 0.0];
    color_matrix(&ColorMatrix { r_vector: row, g_vector: row, b_vector: row, ..ColorMatrix::default() })
}

/// The band: the narrow luma first, the blurred coefficients second, as auxiliary images.
pub fn texture(gain: f32) -> PerPixel {
    PerPixel { name: "texture", kernel: &TEXTURE, params: uniform(HEAD, &[[gain, 0.0, 0.0, 0.0]]) }
}

pub fn scanner_encode(root: f32) -> PerPixel {
    PerPixel { name: "scanner-encode", kernel: &SCANNER_ENCODE, params: uniform(HEAD, &[[root, 0.0, 0.0, 0.0]]) }
}

/// The warmth look's code as the auxiliary image, at `share`, its `lift` taken off.
pub fn scanner_output(share: f32, lift: f32) -> PerPixel {
    PerPixel { name: "scanner-output", kernel: &SCANNER_OUTPUT, params: uniform(HEAD, &[[share, lift, 0.0, 0.0]]) }
}

/// The vignette on `extent`, centred on it; coordinates are the graph's, which only the radius reads.
pub fn vignette(extent: Rect, stops: f32, hunt: f32) -> PerPixel {
    let k = Vignette {
        centre: [extent.x as f32 + extent.w as f32 / 2.0, extent.y as f32 + extent.h as f32 / 2.0],
        half_side: [extent.w as f32 / 2.0, extent.h as f32 / 2.0],
        dose: [stops, oni_render::VIGNETTE_MIDPOINT],
        hunt,
    };
    PerPixel { name: "vignette", kernel: &VIGNETTE, params: uniform(HEAD, &[vec![[0.0; 4]], k.rows()].concat()) }
}

/// A lens's optical vignetting on the scan of `extent`: the vignette circular in pixels, its radius 1 at
/// the corner, from its centre on, so a gain of `2^(−stops·r²)`.
pub fn optical_vignette(extent: Rect, stops: f32) -> PerPixel {
    let half = (f64::from(extent.w).hypot(f64::from(extent.h)) / 2.0 / std::f64::consts::SQRT_2) as f32;
    let k = Vignette {
        centre: [extent.x as f32 + extent.w as f32 / 2.0, extent.y as f32 + extent.h as f32 / 2.0],
        half_side: [half, half],
        dose: [stops, 0.0],
        hunt: 0.0,
    };
    PerPixel { name: "optical-vignette", kernel: &VIGNETTE, params: uniform(HEAD, &[vec![[0.0; 4]], k.rows()].concat()) }
}

/// The light each pixel of a film's layers' exposures sends into its glow.
pub fn halation_source(k: &Halation) -> PerPixel {
    PerPixel { name: "halation-source", kernel: &HALATION_SOURCE, params: uniform(HEAD, &k.rows()) }
}

/// The halation on a film's layers' exposures, their glow the auxiliary image.
pub fn halation(k: &Halation) -> PerPixel {
    PerPixel { name: "halation", kernel: &HALATION, params: uniform(HEAD, &k.rows()) }
}
