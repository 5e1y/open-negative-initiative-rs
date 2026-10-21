//! The stages the graph itself needs (Lanczos for previews) and the set its tests drive.

use crate::{Rect, Stage};

/// Multiplies every channel: the per-pixel test stage.
pub struct Gain(pub [f32; 4]);

impl Stage for Gain {
    fn name(&self) -> &'static str {
        "gain"
    }

    fn shader(&self) -> &'static str {
        r#"
struct Params { gain: vec4<f32> }
@group(0) @binding(3) var<uniform> params: Params;
fn pixel(p: vec2<i32>) -> vec4<f32> {
    return load(p) * params.gain;
}
"#
    }

    fn params(&self) -> Vec<u8> {
        bytemuck::bytes_of(&self.0).to_vec()
    }
}

/// The 3 x 3 mean, edges replicated: the neighbourhood test stage, reading one pixel past its output.
pub struct BoxBlur3;

pub(crate) const BOX_BLUR_3: &str = r#"
fn pixel(p: vec2<i32>) -> vec4<f32> {
    var sum = vec4<f32>(0.0);
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            sum += load(p + vec2<i32>(dx, dy));
        }
    }
    return sum / 9.0;
}
"#;

impl Stage for BoxBlur3 {
    fn name(&self) -> &'static str {
        "box-blur-3"
    }

    fn shader(&self) -> &'static str {
        BOX_BLUR_3
    }

    fn input_region(&self, output: Rect) -> Rect {
        output.outset(1, 1)
    }
}

/// Keeps the pixels inside `0`, in the input's own coordinates, as Core Image's `cropped(to:)` does.
pub struct Crop(pub Rect);

