//! Swift's `KeycapChrome` and `KeycapSurface`, drawn as SwiftUI draws them: the bezel, the walls
//! blurred at `seam_blur`, the dished face, the socket's blurred rim, each casting its own shadow.

use tiny_skia::{FillRule, Mask, Path, Stroke, Transform};

use crate::blur::{Plane, gaussian};
use crate::light::{DISH_STOPS, Light, Wall};
use crate::shape::rounded;
use crate::style::{KeycapStyle, Rgba};

/// SwiftUI's `.blur(radius:)` and `.shadow(radius:)` as a Gaussian's deviation: 0.9 radius each,
/// fitted on the Swift bench's own sheet (`reference::sweep`), where 0.5 doubles the error.
pub const BLUR_SIGMA_PER_RADIUS: f64 = 0.9;
pub const SHADOW_SIGMA_PER_RADIUS: f64 = 0.9;

/// Where a key is drawn: its size in points, the pixels per point, and the room kept around it.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub width: f64,
    pub height: f64,
    pub scale: f64,
    pub margin: f64,
}

impl Frame {
    pub fn pixels(&self) -> (usize, usize) {
        let w = ((self.width + 2.0 * self.margin) * self.scale).ceil().max(1.0) as usize;
        let h = ((self.height + 2.0 * self.margin) * self.scale).ceil().max(1.0) as usize;
        (w, h)
    }
}

/// The two blur conversions, apart so a check can show they are measured.
#[derive(Clone, Copy, Debug)]
pub struct Sigmas {
    pub blur: f64,
    pub shadow: f64,
}

pub const SIGMAS: Sigmas = Sigmas { blur: BLUR_SIGMA_PER_RADIUS, shadow: SHADOW_SIGMA_PER_RADIUS };

/// The whole key as premultiplied sRGB floats.
pub fn compose(style: &KeycapStyle, frame: Frame, depth: f64) -> Plane {
    compose_with(style, frame, depth, None, SIGMAS)
}

