//! The stages that read around the pixel they write: the blur, the affine resampling, the orientation,
//! the lens warp.

use super::*;

const BLUR_PIXEL: &str = "fn pixel(p: vec2<i32>) -> vec4<f32> { return gaussian_px(p); }";
shader!(GAUSSIAN, [wgsl::DETAIL_TAPS, wgsl::GAUSSIAN], &format!("fn gaussian_pre(v: vec4<f32>) -> vec4<f32> {{ return v; }}\n{BLUR_PIXEL}"));
shader!(GAUSSIAN_LUMA, [wgsl::DETAIL_TAPS, wgsl::GAUSSIAN], &format!("fn gaussian_pre(v: vec4<f32>) -> vec4<f32> {{ let y = dot(v.rgb, LUMA); return vec4<f32>(y, y, y, v.a); }}\n{BLUR_PIXEL}"));
shader!(GAUSSIAN_MOMENTS, [wgsl::DETAIL_TAPS, wgsl::TEXTURE_MATH, wgsl::GAUSSIAN], &format!("fn gaussian_pre(v: vec4<f32>) -> vec4<f32> {{ return texture_moments_of(v); }}\n{BLUR_PIXEL}"));
shader!(GAUSSIAN_COEFF, [wgsl::DETAIL_TAPS, wgsl::TEXTURE_MATH, wgsl::GAUSSIAN], &format!("fn gaussian_pre(v: vec4<f32>) -> vec4<f32> {{ return texture_coeff_of(v, P.sigma_reach.z); }}\n{BLUR_PIXEL}"));
shader!(GRAIN, [wgsl::GRAIN, wgsl::GRAIN_LAID], "fn pixel(p: vec2<i32>) -> vec4<f32> { return grain_px(p); }");
shader!(GRAIN_FIELD, [wgsl::GRAIN], "fn pixel(p: vec2<i32>) -> vec4<f32> { return grain_field_px(p); }");
shader!(HALATION_LAID, [wgsl::HALATION_LAID], "fn pixel(p: vec2<i32>) -> vec4<f32> { return halation_laid_px(p); }");
shader!(AFFINE, [wgsl::AFFINE], "fn pixel(p: vec2<i32>) -> vec4<f32> { return affine_px(p); }");
shader!(ORIENT, [wgsl::ORIENT], "fn pixel(p: vec2<i32>) -> vec4<f32> { return orient_px(p); }");
shader!(WARP, [wgsl::WARP], "fn pixel(p: vec2<i32>) -> vec4<f32> { return warp_px(p); }");

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

/// A per-pixel step a blur applies to each tap it reads, so the step costs no pass of its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Before {
    /// `luma_only`.
    Luma,
    /// `texture_moments`.
    Moments,
    /// `texture_coeff` at its `relative` edge contrast.
    Coeff(f32),
}

/// The rows' axis of `CIGaussianBlur` reading its input through `Before`, edges repeated.
pub struct BlurOf(pub Gaussian, pub Before);

impl Stage for BlurOf {
    fn name(&self) -> &'static str {
        match self.1 {
            Before::Luma => "gaussian-luma",
            Before::Moments => "gaussian-moments",
            Before::Coeff(_) => "gaussian-coeff",
        }
    }

    fn shader(&self) -> &'static str {
        match self.1 {
            Before::Luma => GAUSSIAN_LUMA.as_str(),
            Before::Moments => GAUSSIAN_MOMENTS.as_str(),
            Before::Coeff(_) => GAUSSIAN_COEFF.as_str(),
        }
    }

    fn params(&self) -> Vec<u8> {
        let mut bytes = self.0.bytes();
        if let Before::Coeff(lane) = self.1 {
            bytes[8..12].copy_from_slice(&lane.to_ne_bytes());
        }
        bytes
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

/// Both axes of `CIGaussianBlur`, edges repeated, of `before` applied to the image.
pub fn blurred_of(image: oni_graph::Image, sigma: f64, before: Before) -> oni_graph::Image {
    let k = |vertical| Gaussian { sigma: sigma as f32, vertical, transparent: false };
    image.then(BlurOf(k(false), before)).then(Blur(k(true)))
}

/// The bounding box of `rect`'s pixel area mapped through `m`, outset by `margin` whole pixels.
pub(super) fn mapped(m: &[[f32; 3]; 2], rect: Rect, margin: i32) -> Rect {
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

/// Stage 0's turns and angle and the lens corrections in one read onto `extent`.
#[derive(Clone, Copy, Debug)]
pub struct Warped {
    pub k: Warp,
    pub extent: Rect,
}

/// The least and most of `P(u) = k0 + k1 u + k2 u² + k3 u³` on `[low, high]`: its ends and the
/// real roots of `P′` between them.
fn factor_span(k: [f32; 4], low: f64, high: f64) -> (f64, f64) {
    let k = k.map(f64::from);
    let p = |u: f64| k[0] + u * (k[1] + u * (k[2] + u * k[3]));
    let (a, b, c) = (3.0 * k[3], 2.0 * k[2], k[1]);
    let mut at = vec![low, high];
    if a == 0.0 {
        if b != 0.0 {
            at.push(-c / b);
        }
    } else if b * b - 4.0 * a * c >= 0.0 {
        let root = (b * b - 4.0 * a * c).sqrt();
        at.extend([(-b - root) / (2.0 * a), (-b + root) / (2.0 * a)]);
    }
    at.into_iter().filter(|u| (low..=high).contains(u)).map(p).fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)))
}

