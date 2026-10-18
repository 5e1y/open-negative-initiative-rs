//! Core Image's linear and radial gradients, generated over an extent.

use super::*;

/// `CILinearGradient`, in Core Image coordinates; colours in the working space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearGradient {
    pub point0: V2,
    pub point1: V2,
    pub color0: V4,
    pub color1: V4,
}

impl LinearGradient {
    pub fn rows(&self) -> Vec<V4> {
        vec![v22(self.point0, self.point1), self.color0, self.color1]
    }
}

/// `CIRadialGradient`, in Core Image coordinates; colours in the working space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadialGradient {
    pub center: V2,
    pub radius0: f32,
    pub radius1: f32,
    pub color0: V4,
    pub color1: V4,
}

impl RadialGradient {
    pub fn rows(&self) -> Vec<V4> {
        vec![[self.center[0], self.center[1], self.radius0, self.radius1], self.color0, self.color1]
    }
}

pub fn linear_gradient(at: V2, k: &LinearGradient) -> V4 {
    let axis = [k.point1[0] - k.point0[0], k.point1[1] - k.point0[1]];
    let along = (at[0] - k.point0[0]) * axis[0] + (at[1] - k.point0[1]) * axis[1];
    let t = clamp(along / (axis[0] * axis[0] + axis[1] * axis[1]), 0.0, 1.0);
    std::array::from_fn(|c| mix(k.color0[c], k.color1[c], t))
}

pub fn radial_gradient(at: V2, k: &RadialGradient) -> V4 {
    let d = [at[0] - k.center[0], at[1] - k.center[1]];
    let t = clamp(((d[0] * d[0] + d[1] * d[1]).sqrt() - k.radius0) / (k.radius1 - k.radius0), 0.0, 1.0);
    std::array::from_fn(|c| mix(k.color0[c], k.color1[c], t))
}

/// A generator rendered on an extent, each pixel read at its centre in Core Image's frame.
pub fn generated(origin: V2, width: usize, height: usize, f: impl Fn(V2) -> V4) -> Image {
    let blank = Image { origin, ..Image::new(width, height, vec![[0.0; 4]; width * height]) };
    image_map(&blank, |x, y, _| f(blank.ci_coord(x, y)))
}

const LINEAR_GRADIENT: Shader = shader!("linear_gradient");

const RADIAL_GRADIENT: Shader = shader!("radial_gradient");

impl Gpu {
    /// The gradient rendered on the extent of `width` by `height` at `origin`.
    pub fn linear_gradient(&self, origin: [f32; 2], width: usize, height: usize, k: &LinearGradient) -> Image {
        self.generated(LINEAR_GRADIENT, origin, width, height, &k.rows())
    }

    pub fn radial_gradient(&self, origin: [f32; 2], width: usize, height: usize, k: &RadialGradient) -> Image {
        self.generated(RADIAL_GRADIENT, origin, width, height, &k.rows())
    }

    fn generated(&self, shader: Shader, origin: [f32; 2], width: usize, height: usize, k: &[[f32; 4]]) -> Image {
        let shape = Image { width, height, origin, pixels: Vec::new() };
        self.banded(shader, |band| [vec![[band.origin[0], band.origin[1], 0.0, 0.0]], k.to_vec()].concat(), &shape, &[], &[], 0)
    }
}
