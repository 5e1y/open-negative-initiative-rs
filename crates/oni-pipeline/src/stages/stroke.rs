//! A stroke's hard mask, its points the input row.

use super::*;

shader!(STROKE, [wgsl::STROKE_MASK], "fn pixel(p: vec2<i32>) -> vec4<f32> { return stroke_mask_px(p); }");

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