/// As `compose`; `backing` lays a colour under the walls inside the cap, as the bench's sheet has.
pub fn compose_with(style: &KeycapStyle, frame: Frame, depth: f64, backing: Option<Rgba>, sigmas: Sigmas) -> Plane {
    let (pw, ph) = frame.pixels();
    let k = frame.scale as f32;
    let m = frame.margin as f32;
    let (w, h) = (frame.width as f32, frame.height as f32);
    let base = Transform::from_row(k, 0.0, 0.0, k, m * k, m * k);
    let s = style.scale_at(depth) as f32;
    // `scaleEffect` about the button's centre, then the contour's padding.
    let group = base.pre_concat(
        Transform::from_translate(w / 2.0, h / 2.0).pre_scale(s, s).pre_translate(-w / 2.0, -h / 2.0),
    );
    let c = style.contour_width as f32;
    let cap = group.pre_translate(c, c);
    let (cw, ch) = (w - 2.0 * c, h - 2.0 * c);
    let travel = style.travel_at(depth) as f32;
    let moved = cap.pre_translate(0.0, travel);
    let r = style.corner_radius as f32;

    let bezel = mask(pw, ph, rounded(0.0, 0.0, w, h, r + c, style.squircle), base);
    let cap_clip = mask(pw, ph, rounded(0.0, 0.0, cw, ch, r, style.squircle), cap);
    let light = Light::new(style);
    let mut items = vec![solid(&bezel, style.socket_color, None)];

    if let Some(colour) = backing {
        items.push(solid(&cap_clip, colour, Some(&bezel)));
    }

    // The walls, grown past the silhouette and under the face by the bleed so the blur thins neither edge.
    let face = style.face_rect(cw as f64, ch as f64);
    let b = 2.0 * style.seam_blur as f32;
    let outer = [-b, -b, cw + b, ch + b];
    let (fx, fy, fw, fh) = (face.x as f32, face.y as f32, face.w as f32, face.h as f32);
    let inner = [fx + b, fy + b, fx + fw - b, fy + fh - b];
    let mut walls = Plane::new(pw, ph, 4);
    for wall in Wall::ALL {
        let colour = light.colour(wall.normal(wall.run(style), style.rise));
        let quad = polygon(&wall.corners(outer.map(f64::from), inner.map(f64::from)));
        over(&mut walls, &mask(pw, ph, quad, moved), colour);
    }
    gaussian(&mut walls, (sigmas.blur * style.seam_blur) as f32 * s * k);
    clip(&mut walls, &cap_clip);
    clip(&mut walls, &bezel);
    items.push(walls);

    // The dished face: seven stops top to bottom over the face's own height.
    let face_mask = mask(pw, ph, rounded(fx, fy, fw, fh, style.face_radius as f32, style.squircle), moved);
    let stops = light.face_stops(style, face.h);
    let mut dish = Plane::new(pw, ph, 4);
    for y in 0..ph {
        // The pixel row back in the face's own coordinates, through the scale and the travel.
        let local = ((y as f32 + 0.5 - m * k) / k - h / 2.0) / s + h / 2.0 - c - travel;
        let colour = gradient(&stops, ((local - fy) / fh.max(1e-6)).clamp(0.0, 1.0));
        for x in 0..pw {
            let cov = face_mask.data()[y * pw + x] as f32 / 255.0;
            put(&mut dish, y * pw + x, colour, cov);
        }
    }
    clip(&mut dish, &cap_clip);
    clip(&mut dish, &bezel);
    items.push(dish);

    // The socket's rim: a stroke centred on the silhouette, blurred, laid over the cap outside its clip.
    let spread = style.socket_shadow_at(depth) as f32;
    let off = style.socket_shadow_offset as f32;
    if spread > 0.0 {
        // At the cap's origin, as Swift's `GeometryReader` lays it: an offset grows the ring right and down.
        let ring = rounded(0.0, 0.0, cw + 2.0 * off, ch + 2.0 * off, r + off, style.squircle)
            .and_then(|p| p.stroke(&Stroke { width: 2.0 * spread, ..Stroke::default() }, 1.0));
        let mut rim = solid(&mask(pw, ph, ring, cap), style.socket_color, None);
        gaussian(&mut rim, (sigmas.blur * style.socket_shadow_softness) as f32 * spread * s * k);
        clip(&mut rim, &bezel);
        items.push(rim);
    }

    // `.shadow` after the clip shadows each layer, not the flattened key: the face darkens the wall below it.
    let mut canvas = Plane::new(pw, ph, 4);
    let strength = (style.shadow_strength_at(depth) * style.shadow_color.a) as f32;
    let sigma = (sigmas.shadow * style.shadow_blur) as f32 * k;
    let dy = style.shadow_offset_at(depth) as f32 * k;
    for item in &items {
        if strength > 0.0 {
            shadow(&mut canvas, item, style.shadow_color, strength, sigma, dy);
        }
        composite(&mut canvas, item);
    }
    canvas
}

fn mask(w: usize, h: usize, path: Option<Path>, transform: Transform) -> Mask {
    let mut out = Mask::new(w as u32, h as u32).expect("a key's canvas is never empty");
    if let Some(path) = path {
        out.fill_path(&path, FillRule::Winding, true, transform);
    }
    out
}

fn polygon(points: &[(f64, f64); 4]) -> Option<Path> {
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(points[0].0 as f32, points[0].1 as f32);
    for p in &points[1..] {
        pb.line_to(p.0 as f32, p.1 as f32);
    }
    pb.close();
    pb.finish()
}

fn put(plane: &mut Plane, at: usize, colour: Rgba, cov: f32) {
    let a = colour.a as f32 * cov;
    let px = &mut plane.data[at * 4..at * 4 + 4];
    px[0] = colour.r as f32 * a;
    px[1] = colour.g as f32 * a;
    px[2] = colour.b as f32 * a;
    px[3] = a;
}

