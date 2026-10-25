//! Swift's `Frustum`: the four flat walls and the dished face, each shaded by one Lambert light.

use crate::style::{KeycapStyle, Rgba};

pub type Vec3 = (f64, f64, f64);

/// Seven stops read as a curve: at five a seam shows on a magnified face.
pub const DISH_STOPS: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wall {
    Top,
    Bottom,
    Left,
    Right,
}

impl Wall {
    pub const ALL: [Wall; 4] = [Wall::Top, Wall::Bottom, Wall::Left, Wall::Right];

    /// A wall leaning out by `run` over `rise`; y grows downwards, as on screen.
    pub fn normal(self, run: f64, rise: f64) -> Vec3 {
        let length = (run * run + rise * rise).sqrt().max(1e-9);
        let (s, c) = (rise / length, run / length);
        match self {
            Wall::Top => (0.0, -s, c),
            Wall::Bottom => (0.0, s, c),
            Wall::Left => (-s, 0.0, c),
            Wall::Right => (s, 0.0, c),
        }
    }

    pub fn run(self, style: &KeycapStyle) -> f64 {
        match self {
            Wall::Top => style.wall_top,
            Wall::Bottom => style.wall_bottom,
            Wall::Left => style.wall_left,
            Wall::Right => style.wall_right,
        }
    }

    /// The quad between the base's square corners and the face's: the four tile the silhouette.
    pub fn corners(self, base: [f64; 4], face: [f64; 4]) -> [(f64, f64); 4] {
        let ([bx0, by0, bx1, by1], [fx0, fy0, fx1, fy1]) = (base, face);
        match self {
            Wall::Top => [(bx0, by0), (bx1, by0), (fx1, fy0), (fx0, fy0)],
            Wall::Bottom => [(bx0, by1), (bx1, by1), (fx1, fy1), (fx0, fy1)],
            Wall::Left => [(bx0, by0), (bx0, by1), (fx0, fy1), (fx0, fy0)],
            Wall::Right => [(bx1, by0), (bx1, by1), (fx1, fy1), (fx1, fy0)],
        }
    }
}

/// The scoop's normal at `t`, 0 at the face's top edge to 1 at its bottom.
pub fn dish_normal(t: f64, dish_depth: f64, face_height: f64) -> Vec3 {
    if face_height <= 0.0 {
        return (0.0, 0.0, 1.0);
    }
    let slope = 4.0 * dish_depth * (2.0 * t - 1.0) / face_height;
    let length = (1.0 + slope * slope).sqrt();
    (0.0, -slope / length, 1.0 / length)
}

#[derive(Clone, Copy, Debug)]
pub struct Light {
    pub albedo: Rgba,
    pub ambient: f64,
    pub diffuse: f64,
    pub azimuth: f64,
    pub elevation: f64,
    pub midtone_match: f64,
}

impl Light {
    pub fn new(style: &KeycapStyle) -> Self {
        Self {
            albedo: style.albedo,
            ambient: style.ambient,
            diffuse: style.diffuse,
            azimuth: style.light_azimuth,
            elevation: style.light_elevation,
            midtone_match: style.midtone_match,
        }
    }

    pub fn direction(&self) -> Vec3 {
        let (e, a) = (self.elevation.to_radians(), self.azimuth.to_radians());
        (e.cos() * a.cos(), -e.cos() * a.sin(), e.sin())
    }

    /// 1 at `midtone_match` 0, the flat face's shade at 1: one exposure, not a mix of two.
    pub fn normaliser(&self) -> f64 {
        let flat = self.ambient + self.diffuse * self.direction().2.max(0.0);
        1.0 + (flat - 1.0) * self.midtone_match.clamp(0.0, 1.0)
    }

    pub fn lambert(&self, n: Vec3) -> f64 {
        let l = self.direction();
        (n.0 * l.0 + n.1 * l.1 + n.2 * l.2).max(0.0)
    }

    /// Unclamped: a shade above 1 is a highlight, clipped per channel by `Rgba::shaded`.
    pub fn shade(&self, n: Vec3) -> f64 {
        (self.ambient + self.diffuse * self.lambert(n)) / self.normaliser().max(1e-6)
    }

    pub fn colour(&self, n: Vec3) -> Rgba {
        self.albedo.shaded(self.shade(n))
    }

    /// The face's seven gradient stops, top to bottom, evenly placed.
    pub fn face_stops(&self, style: &KeycapStyle, face_height: f64) -> [Rgba; DISH_STOPS] {
        std::array::from_fn(|i| {
            let t = i as f64 / (DISH_STOPS - 1) as f64;
            self.colour(dish_normal(t, style.dish_depth, face_height))
        })
    }
}
