//! Every kernel as a render-graph stage: oni-render's body behind a `fn pixel`, the region it reads
//! and the extent it writes. The parameters are the bytes the kernel's own `Params` lays out.

use oni_graph::{Rect, Stage};
use oni_render::{
    Affine, ColorMatrix, Curves, Detail, FlatField, Gaussian, LUT_SIZE, Orient, SaturationCurve, ScannerDensity,
    Spectrogram, Stage47, StrokeMask, TextureCoeff, Vignette, uniform, wgsl,
};
use std::sync::LazyLock;

/// The shared prelude, then each part's body, the uniform at the graph's binding 3, then `wrapper`.
fn compose(parts: &[&str], wrapper: &str) -> String {
    let mut out = String::from(wgsl::PRELUDE);
    for part in parts {
        out.push('\n');
        out.push_str(wgsl::body(part));
    }
    out.push_str("\n@group(0) @binding(3) var<uniform> P: Params;\n");
    out.push_str(wrapper);
    out
}

macro_rules! shader {
    ($name:ident, [$($part:expr),*], $wrapper:expr) => {
        static $name: LazyLock<String> = LazyLock::new(|| compose(&[$($part),*], $wrapper));
    };
}

const HEAD: [u32; 4] = [0; 4];

/// A kernel reading its input, and any auxiliary image, at the pixel it writes.
pub struct PerPixel {
    name: &'static str,
    shader: &'static LazyLock<String>,
    params: Vec<u8>,
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
shader!(
    CURVES,
    [wgsl::CURVES],
    r#"
fn curves_rgb_lut(k: u32) -> vec4<f32> { return load1(vec2<i32>(i32(k), 0)); }
fn curves_luma_lut(k: u32) -> vec4<f32> { return load2(vec2<i32>(i32(k), 0)); }
fn pixel(p: vec2<i32>) -> vec4<f32> { return curves_px(load(p)); }
"#
);
shader!(
    CUBE,
    [wgsl::COLOR_CUBE],
    r#"
fn cube_node(r: u32, g: u32, b: u32, n: u32) -> vec3<f32> { return load1(vec2<i32>(i32(g * n + r), i32(b))).rgb; }
fn pixel(p: vec2<i32>) -> vec4<f32> { return color_cube_px(load(p)); }
"#
);

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

/// A kernel reading whole tables beside its input: the curves' two rows, or a cube laid `n²` by `n`.
pub struct Tabled {
    inner: PerPixel,
    tables: Rect,
}

impl Stage for Tabled {
    fn name(&self) -> &'static str {
        self.inner.name
    }

    fn shader(&self) -> &'static str {
        self.inner.shader()
    }

    fn params(&self) -> Vec<u8> {
        self.inner.params()
    }

    fn aux_region(&self, _: usize, _: Rect) -> Rect {
        self.tables
    }
}

/// The curves, their RGB table the first auxiliary image and the luma table the second.
pub fn curves(k: &Curves) -> Tabled {
    let head = [0, 0, 0, LUT_SIZE as u32];
    let bent = [k.bent[0], k.bent[1], k.bent[2], 0.0];
    Tabled {
        inner: PerPixel { name: "curves", shader: &CURVES, params: uniform(head, &[bent]) },
        tables: Rect::new(0, 0, LUT_SIZE as u32, 1),
    }
}

/// `CIColorCube` of `n` nodes a side, the cube the auxiliary image, node (r, g, b) at (r + n·g, b).
pub fn color_cube(n: usize) -> Tabled {
    let head = [0, 0, 0, n as u32];
    Tabled {
        inner: PerPixel { name: "color-cube", shader: &CUBE, params: uniform(head, &[]) },
        tables: Rect::new(0, 0, (n * n) as u32, n as u32),
    }
}

shader!(
    DETAIL_ROWS,
    [wgsl::DETAIL_TAPS, wgsl::DETAIL_ROWS],
    r#"
fn detail_source(x: i32, y: i32) -> vec3<f32> { return load(vec2<i32>(x, y)).rgb; }
fn pixel(p: vec2<i32>) -> vec4<f32> { return detail_rows_px(p.x, p.y); }
"#
);
shader!(
    DETAIL_COLUMNS,
    [wgsl::DETAIL_TAPS, wgsl::DETAIL_COLUMNS],
    r#"
fn detail_row(x: i32, y: i32) -> vec4<f32> { return load1(vec2<i32>(x, y)); }
fn pixel(p: vec2<i32>) -> vec4<f32> { return detail_columns_px(load(p), p.x, p.y); }
"#
);

/// `detailRows`: the source widened by `disk` each side, read clamped to its edges.
pub struct DetailRows(pub Detail);

impl Stage for DetailRows {
    fn name(&self) -> &'static str {
        "detail-rows"
    }

    fn shader(&self) -> &'static str {
        DETAIL_ROWS.as_str()
    }

    fn params(&self) -> Vec<u8> {
        uniform(HEAD, &self.0.rows())
    }

    fn extent(&self, input: Rect) -> Rect {
        input.outset(self.0.disk as i32, 0)
    }

    fn input_region(&self, output: Rect) -> Rect {
        output.outset(self.0.reach[0].max(self.0.reach[1]) as i32, 0)
    }
}

