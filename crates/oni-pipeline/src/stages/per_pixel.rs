//! Kernels that read their input, and any auxiliary image, at the pixel they write.

use super::*;

/// A kernel reading its input, and any auxiliary image, at the pixel it writes.
pub struct PerPixel {
    pub(super) name: &'static str,
    pub(super) shader: &'static LazyLock<String>,
    pub(super) params: Vec<u8>,
}

impl Stage for PerPixel {
    fn name(&self) -> &'static str {
        self.name
    }

    fn shader(&self) -> &'static str {
        self.shader.as_str()
    }

    fn params(&self) -> Vec<u8> {
        self.params.clone()
    }
}

shader!(STAGE47, [wgsl::STAGE47], "fn pixel(p: vec2<i32>) -> vec4<f32> { return stage47_px(load(p)); }");
shader!(EXPOSURE, [wgsl::EXPOSURE_ADJUST], "fn pixel(p: vec2<i32>) -> vec4<f32> { return exposure_adjust_px(load(p)); }");
shader!(OUTPUT_CLIP, [wgsl::OUTPUT_CLIP], "fn pixel(p: vec2<i32>) -> vec4<f32> { return output_clip_px(load(p)); }");
shader!(FLAT_FIELD, [wgsl::FLAT_FIELD], "fn pixel(p: vec2<i32>) -> vec4<f32> { return flat_field_px(load(p), load1(p)); }");
shader!(SATURATION, [wgsl::SATURATION_CURVE], "fn pixel(p: vec2<i32>) -> vec4<f32> { return saturation_curve_px(load(p)); }");
shader!(SPECTROGRAM, [wgsl::SPECTROGRAM], "fn pixel(p: vec2<i32>) -> vec4<f32> { return spectrogram_px(load(p)); }");
shader!(MATRIX, [wgsl::COLOR_MATRIX], "fn pixel(p: vec2<i32>) -> vec4<f32> { return color_matrix_px(load(p)); }");
shader!(MOMENTS, [wgsl::TEXTURE_MOMENTS], "fn pixel(p: vec2<i32>) -> vec4<f32> { return texture_moments_px(load(p)); }");
shader!(COEFF, [wgsl::TEXTURE_COEFF], "fn pixel(p: vec2<i32>) -> vec4<f32> { return texture_coeff_px(load(p)); }");
shader!(TEXTURE, [wgsl::TEXTURE], "fn pixel(p: vec2<i32>) -> vec4<f32> { return texture_px(load(p), load1(p), load2(p)); }");
shader!(SCANNER_DENSITY, [wgsl::SCANNER_DENSITY], "fn pixel(p: vec2<i32>) -> vec4<f32> { return scanner_density_px(load(p)); }");
shader!(SCANNER_ENCODE, [wgsl::SCANNER_ENCODE], "fn pixel(p: vec2<i32>) -> vec4<f32> { return scanner_encode_px(load(p)); }");
shader!(SCANNER_OUTPUT, [wgsl::SCANNER_OUTPUT], "fn pixel(p: vec2<i32>) -> vec4<f32> { return scanner_output_px(load(p), load1(p)); }");
shader!(VIGNETTE, [wgsl::VIGNETTE], "fn pixel(p: vec2<i32>) -> vec4<f32> { return vignette_px(load(p), vec2<f32>(p) + 0.5); }");

/// The fused stages 4 to 7.
pub fn stage47(k: &Stage47) -> PerPixel {
    PerPixel { name: "stage47", shader: &STAGE47, params: bytemuck::bytes_of(&k.packed(0)).to_vec() }
}

/// `CIExposureAdjust` by `ev` stops.
pub fn exposure(ev: f32) -> PerPixel {
    PerPixel { name: "exposure-adjust", shader: &EXPOSURE, params: uniform(HEAD, &[[ev, 0.0, 0.0, 0.0]]) }
}

pub fn output_clip() -> PerPixel {
    PerPixel { name: "output-clip", shader: &OUTPUT_CLIP, params: uniform(HEAD, &[]) }
}

/// The division by the mask its first auxiliary image holds, laid on the frame's extent.
pub fn flat_field(k: &FlatField) -> PerPixel {
    PerPixel { name: "flat-field", shader: &FLAT_FIELD, params: uniform(HEAD, &k.rows()) }
}

pub fn saturation(k: &SaturationCurve) -> PerPixel {
    PerPixel { name: "saturation-curve", shader: &SATURATION, params: uniform(HEAD, &k.rows()) }
}

pub fn spectrogram(k: &Spectrogram) -> PerPixel {
    PerPixel { name: "spectrogram", shader: &SPECTROGRAM, params: uniform(HEAD, &k.rows()) }
}

pub fn color_matrix(k: &ColorMatrix) -> PerPixel {
    PerPixel { name: "color-matrix", shader: &MATRIX, params: uniform(HEAD, &k.rows()) }
}

/// `Pipeline.lumaOnly`: every channel the Rec. 2020 luminance, alpha kept.
pub fn luma_only() -> PerPixel {
    let w = oni_settings::axis::pipeline::LUMA_WEIGHTS;
    let row = [w[0], w[1], w[2], 0.0];
    color_matrix(&ColorMatrix { r_vector: row, g_vector: row, b_vector: row, ..ColorMatrix::default() })
}

pub fn texture_moments() -> PerPixel {
    PerPixel { name: "texture-moments", shader: &MOMENTS, params: uniform(HEAD, &[]) }
}

pub fn texture_coeff(k: &TextureCoeff) -> PerPixel {
    PerPixel { name: "texture-coeff", shader: &COEFF, params: uniform(HEAD, &[[k.relative, 0.0, 0.0, 0.0]]) }
}

/// The band: the narrow luma first, the blurred coefficients second, as auxiliary images.
pub fn texture(gain: f32) -> PerPixel {
    PerPixel { name: "texture", shader: &TEXTURE, params: uniform(HEAD, &[[gain, 0.0, 0.0, 0.0]]) }
}

pub fn scanner_density(k: &ScannerDensity) -> PerPixel {
    PerPixel { name: "scanner-density", shader: &SCANNER_DENSITY, params: uniform(HEAD, &k.rows()) }
}

pub fn scanner_encode(root: f32) -> PerPixel {
    PerPixel { name: "scanner-encode", shader: &SCANNER_ENCODE, params: uniform(HEAD, &[[root, 0.0, 0.0, 0.0]]) }
}

/// The warmth look's code as the auxiliary image, at `share`, its `lift` taken off, then `contrast`.
pub fn scanner_output(share: f32, lift: f32, contrast: f32) -> PerPixel {
    PerPixel { name: "scanner-output", shader: &SCANNER_OUTPUT, params: uniform(HEAD, &[[share, lift, contrast, 0.0]]) }
}

/// The vignette on `extent`, centred on it; coordinates are the graph's, which only the radius reads.
pub fn vignette(extent: Rect, stops: f32, hunt: f32) -> PerPixel {
    let k = Vignette {
        centre: [extent.x as f32 + extent.w as f32 / 2.0, extent.y as f32 + extent.h as f32 / 2.0],
        half_side: [extent.w as f32 / 2.0, extent.h as f32 / 2.0],
        dose: [stops, oni_render::VIGNETTE_MIDPOINT],
        hunt,
    };
    PerPixel { name: "vignette", shader: &VIGNETTE, params: uniform(HEAD, &[vec![[0.0; 4]], k.rows()].concat()) }
}