impl Stage for Warped {
    fn name(&self) -> &'static str {
        "warp"
    }

    fn shader(&self) -> &'static str {
        WARP.as_str()
    }

    fn params(&self) -> Vec<u8> {
        self.k.bytes()
    }

    fn extent(&self, _: Rect) -> Rect {
        self.extent
    }

    /// The output's box on the corrected scan, then every radius in it scaled by the channels' least
    /// and most factor there: the bilinear taps of every pixel lie inside.
    fn input_region(&self, output: Rect) -> Rect {
        let k = &self.k;
        let t = k.t.map(|r| r.map(f64::from));
        let at = |(x, y): (f64, f64)| (t[0][0] * x + t[0][1] * y + t[0][2], t[1][0] * x + t[1][1] * y + t[1][2]);
        let (x, y, r, b) = (f64::from(output.x), f64::from(output.y), f64::from(output.right()), f64::from(output.bottom()));
        let corners = [(x, y), (r, y), (x, b), (r, b)].map(at);
        let (x0, x1) = corners.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.0), b.max(p.0)));
        let (y0, y1) = corners.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
        let (cx, cy) = (f64::from(k.centre[0]), f64::from(k.centre[1]));
        let unit = f64::from(k.inv_unit2);
        let near = (cx.clamp(x0, x1) - cx).powi(2) + (cy.clamp(y0, y1) - cy).powi(2);
        let far = (cx - x0).abs().max((cx - x1).abs()).powi(2) + (cy - y0).abs().max((cy - y1).abs()).powi(2);
        let (least, most) = k.coefficients.iter().map(|c| factor_span(*c, near * unit, far * unit)).fold((f64::MAX, f64::MIN), |(lo, hi), (a, b)| (lo.min(a), hi.max(b)));
        let reached = [(x0, y0), (x1, y0), (x0, y1), (x1, y1)].into_iter().flat_map(|(px, py)| [least, most].map(|f| (cx + f * (px - cx), cy + f * (py - cy))));
        let (rx0, rx1, ry0, ry1) = reached.fold((f64::MAX, f64::MIN, f64::MAX, f64::MIN), |(a, b, c, d), p| (a.min(p.0), b.max(p.0), c.min(p.1), d.max(p.1)));
        Rect::from_corners(rx0.floor() as i32 - 1, ry0.floor() as i32 - 1, rx1.ceil() as i32 + 1, ry1.ceil() as i32 + 1)
    }
}

/// The grain, read at each pixel's absolute coordinate, so a region renders what the whole holds there; it reads the
/// film where the grains over each pixel are centred.
pub struct Grained(pub Grain);

impl Stage for Grained {
    fn name(&self) -> &'static str {
        "grain"
    }

    fn shader(&self) -> &'static str {
        GRAIN.as_str()
    }

    fn params(&self) -> Vec<u8> {
        uniform(self.0.head(), &self.0.rows())
    }

    fn input_region(&self, output: Rect) -> Rect {
        let [x, y] = self.0.render;
        let scale = (x[0] * y[1] - x[1] * y[0]).abs().sqrt();
        let margin = (self.0.reach() * scale).ceil() as i32 + 1;
        output.outset(margin, margin)
    }
}

/// The grain `k`, reading its layers' field as its one auxiliary input.
pub fn grain(k: &Grain) -> Grained {
    Grained(*k)
}

/// The layers' field of the grain `k`, over an input of the film's extent it never reads.
pub struct GrainField(pub Grain);

impl Stage for GrainField {
    fn name(&self) -> &'static str {
        "grain-field"
    }

    fn shader(&self) -> &'static str {
        GRAIN_FIELD.as_str()
    }

    fn params(&self) -> Vec<u8> {
        uniform(self.0.head(), &self.0.field_rows())
    }
}

/// The layers' field of the grain `k`.
pub fn grain_field(k: &Grain) -> GrainField {
    GrainField(*k)
}


/// The halation of `k` on a film's layers' exposures, its glow read off the film's plate of extent `plate`, where `map`
/// takes a pixel's centre.
pub struct HalationLaid {
    pub k: oni_render::Halation,
    pub map: [[f32; 3]; 2],
    pub plate: Rect,
}

impl Stage for HalationLaid {
    fn name(&self) -> &'static str {
        "halation-laid"
    }

    fn shader(&self) -> &'static str {
        HALATION_LAID.as_str()
    }

    fn params(&self) -> Vec<u8> {
        let [x, y] = self.map;
        uniform([0; 4], &[self.k.rows(), vec![[x[0], x[1], x[2], 0.0], [y[0], y[1], y[2], 0.0]]].concat())
    }

    fn aux_region(&self, _index: usize, output: Rect) -> Rect {
        mapped(&self.map, output, 2).nearest_in(&self.plate)
    }
}

/// The halation of `k`, its glow read off a plate of extent `plate` through `map`.
pub fn halation_laid(k: &oni_render::Halation, map: [[f32; 3]; 2], plate: Rect) -> HalationLaid {
    HalationLaid { k: *k, map, plate }
}