/// `detailColumns`: the rows pass its auxiliary image, read `disk` across and `margin` down.
pub struct DetailColumns(pub Detail);

impl Stage for DetailColumns {
    fn name(&self) -> &'static str {
        "detail-columns"
    }

    fn shader(&self) -> &'static str {
        DETAIL_COLUMNS.as_str()
    }

    fn params(&self) -> Vec<u8> {
        uniform(HEAD, &self.0.rows())
    }

    fn aux_region(&self, _: usize, output: Rect) -> Rect {
        output.outset(self.0.disk as i32, self.0.margin() as i32)
    }
}

shader!(GAUSSIAN, [wgsl::DETAIL_TAPS, wgsl::GAUSSIAN], "fn pixel(p: vec2<i32>) -> vec4<f32> { return gaussian_px(p); }");
shader!(AFFINE, [wgsl::AFFINE], "fn pixel(p: vec2<i32>) -> vec4<f32> { return affine_px(p); }");
shader!(ORIENT, [wgsl::ORIENT], "fn pixel(p: vec2<i32>) -> vec4<f32> { return orient_px(p); }");
shader!(STROKE, [wgsl::STROKE_MASK], "fn pixel(p: vec2<i32>) -> vec4<f32> { return stroke_mask_px(p); }");

/// One axis of `CIGaussianBlur` on the input's own extent.
pub struct Blur(pub Gaussian);

impl Stage for Blur {
    fn name(&self) -> &'static str {
        "gaussian"
    }

    fn shader(&self) -> &'static str {
        GAUSSIAN.as_str()
    }

    fn params(&self) -> Vec<u8> {
        self.0.bytes()
    }

    fn input_region(&self, output: Rect) -> Rect {
        let r = self.0.reach();
        if self.0.vertical { output.outset(0, r) } else { output.outset(r, 0) }
    }
}

/// Both axes of `CIGaussianBlur`, edges repeated or transparent.
pub fn blurred(image: oni_graph::Image, sigma: f64, transparent: bool) -> oni_graph::Image {
    let k = |vertical| Gaussian { sigma: sigma as f32, vertical, transparent };
    image.then(Blur(k(false))).then(Blur(k(true)))
}

/// The bounding box of `rect`'s pixel area mapped through `m`, outset by `margin` whole pixels.
fn mapped(m: &[[f32; 3]; 2], rect: Rect, margin: i32) -> Rect {
    let corners = [
        (rect.x as f32, rect.y as f32),
        (rect.right() as f32, rect.y as f32),
        (rect.x as f32, rect.bottom() as f32),
        (rect.right() as f32, rect.bottom() as f32),
    ];
    let at = |(x, y): (f32, f32)| (m[0][0] * x + m[0][1] * y + m[0][2], m[1][0] * x + m[1][1] * y + m[1][2]);
    let points = corners.map(at);
    let (x0, x1) = points.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.0), b.max(p.0)));
    let (y0, y1) = points.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
    Rect::from_corners(x0.floor() as i32 - margin, y0.floor() as i32 - margin, x1.ceil() as i32 + margin, y1.ceil() as i32 + margin)
}

/// A bilinear affine resampling onto `extent`.
pub struct Resample {
    pub k: Affine,
    pub extent: Rect,
}

impl Stage for Resample {
    fn name(&self) -> &'static str {
        "affine"
    }

    fn shader(&self) -> &'static str {
        AFFINE.as_str()
    }

    fn params(&self) -> Vec<u8> {
        self.k.bytes()
    }

    fn extent(&self, _: Rect) -> Rect {
        self.extent
    }

    fn input_region(&self, output: Rect) -> Rect {
        mapped(&self.k.m, output, 1)
    }
}

/// Quarter turns and mirrors onto `extent`.
pub struct Oriented {
    pub k: Orient,
    pub extent: Rect,
}

impl Stage for Oriented {
    fn name(&self) -> &'static str {
        "orient"
    }

    fn shader(&self) -> &'static str {
        ORIENT.as_str()
    }

    fn params(&self) -> Vec<u8> {
        self.k.bytes()
    }

    fn extent(&self, _: Rect) -> Rect {
        self.extent
    }

    fn input_region(&self, output: Rect) -> Rect {
        let a = &self.k.a;
        let m = [[a[0] as f32, a[1] as f32, self.k.b[0] as f32], [a[2] as f32, a[3] as f32, self.k.b[1] as f32]];
        let last = Rect::new(output.x, output.y, output.w - 1, output.h - 1);
        let r = mapped(&m, last, 0);
        Rect::new(r.x, r.y, r.w + 1, r.h + 1)
    }
}

/// A stroke's hard mask on `extent`, its points the input row.
pub struct Stroke {
    pub k: StrokeMask,
    pub extent: Rect,
}

