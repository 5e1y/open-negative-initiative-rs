//! The design system's `KeycapLook` as numbers, turned into a style and a frame, and the renders kept by
//! those numbers: oni-lab (interpreted Slint) and oni-app (compiled Slint) answer `Raster.keycap` with it.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::style::{KeycapStyle, Rgba};
use crate::surface::Frame;
use crate::{Image, render_in};

/// A press sweeps the depth continuously; 1/256 steps cannot be told apart and bound the renders.
pub const DEPTH_STEPS: f64 = 256.0;

/// `KeycapLook`'s fields, every length in points; the tunables at the large key's lengths, `factor` the variant's.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Look {
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    /// sRGB, straight alpha.
    pub albedo: [u8; 4],
    pub radius: f64,
    pub face_radius: f64,
    pub factor: f64,
    pub margin: f64,
    /// Pixels per point; the window's when 0.
    pub scale: f64,
    pub level: bool,
    pub shadow: bool,
    pub rim: bool,
    pub azimuth: f64,
    pub elevation: f64,
    pub ambient: f64,
    pub diffuse: f64,
    pub midtone: f64,
    pub wall_top: f64,
    pub wall_bottom: f64,
    pub wall_side: f64,
    pub rise: f64,
    pub dish: f64,
    pub seam_blur: f64,
    pub socket_rest: f64,
    pub socket_pressed: f64,
    pub socket_softness: f64,
    pub scale_pressed: f64,
    pub travel: f64,
    pub shadow_opacity: f64,
    pub shadow_blur: f64,
    pub shadow_y: f64,
    pub bottom_lift: f64,
}

impl Look {
    /// The Swift style the look asks for: the tunables at the large key's lengths, then the variant's factor.
    pub fn style(&self) -> KeycapStyle {
        let base = KeycapStyle {
            light_azimuth: self.azimuth,
            light_elevation: self.elevation,
            ambient: self.ambient,
            diffuse: self.diffuse,
            midtone_match: self.midtone,
            wall_top: self.wall_top,
            wall_bottom: self.wall_bottom,
            wall_left: self.wall_side,
            wall_right: self.wall_side,
            rise: self.rise,
            dish_depth: self.dish,
            seam_blur: self.seam_blur,
            socket_shadow_rest: self.socket_rest,
            socket_shadow_pressed: self.socket_pressed,
            socket_shadow_softness: self.socket_softness,
            shadow_opacity: self.shadow_opacity,
            shadow_blur: self.shadow_blur,
            shadow_y: self.shadow_y,
            bottom_lift: self.bottom_lift,
            scale_pressed: self.scale_pressed,
            travel: self.travel,
            ..KeycapStyle::hardened()
        };
        let mut style = base.scaled(self.factor);
        style.corner_radius = self.radius;
        style.face_radius = self.face_radius;
        let [r, g, b, a] = self.albedo;
        style.albedo = Rgba::from_u8(r, g, b, a);
        // `DSBarHandle`: seen head-on, equal walls; caught rather than pressed, no shadow and no rim.
        if self.level {
            style.wall_bottom = style.wall_top;
        }
        if !self.shadow {
            style.shadow_opacity = 0.0;
        }
        if !self.rim {
            style.socket_shadow_rest = 0.0;
            style.socket_shadow_pressed = 0.0;
        }
        style
    }

    /// The depth a render is made at: clamped, on the 1/256 grid.
    pub fn quantised_depth(&self) -> f64 {
        (self.depth.clamp(0.0, 1.0) * DEPTH_STEPS).round() / DEPTH_STEPS
    }

    /// Every number and flag that changes a render's pixels, at `scale`.
    fn key(&self, scale: f64) -> Vec<u64> {
        let numbers = [
            self.width, self.height, self.quantised_depth(), self.radius, self.face_radius, self.factor, self.margin,
            scale, self.azimuth, self.elevation, self.ambient, self.diffuse, self.midtone, self.wall_top,
            self.wall_bottom, self.wall_side, self.rise, self.dish, self.seam_blur, self.socket_rest,
            self.socket_pressed, self.socket_softness, self.scale_pressed, self.travel, self.shadow_opacity,
            self.shadow_blur, self.shadow_y, self.bottom_lift,
        ];
        let mut key: Vec<u64> = numbers.iter().map(|n| n.to_bits()).collect();
        key.push(u32::from_ne_bytes(self.albedo) as u64 | (self.level as u64) << 32 | (self.shadow as u64) << 33 | (self.rim as u64) << 34);
        key
    }
}

/// What the renders cost so far: how many, their time, and the bytes the cache holds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    pub renders: u64,
    pub hits: u64,
    pub time: Duration,
    pub bytes: usize,
    pub kept: usize,
}

/// The renders kept by their numbers, so a still key is drawn once and a pressed one once per depth
/// step; `T` is the toolkit's image. Past `budget` bytes it starts over: the shown images live on.
pub struct Cache<T> {
    kept: HashMap<Vec<u64>, (T, usize)>,
    budget: usize,
    pub stats: Stats,
}

impl<T: Clone> Cache<T> {
    pub fn new(budget: usize) -> Self {
        Self { kept: HashMap::new(), budget, stats: Stats::default() }
    }