impl Stage for Crop {
    fn name(&self) -> &'static str {
        "crop"
    }

    fn shader(&self) -> &'static str {
        "fn pixel(p: vec2<i32>) -> vec4<f32> { return load(p); }"
    }

    fn params(&self) -> Vec<u8> {
        bytemuck::bytes_of(&[self.0.x, self.0.y, self.0.w as i32, self.0.h as i32]).to_vec()
    }

    fn extent(&self, input: Rect) -> Rect {
        input.intersect(&self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

/// Lanczos-3 resampling along one axis, from `from` pixels starting at `origin` to `to` pixels
/// starting at 0; the kernel widens by `from / to` when reducing, so no frequency folds back.
#[derive(Clone, Copy, Debug)]
pub struct Lanczos {
    pub axis: Axis,
    pub origin: i32,
    pub from: u32,
    pub to: u32,
}

/// Lobes of the kernel, Core Image's `CILanczosScaleTransform` count.
pub const LANCZOS_LOBES: f64 = 3.0;

/// `CILanczosScaleTransform` along one axis: `ceil(from × scale)` pixels at the exact scale, anchored
/// at Core Image's origin, the left and the bottom, and clear past the input, so a partial last pixel
/// keeps only its share; a patch reduced otherwise lands up to half a pixel away, stretched. `shift`
/// sets the input's leading edge (left, or bottom) that many output pixels in, so it lands sub-pixel exact.
#[derive(Clone, Copy, Debug)]
pub struct AnchoredLanczos {
    pub axis: Axis,
    pub origin: i32,
    pub from: u32,
    pub scale: f64,
    pub shift: f64,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct AnchoredParams {
    origin: i32,
    from: u32,
    to: u32,
    axis: u32,
    scale: f32,
    pad: [f32; 3],
}

impl AnchoredLanczos {
    pub fn to(&self) -> u32 {
        // The float noise of `from × scale` must not add a whole pixel.
        ((f64::from(self.from) * self.scale + self.shift - 1e-9).ceil() as u32).max(1)
    }

    /// The input coordinate an output pixel's centre reads, in the axis's own counting.
    pub fn centre(&self, o: i32) -> f64 {
        let to = self.to() as i32;
        let k = if self.axis == Axis::Y { to - 1 - o } else { o };
        let up = (f64::from(k) + 0.5 - self.shift) / self.scale - 0.5;
        f64::from(self.origin) + if self.axis == Axis::Y { f64::from(self.from) - 1.0 - up } else { up }
    }

    fn reach(&self, a: i32, b: i32) -> (i32, i32) {
        let radius = LANCZOS_LOBES / self.scale.min(1.0);
        let (c0, c1) = (self.centre(a), self.centre(b - 1));
        ((c0.min(c1) - radius).ceil() as i32 - 1, (c0.max(c1) + radius).floor() as i32 + 2)
    }
}

impl Stage for AnchoredLanczos {
    fn name(&self) -> &'static str {
        "anchored-lanczos"
    }

    fn shader(&self) -> &'static str {
        r#"
struct Params { origin: i32, from_len: u32, to_len: u32, axis: u32, scale: f32, shift: f32, pad1: f32, pad2: f32 }
@group(0) @binding(3) var<uniform> params: Params;
const PI: f32 = 3.14159265358979;
fn lanczos3(t: f32) -> f32 {
    let a = abs(t);
    if (a < 1e-6) { return 1.0; }
    if (a >= 3.0) { return 0.0; }
    let x = PI * t;
    return 3.0 * sin(x) * sin(x / 3.0) / (x * x);
}
fn pixel(p: vec2<i32>) -> vec4<f32> {
    let narrow = min(params.scale, 1.0);
    let o = select(p.x, p.y, params.axis == 1u);
    let k = select(f32(o), f32(i32(params.to_len) - 1 - o), params.axis == 1u);
    let up = (k + 0.5 - params.shift) / params.scale - 0.5;
    let centre = f32(params.origin) + select(up, f32(params.from_len) - 1.0 - up, params.axis == 1u);
    let radius = 3.0 / narrow;
    var sum = vec4<f32>(0.0);
    var weights = 0.0;
    for (var i = i32(ceil(centre - radius)); i <= i32(floor(centre + radius)); i++) {
        let w = lanczos3((f32(i) - centre) * narrow);
        let q = select(vec2<i32>(i, p.y), vec2<i32>(p.x, i), params.axis == 1u);
        let held = i >= params.origin && i < params.origin + i32(params.from_len);
        sum += w * select(vec4<f32>(0.0), load(q), held);
        weights += w;
    }
    return sum / weights;
}
"#
    }

    fn params(&self) -> Vec<u8> {
        let axis = if self.axis == Axis::Y { 1 } else { 0 };
        bytemuck::bytes_of(&AnchoredParams { origin: self.origin, from: self.from, to: self.to(), axis, scale: self.scale as f32, pad: [self.shift as f32, 0.0, 0.0] }).to_vec()
    }

    fn extent(&self, input: Rect) -> Rect {
        let (origin, from) = if self.axis == Axis::X { (input.x, input.w) } else { (input.y, input.h) };
        assert!((origin, from) == (self.origin, self.from), "{self:?} built for another input than {input:?}");
        if self.axis == Axis::X { Rect::new(0, input.y, self.to(), input.h) } else { Rect::new(input.x, 0, input.w, self.to()) }
    }

    fn input_region(&self, output: Rect) -> Rect {
        if self.axis == Axis::X {
            let (lo, hi) = self.reach(output.x, output.right());
            Rect::from_corners(lo, output.y, hi, output.bottom())
        } else {
            let (lo, hi) = self.reach(output.y, output.bottom());
            Rect::from_corners(output.x, lo, output.right(), hi)
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LanczosParams {
    origin: i32,
    from: u32,
    to: u32,
    axis: u32,
}

impl Lanczos {
    /// Input pixels per output pixel, and the kernel's width factor (1 when enlarging).
    pub fn step(&self) -> (f64, f64) {
        let step = self.from as f64 / self.to as f64;
        (step, step.max(1.0))
    }

    /// The input range `[lo, hi)` read for outputs `[a, b)` along the axis.
    fn reach(&self, a: i32, b: i32) -> (i32, i32) {
        let (step, widen) = self.step();
        let radius = LANCZOS_LOBES * widen;
        let centre = |o: i32| self.origin as f64 + (o as f64 + 0.5) * step - 0.5;
        // One pixel of slack each side: the GPU rounds the centre in f32, a tap may move across an integer.
        let lo = (centre(a) - radius).ceil() as i32 - 1;
        let hi = (centre(b - 1) + radius).floor() as i32 + 2;
        (lo, hi)
    }
}

impl Stage for Lanczos {
    fn name(&self) -> &'static str {
        "lanczos"
    }

    fn shader(&self) -> &'static str {
        r#"
struct Params { origin: i32, from_len: u32, to_len: u32, axis: u32 }
@group(0) @binding(3) var<uniform> params: Params;
const PI: f32 = 3.14159265358979;
fn lanczos3(t: f32) -> f32 {
    let a = abs(t);
    if (a < 1e-6) { return 1.0; }
    if (a >= 3.0) { return 0.0; }
    let x = PI * t;
    return 3.0 * sin(x) * sin(x / 3.0) / (x * x);
}
fn pixel(p: vec2<i32>) -> vec4<f32> {
    let step = f32(params.from_len) / f32(params.to_len);
    let narrow = min(f32(params.to_len) / f32(params.from_len), 1.0);
    let o = select(p.x, p.y, params.axis == 1u);
    let centre = f32(params.origin) + (f32(o) + 0.5) * step - 0.5;
    let radius = 3.0 / narrow;
    var sum = vec4<f32>(0.0);
    var weights = 0.0;
    for (var i = i32(ceil(centre - radius)); i <= i32(floor(centre + radius)); i++) {
        let w = lanczos3((f32(i) - centre) * narrow);
        let q = select(vec2<i32>(i, p.y), vec2<i32>(p.x, i), params.axis == 1u);
        sum += w * load(q);
        weights += w;
    }
    return sum / weights;
}
"#
    }

    fn params(&self) -> Vec<u8> {
        let axis = match self.axis {
            Axis::X => 0,
            Axis::Y => 1,
        };
        bytemuck::bytes_of(&LanczosParams { origin: self.origin, from: self.from, to: self.to, axis }).to_vec()
    }

    /// Panics on an input other than the one the stage was built for: its taps would land elsewhere.
    fn extent(&self, input: Rect) -> Rect {
        let (origin, from) = match self.axis {
            Axis::X => (input.x, input.w),
            Axis::Y => (input.y, input.h),
        };
        assert!(
            (origin, from) == (self.origin, self.from) && self.to > 0,
            "{self:?} built for another input than {input:?}"
        );
        match self.axis {
            Axis::X => Rect::new(0, input.y, self.to, input.h),
            Axis::Y => Rect::new(input.x, 0, input.w, self.to),
        }
    }

    fn input_region(&self, output: Rect) -> Rect {
        match self.axis {
            Axis::X => {
                let (lo, hi) = self.reach(output.x, output.right());
                Rect::from_corners(lo, output.y, hi, output.bottom())
            }
            Axis::Y => {
                let (lo, hi) = self.reach(output.y, output.bottom());
                Rect::from_corners(output.x, lo, output.right(), hi)
            }
        }
    }
}
