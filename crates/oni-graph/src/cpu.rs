//! The scalar references: slow, written to be read beside the kernels they judge.

use crate::Rect;
use crate::stages::Axis;

/// Pixels over an extent, row-major from its top-left corner.
#[derive(Clone, Debug, PartialEq)]
pub struct Plane {
    pub extent: Rect,
    pub pixels: Vec<[f32; 4]>,
}

impl Plane {
    pub fn new(extent: Rect, pixels: Vec<[f32; 4]>) -> Self {
        assert_eq!(pixels.len() as u64, extent.area(), "{extent:?} needs {} pixels", extent.area());
        Self { extent, pixels }
    }

    /// The pixel at image coordinate `(x, y)`, the nearest edge pixel outside: the kernels' `load`.
    pub fn load(&self, x: i32, y: i32) -> [f32; 4] {
        let e = self.extent;
        let cx = x.clamp(e.x, e.right() - 1) - e.x;
        let cy = y.clamp(e.y, e.bottom() - 1) - e.y;
        self.pixels[cy as usize * e.w as usize + cx as usize]
    }

    /// The pixels of `region`, which must lie inside the extent.
    pub fn cut(&self, region: Rect) -> Plane {
        assert!(self.extent.contains(&region), "{region:?} outside {:?}", self.extent);
        let pixels = (region.y..region.bottom())
            .flat_map(|y| (region.x..region.right()).map(move |x| (x, y)))
            .map(|(x, y)| self.load(x, y))
            .collect();
        Plane::new(region, pixels)
    }

    fn map(&self, extent: Rect, f: impl Fn(i32, i32) -> [f32; 4]) -> Plane {
        let pixels = (extent.y..extent.bottom())
            .flat_map(|y| (extent.x..extent.right()).map(move |x| (x, y)))
            .map(|(x, y)| f(x, y))
            .collect();
        Plane::new(extent, pixels)
    }
}

pub fn gain(input: &Plane, g: [f32; 4]) -> Plane {
    input.map(input.extent, |x, y| {
        let p = input.load(x, y);
        std::array::from_fn(|c| p[c] * g[c])
    })
}

/// Summed in the kernel's order, so the two part only by the GPU's rounding of the division.
pub fn box_blur_3(input: &Plane) -> Plane {
    input.map(input.extent, |x, y| {
        let mut sum = [0.0f32; 4];
        for dy in -1..=1 {
            for dx in -1..=1 {
                let p = input.load(x + dx, y + dy);
                for c in 0..4 {
                    sum[c] += p[c];
                }
            }
        }
        sum.map(|s| s / 9.0)
    })
}

pub fn crop(input: &Plane, rect: Rect) -> Plane {
    input.cut(input.extent.intersect(&rect))
}

pub fn lanczos3(t: f32) -> f32 {
    let a = t.abs();
    if a < 1e-6 {
        return 1.0;
    }
    if a >= 3.0 {
        return 0.0;
    }
    let x = std::f32::consts::PI * t;
    3.0 * x.sin() * (x / 3.0).sin() / (x * x)
}

/// `AnchoredLanczos` along `axis`, the GPU kernel's arithmetic in f32.
pub fn lanczos_anchored(input: &Plane, axis: Axis, scale: f64) -> Plane {
    lanczos_shifted(input, axis, scale, 0.0)
}

/// `AnchoredLanczos` with its leading edge `shift` output pixels in.
pub fn lanczos_shifted(input: &Plane, axis: Axis, scale: f64, shift: f64) -> Plane {
    let e = input.extent;
    let (origin, from) = if axis == Axis::X { (e.x, e.w) } else { (e.y, e.h) };
    let stage = crate::stages::AnchoredLanczos { axis, origin, from, scale, shift };
    let to = stage.to();
    let extent = if axis == Axis::X { Rect::new(0, e.y, to, e.h) } else { Rect::new(e.x, 0, e.w, to) };
    let narrow = (scale as f32).min(1.0);
    let radius = 3.0 / narrow;
    input.map(extent, |x, y| {
        let o = if axis == Axis::X { x } else { y };
        let k = if axis == Axis::Y { to as i32 - 1 - o } else { o } as f32;
        let up = (k + 0.5 - shift as f32) / scale as f32 - 0.5;
        let centre = origin as f32 + if axis == Axis::Y { from as f32 - 1.0 - up } else { up };
        let mut sum = [0.0f32; 4];
        let mut weights = 0.0f32;
        for i in (centre - radius).ceil() as i32..=(centre + radius).floor() as i32 {
            let w = lanczos3((i as f32 - centre) * narrow);
            let held = i >= origin && i < origin + from as i32;
            let p = if !held { [0.0; 4] } else if axis == Axis::X { input.load(i, y) } else { input.load(x, i) };
            for c in 0..4 {
                sum[c] += w * p[c];
            }
            weights += w;
        }
        sum.map(|s| s / weights)
    })
}

/// Lanczos-3 along `axis` to `to` pixels, in f32 and the kernel's own order of operations.
pub fn lanczos(input: &Plane, axis: Axis, to: u32) -> Plane {
    let e = input.extent;
    let (origin, from) = match axis {
        Axis::X => (e.x, e.w),
        Axis::Y => (e.y, e.h),
    };
    let extent = match axis {
        Axis::X => Rect::new(0, e.y, to, e.h),
        Axis::Y => Rect::new(e.x, 0, e.w, to),
    };
    let step = from as f32 / to as f32;
    let narrow = (to as f32 / from as f32).min(1.0);
    let radius = 3.0 / narrow;
    input.map(extent, |x, y| {
        let o = if axis == Axis::X { x } else { y };
        let centre = origin as f32 + (o as f32 + 0.5) * step - 0.5;
        let mut sum = [0.0f32; 4];
        let mut weights = 0.0f32;
        for i in (centre - radius).ceil() as i32..=(centre + radius).floor() as i32 {
            let w = lanczos3((i as f32 - centre) * narrow);
            let p = if axis == Axis::X { input.load(i, y) } else { input.load(x, i) };
            for c in 0..4 {
                sum[c] += w * p[c];
            }
            weights += w;
        }
        sum.map(|s| s / weights)
    })
}