    /// The key `look` describes at `scale` pixels a point (the look's own when it has one), `make`
    /// turning a fresh render into `T`; `None` for no size.
    pub fn get(&mut self, look: &Look, scale: f64, make: impl FnOnce(&Image) -> T) -> Option<T> {
        let scale = if look.scale > 0.0 { look.scale } else { scale };
        if look.width <= 0.0 || look.height <= 0.0 || scale <= 0.0 {
            return None;
        }
        let key = look.key(scale);
        if let Some((image, _)) = self.kept.get(&key) {
            self.stats.hits += 1;
            return Some(image.clone());
        }
        let start = Instant::now();
        let frame = Frame { width: look.width, height: look.height, scale, margin: look.margin };
        let drawn = render_in(&look.style(), frame, look.quantised_depth());
        let image = make(&drawn);
        self.stats.time += start.elapsed();
        self.stats.renders += 1;
        let bytes = drawn.pixels.len();
        if self.stats.bytes + bytes > self.budget {
            self.kept.clear();
            self.stats.bytes = 0;
        }
        self.stats.bytes += bytes;
        self.kept.insert(key, (image.clone(), bytes));
        self.stats.kept = self.kept.len();
        Some(image)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small() -> Look {
        Look {
            width: 97.0, height: 40.0, albedo: [0x31, 0x31, 0x31, 0xff], radius: 8.4, face_radius: 7.35,
            factor: 0.525, shadow: true, rim: true, azimuth: 100.0, elevation: 33.0, ambient: 0.25, diffuse: 1.0,
            midtone: 1.0, wall_top: 8.0, wall_bottom: 12.0, wall_side: 8.0, rise: 6.0, dish: 4.0, seam_blur: 3.0,
            socket_rest: 2.0, socket_pressed: 6.0, socket_softness: 0.5, scale_pressed: 0.95, travel: 1.0,
            shadow_blur: 8.0, shadow_y: 10.0, bottom_lift: 0.4, ..Look::default()
        }
    }

    #[test]
    fn a_still_key_renders_once_and_a_depth_step_again() {
        let mut cache = Cache::new(1 << 24);
        let look = small();
        cache.get(&look, 2.0, |i| i.pixels.len()).unwrap();
        cache.get(&look, 2.0, |i| i.pixels.len()).unwrap();
        assert_eq!((cache.stats.renders, cache.stats.hits), (1, 1));
        // Within one 1/256 step the render is the same; a whole step further it is not.
        cache.get(&Look { depth: 0.4 / DEPTH_STEPS, ..look }, 2.0, |_| 0).unwrap();
        assert_eq!(cache.stats.renders, 1);
        cache.get(&Look { depth: 1.0 / DEPTH_STEPS, ..look }, 2.0, |_| 0).unwrap();
        assert_eq!(cache.stats.renders, 2);
        // Twin: another scale is another render.
        cache.get(&look, 1.0, |_| 0).unwrap();
        assert_eq!(cache.stats.renders, 3);
    }

    #[test]
    fn the_budget_starts_the_cache_over() {
        let look = small();
        let one = crate::render_in(&look.style(), Frame { width: 97.0, height: 40.0, scale: 2.0, margin: 0.0 }, 0.0).pixels.len();
        let mut cache = Cache::new(one * 2);
        for i in 0..3 {
            cache.get(&Look { depth: i as f64 / 8.0, ..look }, 2.0, |_| ()).unwrap();
        }
        assert_eq!(cache.stats.kept, 1, "the third render starts it over");
        assert!(cache.stats.bytes <= one * 2);
        // Twin: a budget that holds them all keeps them all.
        let mut roomy = Cache::new(one * 3);
        for i in 0..3 {
            roomy.get(&Look { depth: i as f64 / 8.0, ..look }, 2.0, |_| ()).unwrap();
        }
        assert_eq!(roomy.stats.kept, 3);
    }

    #[test]
    fn no_size_draws_nothing() {
        let mut cache = Cache::new(1 << 20);
        assert!(cache.get(&Look { width: 0.0, ..small() }, 2.0, |_| ()).is_none());
        assert!(cache.get(&small(), 0.0, |_| ()).is_none());
        assert!(cache.get(&Look { scale: 1.0, ..small() }, 0.0, |_| ()).is_some(), "the look's own scale serves");
    }

    #[test]
    fn no_cast_shadow_draws_nothing_outside_the_key() {
        // A margin is room for the cast shadow alone: without it, every pixel past the key is clear.
        let look = Look { shadow_opacity: 0.0, ..small() };
        let frame = Frame { width: 97.0, height: 40.0, scale: 2.0, margin: 6.0 };
        let drawn = render_in(&look.style(), frame, 1.0);
        let (w, m) = (drawn.width as usize, 12);
        let outside = |x: usize, y: usize| x < m || y < m || x >= w - m || y >= drawn.height as usize - m;
        let alpha = |shadow: f64| {
            let style = Look { shadow_opacity: shadow, ..look }.style();
            let drawn = render_in(&style, frame, 0.0);
            (0..drawn.height as usize).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| outside(x, y))
                .map(|(x, y)| drawn.pixels[(y * w + x) * 4 + 3] as u32).max().unwrap()
        };
        assert_eq!(alpha(0.0), 0);
        assert_eq!(drawn.pixels.chunks(4).enumerate().filter(|(i, _)| outside(i % w, i / w)).map(|(_, p)| p[3]).max(), Some(0));
        // Twin: a cast shadow reaches past the key.
        assert!(alpha(0.5) > 0);
    }
}
