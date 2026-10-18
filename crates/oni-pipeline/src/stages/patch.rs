//! A placed patch composited through its mask.

use super::*;

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
