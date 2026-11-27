//! A reduced decode's border rebuilt: scaling darkens its two outer rows and columns, the third's laid over them.

use super::*;

/// A reduced decode's two outer rows and columns taken from the third,
/// because scaling darkens the border. Follows a crop to `inner`, and reads only inside it.
pub(crate) struct Edge {
    extent: Rect,
    inner: Rect,
}

impl Stage for Edge {
    fn name(&self) -> &'static str {
        "edge"
    }

    fn shader(&self) -> &'static str {
        "fn pixel(p: vec2<i32>) -> vec4<f32> { return load(p); }"
    }

    fn params(&self) -> Vec<u8> {
        let (e, i) = (self.extent, self.inner);
        bytemuck::bytes_of(&[e.x, e.y, e.w as i32, e.h as i32, i.x, i.y, i.w as i32, i.h as i32]).to_vec()
    }

    fn extent(&self, _input: Rect) -> Rect {
        self.extent
    }

    fn input_region(&self, output: Rect) -> Rect {
        let i = self.inner;
        let x0 = output.x.clamp(i.x, i.right() - 1);
        let y0 = output.y.clamp(i.y, i.bottom() - 1);
        Rect::from_corners(x0, y0, output.right().clamp(x0 + 1, i.right()), output.bottom().clamp(y0 + 1, i.bottom()))
    }
}

pub(crate) fn rebuilt_edge(image: Image) -> Image {
    let e = image.extent();
    if e.w <= 5 || e.h <= 5 {
        return image;
    }
    let inner = Rect::new(e.x + 2, e.y + 2, e.w - 4, e.h - 4);
    image.then(Crop(inner)).then(Edge { extent: e, inner })
}