fn solid(coverage: &Mask, colour: Rgba, clip_to: Option<&Mask>) -> Plane {
    let (w, h) = (coverage.width() as usize, coverage.height() as usize);
    let mut plane = Plane::new(w, h, 4);
    for (i, &cov) in coverage.data().iter().enumerate() {
        let clip = clip_to.map_or(1.0, |m| m.data()[i] as f32 / 255.0);
        put(&mut plane, i, colour, cov as f32 / 255.0 * clip);
    }
    plane
}

/// Source-over of one flat colour through a coverage, as a Canvas fills each path in turn.
fn over(plane: &mut Plane, coverage: &Mask, colour: Rgba) {
    for (i, &cov) in coverage.data().iter().enumerate() {
        if cov == 0 {
            continue;
        }
        let a = colour.a as f32 * cov as f32 / 255.0;
        let src = [colour.r as f32 * a, colour.g as f32 * a, colour.b as f32 * a, a];
        let px = &mut plane.data[i * 4..i * 4 + 4];
        for ch in 0..4 {
            px[ch] = src[ch] + px[ch] * (1.0 - a);
        }
    }
}

fn clip(plane: &mut Plane, coverage: &Mask) {
    for (i, &cov) in coverage.data().iter().enumerate() {
        let f = cov as f32 / 255.0;
        plane.data[i * 4..i * 4 + 4].iter_mut().for_each(|v| *v *= f);
    }
}

fn composite(canvas: &mut Plane, item: &Plane) {
    for (dst, src) in canvas.data.chunks_exact_mut(4).zip(item.data.chunks_exact(4)) {
        let keep = 1.0 - src[3];
        for ch in 0..4 {
            dst[ch] = src[ch] + dst[ch] * keep;
        }
    }
}

/// `.shadow`: the item's alpha moved down by `dy` pixels, blurred, in the shadow's colour.
fn shadow(canvas: &mut Plane, item: &Plane, colour: Rgba, strength: f32, sigma: f32, dy: f32) {
    let (w, h) = (item.width, item.height);
    // Moved before the blur, so what lies above the canvas once moved is still blurred into it.
    let (whole, frac) = (dy.floor() as isize, dy - dy.floor());
    let source = |x: usize, y: isize| {
        if y < 0 || y >= h as isize { 0.0 } else { item.data[(y as usize * w + x) * 4 + 3] }
    };
    let mut alpha = Plane::new(w, h, 1);
    for y in 0..h {
        let from = y as isize - whole;
        for x in 0..w {
            alpha.data[y * w + x] = source(x, from) * (1.0 - frac) + source(x, from - 1) * frac;
        }
    }
    gaussian(&mut alpha, sigma);
    let rgb = [colour.r as f32, colour.g as f32, colour.b as f32];
    for (px, &a) in canvas.data.chunks_exact_mut(4).zip(&alpha.data) {
        let a = strength * a;
        if a <= 0.0 {
            continue;
        }
        for ch in 0..3 {
            px[ch] = rgb[ch] * a + px[ch] * (1.0 - a);
        }
        px[3] = a + px[3] * (1.0 - a);
    }
}

/// SwiftUI's linear gradient between evenly spaced stops, interpolated in sRGB.
fn gradient(stops: &[Rgba; DISH_STOPS], t: f32) -> Rgba {
    let at = t * (DISH_STOPS - 1) as f32;
    let i = (at.floor() as usize).min(DISH_STOPS - 2);
    let f = (at - i as f32) as f64;
    let (a, b) = (stops[i], stops[i + 1]);
    let mix = |x: f64, y: f64| x + (y - x) * f;
    Rgba { r: mix(a.r, b.r), g: mix(a.g, b.g), b: mix(a.b, b.b), a: mix(a.a, b.a) }
}
