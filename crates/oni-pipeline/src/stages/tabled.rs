//! Kernels that read whole tables beside their input: the curves and the colour cube.

use super::*;

shader!(
    CURVES,
    [wgsl::CURVES],
    r#"
fn curves_rgb_lut(k: u32) -> vec4<f32> { return load1(vec2<i32>(i32(k), 0)); }
fn curves_luma_lut(k: u32) -> vec4<f32> { return load2(vec2<i32>(i32(k), 0)); }
fn pixel(p: vec2<i32>) -> vec4<f32> { return curves_px(load(p)); }
"#
);
shader!(
    CUBE,
    [wgsl::COLOR_CUBE],
    r#"
fn cube_node(r: u32, g: u32, b: u32, n: u32) -> vec3<f32> { return load1(vec2<i32>(i32(g * n + r), i32(b))).rgb; }
fn pixel(p: vec2<i32>) -> vec4<f32> { return color_cube_px(load(p)); }
"#
);

/// A kernel reading whole tables beside its input: the curves' two rows, or a cube laid `n²` by `n`.
pub struct Tabled {
    inner: PerPixel,
    tables: Rect,
}

impl Stage for Tabled {
    fn name(&self) -> &'static str {
        self.inner.name
    }

    fn shader(&self) -> &'static str {
        self.inner.shader()
    }

    fn params(&self) -> Vec<u8> {
        self.inner.params()
    }

    fn aux_region(&self, _: usize, _: Rect) -> Rect {
        self.tables
    }
}

/// The curves, their RGB table the first auxiliary image and the luma table the second.
pub fn curves(k: &Curves) -> Tabled {
    let head = [0, 0, 0, LUT_SIZE as u32];
    let bent = [k.bent[0], k.bent[1], k.bent[2], 0.0];
    Tabled {
        inner: PerPixel { name: "curves", shader: &CURVES, params: uniform(head, &[bent]) },
        tables: Rect::new(0, 0, LUT_SIZE as u32, 1),
    }
}

/// `CIColorCube` of `n` nodes a side, the cube the auxiliary image, node (r, g, b) at (r + n·g, b).
pub fn color_cube(n: usize) -> Tabled {
    let head = [0, 0, 0, n as u32];
    Tabled {
        inner: PerPixel { name: "color-cube", shader: &CUBE, params: uniform(head, &[]) },
        tables: Rect::new(0, 0, (n * n) as u32, n as u32),
    }
}