impl Stage for Stroke {
    fn name(&self) -> &'static str {
        "stroke-mask"
    }

    fn shader(&self) -> &'static str {
        STROKE.as_str()
    }

    fn params(&self) -> Vec<u8> {
        self.k.bytes()
    }

    fn extent(&self, _: Rect) -> Rect {
        self.extent
    }

    fn input_region(&self, _: Rect) -> Rect {
        Rect::new(0, 0, self.k.count as u32, 1)
    }
}

/// Plain stages around the kernels: a frame's edge, a placement, a ground and a patch's composite.
const OPAQUE_EDGE: &str = r#"
struct Params { inner: vec4<i32> }
@group(0) @binding(3) var<uniform> P: Params;
fn pixel(p: vec2<i32>) -> vec4<f32> { return load(clamp(p, P.inner.xy, P.inner.zw - vec2<i32>(1))); }
"#;

/// `opaqueEdged`: the outermost band of `extent` replaced by the line inside it.
pub struct OpaqueEdge {
    pub inner: Rect,
    pub extent: Rect,
}

impl Stage for OpaqueEdge {
    fn name(&self) -> &'static str {
        "opaque-edge"
    }

    fn shader(&self) -> &'static str {
        OPAQUE_EDGE
    }

    fn params(&self) -> Vec<u8> {
        bytemuck::bytes_of(&[self.inner.x, self.inner.y, self.inner.right(), self.inner.bottom()]).to_vec()
    }

    fn extent(&self, _: Rect) -> Rect {
        self.extent
    }

    fn input_region(&self, output: Rect) -> Rect {
        let i = self.inner;
        let cx = |v: i32| v.clamp(i.x, i.right() - 1);
        let cy = |v: i32| v.clamp(i.y, i.bottom() - 1);
        Rect::from_corners(cx(output.x), cy(output.y), cx(output.right() - 1) + 1, cy(output.bottom() - 1) + 1)
    }
}

const TRANSLATE: &str = r#"
struct Params { by: vec4<i32> }
@group(0) @binding(3) var<uniform> P: Params;
fn pixel(p: vec2<i32>) -> vec4<f32> { return load(p - P.by.xy); }
"#;

/// The image moved by whole pixels.
pub struct Translate(pub i32, pub i32);

impl Stage for Translate {
    fn name(&self) -> &'static str {
        "translate"
    }

    fn shader(&self) -> &'static str {
        TRANSLATE
    }

    fn params(&self) -> Vec<u8> {
        bytemuck::bytes_of(&[self.0, self.1, 0, 0]).to_vec()
    }

    fn extent(&self, input: Rect) -> Rect {
        Rect::new(input.x + self.0, input.y + self.1, input.w, input.h)
    }

    fn input_region(&self, output: Rect) -> Rect {
        Rect::new(output.x - self.0, output.y - self.1, output.w, output.h)
    }
}

const ON_GROUND: &str = r#"
fn pixel(p: vec2<i32>) -> vec4<f32> {
    let s = load(p);
    return vec4<f32>(s.rgb * s.a, 1.0);
}
"#;

/// `onGround`: premultiplied, then over opaque black.
pub struct OnGround;

impl Stage for OnGround {
    fn name(&self) -> &'static str {
        "on-ground"
    }

    fn shader(&self) -> &'static str {
        ON_GROUND
    }
}

shader!(
    BLEND,
    [wgsl::BLEND_WITH_MASK],
    r#"
fn pixel(p: vec2<i32>) -> vec4<f32> {
    let s = load(p);
    if (!inside1(p)) { return s; }
    let mask = select(vec4<f32>(0.0), load2(p), inside2(p));
    return blend_with_mask_px(load1(p), s, mask);
}
"#
);

/// `CIBlendWithMask` of a placed patch, its first auxiliary image, through its mask, the second.
pub struct Patched {
    pub patch: Rect,
    pub mask: Rect,
}

impl Stage for Patched {
    fn name(&self) -> &'static str {
        "blend-with-mask"
    }

    fn shader(&self) -> &'static str {
        BLEND.as_str()
    }

    fn params(&self) -> Vec<u8> {
        uniform(HEAD, &[])
    }

    // A region away from the patch still binds one of its pixels: the graph refuses an empty read.
    fn aux_region(&self, index: usize, output: Rect) -> Rect {
        let own = if index == 0 { self.patch } else { self.mask };
        let hit = output.intersect(&own);
        if hit.is_empty() { Rect::new(own.x, own.y, 1, 1) } else { hit }
    }
}

/// `RawDecode.rebuildingEdge`: a reduced decode darkens its two outer rows and columns, which are
/// taken from the third.
pub fn rebuilt_edge(image: oni_graph::Image) -> oni_graph::Image {
    let e = image.extent();
    if e.w <= 5 || e.h <= 5 {
        return image;
    }
    image.then(OpaqueEdge { inner: e.outset(-2, -2), extent: e })
}
