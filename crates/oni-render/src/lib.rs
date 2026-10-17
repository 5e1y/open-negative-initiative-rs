//! The render core: each GPU kernel beside the scalar reference that judges it.

pub mod cpu;
pub mod gpu;
mod params;

pub use params::*;

/// An image as Core Image holds one: rows top first, `origin` its extent's lower-left corner in
/// Core Image's frame, whose y runs up. Kernels that read coordinates read them in that frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub origin: [f32; 2],
    pub pixels: Vec<[f32; 4]>,
}

impl Image {
    /// An image at the origin; panics when `pixels` does not hold `width * height`.
    pub fn new(width: usize, height: usize, pixels: Vec<[f32; 4]>) -> Self {
        assert_eq!(pixels.len(), width * height, "{width}x{height} needs {} pixels", width * height);
        Self { width, height, origin: [0.0; 2], pixels }
    }

    pub fn at(&self, x: usize, y: usize) -> [f32; 4] {
        self.pixels[y * self.width + x]
    }

    /// Core Image's coordinate of the centre of pixel (`x`, `y`).
    pub fn ci_coord(&self, x: usize, y: usize) -> [f32; 2] {
        [self.origin[0] + x as f32 + 0.5, self.origin[1] + (self.height - 1 - y) as f32 + 0.5]
    }

    /// The same shape around other pixels.
    pub fn with_pixels(&self, pixels: Vec<[f32; 4]>) -> Self {
        Self { pixels, ..self.clone_shape() }
    }

    fn clone_shape(&self) -> Self {
        Self { width: self.width, height: self.height, origin: self.origin, pixels: Vec::new() }
    }
}
