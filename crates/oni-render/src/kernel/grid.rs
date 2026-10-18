//! Pixels on the render graph's whole-pixel coordinates, which its built-ins' references read.

use super::*;

/// Pixels on whole-pixel coordinates of the render graph, top row first, origin at (`x`, `y`).
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub x: i32,
    pub y: i32,
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<V4>,
}

impl Grid {
    pub fn new(x: i32, y: i32, width: usize, height: usize, pixels: Vec<V4>) -> Self {
        assert_eq!(pixels.len(), width * height, "{width}x{height} needs {} pixels", width * height);
        Self { x, y, width, height, pixels }
    }

    pub fn inside(&self, q: [i32; 2]) -> bool {
        q[0] >= self.x && q[1] >= self.y && q[0] < self.x + self.width as i32 && q[1] < self.y + self.height as i32
    }

    /// The graph's `load`: the nearest pixel of the extent.
    pub fn load(&self, q: [i32; 2]) -> V4 {
        let cx = q[0].clamp(self.x, self.x + self.width as i32 - 1) - self.x;
        let cy = q[1].clamp(self.y, self.y + self.height as i32 - 1) - self.y;
        self.pixels[cy as usize * self.width + cx as usize]
    }

    pub(super) fn read(&self, q: [i32; 2], transparent: bool) -> V4 {
        if transparent && !self.inside(q) { [0.0; 4] } else { self.load(q) }
    }

    /// A grid of `width` by `height` at (`x`, `y`), each pixel `f` of its coordinate.
    pub fn generated(x: i32, y: i32, width: usize, height: usize, f: impl Fn([i32; 2]) -> V4) -> Self {
        let pixels = (0..height).flat_map(|r| (0..width).map(move |c| [x + c as i32, y + r as i32])).map(f).collect();
        Self { x, y, width, height, pixels }
    }
}

pub(super) fn mix4(a: V4, b: V4, t: f32) -> V4 {
    std::array::from_fn(|c| a[c] + (b[c] - a[c]) * t)
}
