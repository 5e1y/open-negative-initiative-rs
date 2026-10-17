//! What the graph tests share: a noise source, the judges, and a twin of the blur that lies.

#![allow(dead_code)]

use oni_graph::cpu::Plane;
use oni_graph::stages::Lanczos;
use oni_graph::{Rect, Stage};

/// Largest error accepted between a kernel and its reference, as |gpu - cpu| / (1 + |cpu|).
pub const TOLERANCE: f32 = 1e-5;

/// Deterministic noise in -0.2..1.6, unclamped as a scene-linear decode is: a flat picture hides every region bug.
pub fn noise(width: u32, height: u32, seed: u64) -> Vec<[f32; 4]> {
    let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    let mut next = move || {
        state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        ((state >> 40) as f32 / (1u64 << 24) as f32) * 1.8 - 0.2
    };
    (0..width * height).map(|_| [next(), next(), next(), 1.0]).collect()
}

/// Largest relative error between two buffers of the same size. A mismatch that computes to NaN
/// (a NaN or an infinity on one side) is infinite: `f32::max` drops a NaN, which would score it zero.
pub fn error(a: &[[f32; 4]], b: &[[f32; 4]]) -> f32 {
    assert_eq!(a.len(), b.len(), "buffers of different sizes");
    assert!(!a.is_empty(), "nothing to compare");
    a.iter()
        .zip(b)
        .flat_map(|(p, q)| {
            (0..4).map(move |c| {
                if p[c] == q[c] || (p[c].is_nan() && q[c].is_nan()) {
                    return 0.0;
                }
                let e = (p[c] - q[c]).abs() / (1.0 + q[c].abs());
                if e.is_nan() { f32::INFINITY } else { e }
            })
        })
        .fold(0.0, f32::max)
}

/// The pixels of `region` out of a row-major buffer covering `extent`.
pub fn cut(pixels: &[[f32; 4]], extent: Rect, region: Rect) -> Vec<[f32; 4]> {
    Plane::new(extent, pixels.to_vec()).cut(region).pixels
}

/// The 3 x 3 blur declaring no margin: its region renders must disagree with its full render.
pub struct LyingBlur;

impl Stage for LyingBlur {
    fn name(&self) -> &'static str {
        "lying-blur"
    }

    fn shader(&self) -> &'static str {
        r#"
fn pixel(p: vec2<i32>) -> vec4<f32> {
    var sum = vec4<f32>(0.0);
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            sum += load(p + vec2<i32>(dx, dy));
        }
    }
    return sum / 9.0;
}
"#
    }
}

/// A Lanczos pass declaring two pixels less reach each side than its taps need.
pub struct LyingLanczos(pub Lanczos);

impl Stage for LyingLanczos {
    fn name(&self) -> &'static str {
        "lying-lanczos"
    }

    fn shader(&self) -> &'static str {
        self.0.shader()
    }

    fn params(&self) -> Vec<u8> {
        self.0.params()
    }

    fn extent(&self, input: Rect) -> Rect {
        self.0.extent(input)
    }

    fn input_region(&self, output: Rect) -> Rect {
        let r = self.0.input_region(output);
        match self.0.axis {
            oni_graph::stages::Axis::X => Rect::from_corners(r.x + 2, r.y, r.right() - 2, r.bottom()),
            oni_graph::stages::Axis::Y => Rect::from_corners(r.x, r.y + 2, r.right(), r.bottom() - 2),
        }
    }
}
