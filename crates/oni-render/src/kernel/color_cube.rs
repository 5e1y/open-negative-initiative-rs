//! Core Image's `CIColorCube`, interpolated by the kernel itself.

use super::*;

/// `CIColorCube`: `cube_dimension` nodes a side, red running fastest, then green, then blue.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorCube {
    pub cube_dimension: usize,
    pub cube_data: Vec<V4>,
}

impl ColorCube {
    /// A cube from the little-endian `f32` RGBA bytes `inputCubeData` carries.
    pub fn from_bytes(cube_dimension: usize, bytes: &[u8]) -> Option<Self> {
        let n = cube_dimension * cube_dimension * cube_dimension;
        if cube_dimension < 2 || bytes.len() != n * 16 {
            return None;
        }
        let cube_data = bytes
            .chunks_exact(16)
            .map(|c| std::array::from_fn(|k| f32::from_le_bytes([c[4 * k], c[4 * k + 1], c[4 * k + 2], c[4 * k + 3]])))
            .collect();
        Some(Self { cube_dimension, cube_data })
    }
}

pub fn color_cube(s: V4, k: &ColorCube) -> V4 {
    let n = k.cube_dimension;
    let node = |r: usize, g: usize, b: usize| rgb(k.cube_data[(b * n + g) * n + r]);
    let f = map(rgb(s), |_, v| clamp(v, 0.0, 1.0) * (n - 1) as f32);
    let lo = f.map(|v| (v.floor() as usize).min(n - 2));
    let t = map(f, |c, v| v - lo[c] as f32);
    let hi = lo.map(|v| v + 1);
    let lerp = |a: V3, b: V3, t: f32| map(a, |c, v| mix(v, b[c], t));
    let c00 = lerp(node(lo[0], lo[1], lo[2]), node(hi[0], lo[1], lo[2]), t[0]);
    let c10 = lerp(node(lo[0], hi[1], lo[2]), node(hi[0], hi[1], lo[2]), t[0]);
    let c01 = lerp(node(lo[0], lo[1], hi[2]), node(hi[0], lo[1], hi[2]), t[0]);
    let c11 = lerp(node(lo[0], hi[1], hi[2]), node(hi[0], hi[1], hi[2]), t[0]);
    let c = lerp(lerp(c00, c10, t[1]), lerp(c01, c11, t[1]), t[2]);
    with_alpha(map(c, |_, v| v.max(0.0)), s[3])
}

const COLOR_CUBE: Shader = shader!("color_cube");

impl Gpu {
    pub fn color_cube(&self, image: &Image, k: &ColorCube) -> Image {
        let d = k.cube_dimension;
        assert!(d >= 2 && k.cube_data.len() == d * d * d, "color cube: {} nodes for dimension {d}", k.cube_data.len());
        self.per_pixel(COLOR_CUBE, |_| Vec::new(), &[image], &[&k.cube_data], d)
    }
}
