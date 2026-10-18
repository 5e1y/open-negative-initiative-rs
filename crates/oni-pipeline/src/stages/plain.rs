//! Plain stages around the kernels: a frame's opaque edge, a placement by whole pixels, a ground.

use super::*;

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
