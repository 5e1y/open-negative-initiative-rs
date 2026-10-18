//! The stages that read around the pixel they write: the blur, the affine resampling, the orientation.

use super::*;

shader!(GAUSSIAN, [wgsl::DETAIL_TAPS, wgsl::GAUSSIAN], "fn pixel(p: vec2<i32>) -> vec4<f32> { return gaussian_px(p); }");
shader!(AFFINE, [wgsl::AFFINE], "fn pixel(p: vec2<i32>) -> vec4<f32> { return affine_px(p); }");
shader!(ORIENT, [wgsl::ORIENT], "fn pixel(p: vec2<i32>) -> vec4<f32> { return orient_px(p); }");

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
