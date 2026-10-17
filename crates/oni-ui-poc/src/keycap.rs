//! The keycap's light and the slider knob's chrome, rasterised in floating point from the same
//! numbers the Swift `KeycapStyle`, `Frustum` and `DSBarHandle` hold.

/// Linear-light vector, as `Frustum` writes its normals and its light.
pub type V3 = (f64, f64, f64);

/// The subset of `KeycapStyle` the knob reads, at the hardened defaults.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub corner_radius: f64,
    pub wall_top: f64,
    pub wall_bottom: f64,
    pub wall_left: f64,
    pub wall_right: f64,
    pub rise: f64,
    pub face_radius: f64,
    pub dish_depth: f64,
    pub seam_blur: f64,
    pub albedo: [f64; 3],
    pub socket: [f64; 3],
    pub ambient: f64,
    pub diffuse: f64,
    pub azimuth: f64,
    pub elevation: f64,
    pub midtone_match: f64,
}

/// `KeycapStyle.minFaceShare`: the least share of the cap the face keeps.
const MIN_FACE_SHARE: f64 = 0.4;
/// `Keycap.Variant.small.factor`, the app's variant for bar keys and knobs.
pub const SMALL: f64 = 0.525;
/// `DS.Palette.keycap`, #313131, the grey a knob wears when it carries no colour.
pub const KEYCAP_GREY: f64 = 0x31 as f64 / 255.0;
/// `DS.Control.handleThickness` and `handleLength`: the knob along and across its travel.
pub const KNOB_THICKNESS: f64 = 24.0;
pub const KNOB_LENGTH: f64 = 34.0;
/// `DS.Control.knobLine` and `knobLineGap`: the two grip lines on the face.
const GRIP_LINE: f64 = 12.0;
const GRIP_GAP: f64 = 4.0;
/// `DS.Stroke.line`: the black edge `DSBarHandle` strokes round the cap.
const EDGE: f64 = 1.0;
/// `Frustum.dishStops`.
const DISH_STOPS: usize = 7;

impl Style {
    pub fn hardened() -> Self {
        Self {
            corner_radius: 16.0,
            wall_top: 8.0,
            wall_bottom: 12.0,
            wall_left: 8.0,
            wall_right: 8.0,
            rise: 6.0,
            face_radius: 14.0,
            dish_depth: 4.0,
            seam_blur: 3.0,
            albedo: [1.0; 3],
            socket: [0.0; 3],
            ambient: 0.25,
            diffuse: 1.0,
            azimuth: 100.0,
            elevation: 33.0,
            midtone_match: 1.0,
        }
    }

    /// `KeycapStyle.scaled(by:)` for the lengths the knob draws.
    pub fn scaled(mut self, f: f64) -> Self {
        for v in [
            &mut self.corner_radius,
            &mut self.wall_top,
            &mut self.wall_bottom,
            &mut self.wall_left,
            &mut self.wall_right,
            &mut self.rise,
            &mut self.face_radius,
            &mut self.dish_depth,
            &mut self.seam_blur,
        ] {
            *v *= f;
        }
        self
    }

    /// `DSBarHandle.knobStyle`: the small cap, grey, seen head-on with equal walls.
    pub fn knob() -> Self {
        let mut s = Self::hardened().scaled(SMALL);
        s.albedo = [KEYCAP_GREY; 3];
        s.wall_bottom = s.wall_top;
        s
    }

    /// `KeycapStyle.faceRect(in:)`, as `(x, y, w, h)`.
    pub fn face_rect(&self, w: f64, h: f64) -> (f64, f64, f64, f64) {
        let fit = |runs: f64, extent: f64| {
            let allowed = extent * (1.0 - MIN_FACE_SHARE);
            if runs > allowed && runs > 0.0 { allowed / runs } else { 1.0 }
        };
        let squeeze = fit(self.wall_top + self.wall_bottom, h).min(fit(self.wall_left + self.wall_right, w));
        (
            self.wall_left * squeeze,
            self.wall_top * squeeze,
            (w - (self.wall_left + self.wall_right) * squeeze).max(0.0),
            (h - (self.wall_top + self.wall_bottom) * squeeze).max(0.0),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wall {
    Top,
    Bottom,
    Left,
    Right,
}

impl Wall {
    pub const ALL: [Wall; 4] = [Wall::Top, Wall::Bottom, Wall::Left, Wall::Right];

    pub fn run(self, s: &Style) -> f64 {
        match self {
            Wall::Top => s.wall_top,
            Wall::Bottom => s.wall_bottom,
            Wall::Left => s.wall_left,
            Wall::Right => s.wall_right,
        }
    }

    /// `Frustum.Face.normal(run:rise:)`; y grows downward on screen.
    pub fn normal(self, run: f64, rise: f64) -> V3 {
        let l = (run * run + rise * rise).sqrt().max(1e-9);
        match self {
            Wall::Top => (0.0, -rise / l, run / l),
            Wall::Bottom => (0.0, rise / l, run / l),
            Wall::Left => (-rise / l, 0.0, run / l),
            Wall::Right => (rise / l, 0.0, run / l),
        }
    }
}

/// `Frustum.Light`: one shading model for the walls and the dished face.
pub struct Light<'a> {
    pub style: &'a Style,
}

impl Light<'_> {
    pub fn direction(&self) -> V3 {
        let (e, a) = (self.style.elevation.to_radians(), self.style.azimuth.to_radians());
        (e.cos() * a.cos(), -e.cos() * a.sin(), e.sin())
    }

    /// 1 at `midtoneMatch = 0`, the flat face's shade at 1, so the albedo lands on the face's middle.
    pub fn normaliser(&self) -> f64 {
        let s = self.style;
        let flat = s.ambient + s.diffuse * self.direction().2.max(0.0);
        1.0 + (flat - 1.0) * s.midtone_match.clamp(0.0, 1.0)
    }

    pub fn lambert(&self, n: V3) -> f64 {
        let l = self.direction();
        (n.0 * l.0 + n.1 * l.1 + n.2 * l.2).max(0.0)
    }

    /// Unclamped: a shade above 1 lights the albedo past itself, and `shaded` clips each channel.
    pub fn shade(&self, n: V3) -> f64 {
        (self.style.ambient + self.style.diffuse * self.lambert(n)) / self.normaliser().max(1e-6)
    }

    pub fn wall_shade(&self, wall: Wall) -> f64 {
        self.shade(wall.normal(wall.run(self.style), self.style.rise))
    }

    /// `Frustum.dishNormal(at:dishDepth:faceHeight:)`, 0 at the face's top edge.
    pub fn face_shade(&self, t: f64, face_height: f64) -> f64 {
        if face_height <= 0.0 {
            return self.shade((0.0, 0.0, 1.0));
        }
        let slope = 4.0 * self.style.dish_depth * (2.0 * t - 1.0) / face_height;
        let l = (1.0 + slope * slope).sqrt();
        self.shade((0.0, -slope / l, 1.0 / l))
    }

    /// `RGBA.shaded`: albedo times shade, each channel clipped at 1.
    pub fn shaded(&self, shade: f64) -> [f64; 3] {
        self.style.albedo.map(|c| (c * shade).min(1.0))
    }
}

/// A premultiplied RGBA layer in floating point, sRGB-encoded as AppKit composites UI.
struct Layer {
    w: usize,
    h: usize,
    px: Vec<[f64; 4]>,
}

impl Layer {
    fn new(w: usize, h: usize) -> Self {
        Self { w, h, px: vec![[0.0; 4]; w * h] }
    }

    fn over(&mut self, i: usize, c: [f64; 3], coverage: f64) {
        let d = &mut self.px[i];
        let k = 1.0 - coverage;
        for ch in 0..3 {
            d[ch] = c[ch] * coverage + d[ch] * k;
        }
        d[3] = coverage + d[3] * k;
    }

    /// Separable Gaussian, `sigma` in pixels, edges held: the walls are grown past both edges first.
    fn blur(&mut self, sigma: f64) {
        if sigma <= 0.0 {
            return;
        }
        let r = (3.0 * sigma).ceil() as isize;
        let kernel: Vec<f64> = (-r..=r).map(|i| (-(i * i) as f64 / (2.0 * sigma * sigma)).exp()).collect();
        let sum: f64 = kernel.iter().sum();
        let kernel: Vec<f64> = kernel.iter().map(|k| k / sum).collect();
        let (w, h) = (self.w as isize, self.h as isize);
        let pass = |src: &Vec<[f64; 4]>, horizontal: bool| -> Vec<[f64; 4]> {
            let mut out = vec![[0.0; 4]; src.len()];
            for y in 0..h {
                for x in 0..w {
                    let mut acc = [0.0; 4];
                    for (k, weight) in kernel.iter().enumerate() {
                        let o = k as isize - r;
                        let (sx, sy) = if horizontal { ((x + o).clamp(0, w - 1), y) } else { (x, (y + o).clamp(0, h - 1)) };
                        let s = src[(sy * w + sx) as usize];
                        for c in 0..4 {
                            acc[c] += s[c] * weight;
                        }
                    }
                    out[(y * w + x) as usize] = acc;
                }
            }
            out
        };
        let once = pass(&self.px, true);
        self.px = pass(&once, false);
    }
}

/// Coverage of a rounded rectangle at a point, from its signed distance, one pixel of ramp.
fn rounded_coverage(px: f64, py: f64, rect: (f64, f64, f64, f64), radius: f64, pixel: f64) -> f64 {
    let (x, y, w, h) = rect;
    let r = radius.min(w / 2.0).min(h / 2.0).max(0.0);
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let qx = (px - cx).abs() - (w / 2.0 - r);
    let qy = (py - cy).abs() - (h / 2.0 - r);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    let d = outside + qx.max(qy).min(0.0) - r;
    (0.5 - d / pixel).clamp(0.0, 1.0)
}

/// Whether a point lies inside a convex quadrilateral given clockwise or counter-clockwise.
fn in_quad(p: (f64, f64), q: &[(f64, f64); 4]) -> bool {
    let mut sign = 0.0;
    for i in 0..4 {
        let (a, b) = (q[i], q[(i + 1) % 4]);
        let cross = (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0);
        if cross != 0.0 {
            if sign == 0.0 {
                sign = cross.signum();
            } else if cross.signum() != sign {
                return false;
            }
        }
    }
    true
}

/// The knob of a horizontal track at rest, `scale` device pixels per point, premultiplied RGBA8.
/// A hover or a grab only scales it, so one raster serves every state.
pub fn knob_pixels(style: &Style, scale: f64) -> (usize, usize, Vec<u8>) {
    let (wp, hp) = (KNOB_THICKNESS, KNOB_LENGTH);
    let (w, h) = ((wp * scale).round() as usize, (hp * scale).round() as usize);
    let light = Light { style };
    let face = style.face_rect(wp, hp);
    let pixel = 1.0 / scale;

    // The walls in a canvas grown by twice the blur, under the face too, then blurred as one.
    let bleed = 2.0 * style.seam_blur;
    let grow = (bleed * scale).ceil() as usize;
    let mut walls = Layer::new(w + 2 * grow, h + 2 * grow);
    let outer = (-bleed, -bleed, wp + bleed, hp + bleed);
    let inner = (face.0 + bleed, face.1 + bleed, face.0 + face.2 - bleed, face.1 + face.3 - bleed);
    let quad = |wall: Wall| -> [(f64, f64); 4] {
        let (bx0, by0, bx1, by1) = outer;
        let (fx0, fy0, fx1, fy1) = inner;
        match wall {
            Wall::Top => [(bx0, by0), (bx1, by0), (fx1, fy0), (fx0, fy0)],
            Wall::Bottom => [(bx0, by1), (bx1, by1), (fx1, fy1), (fx0, fy1)],
            Wall::Left => [(bx0, by0), (bx0, by1), (fx0, fy1), (fx0, fy0)],
            Wall::Right => [(bx1, by0), (bx1, by1), (fx1, fy1), (fx1, fy0)],
        }
    };
    const SS: usize = 4;
    for y in 0..walls.h {
        for x in 0..walls.w {
            let i = y * walls.w + x;
            for wall in Wall::ALL {
                let q = quad(wall);
                let mut hits = 0;
                for sy in 0..SS {
                    for sx in 0..SS {
                        let p = (
                            (x as f64 - grow as f64 + (sx as f64 + 0.5) / SS as f64) * pixel,
                            (y as f64 - grow as f64 + (sy as f64 + 0.5) / SS as f64) * pixel,
                        );
                        if in_quad(p, &q) {
                            hits += 1;
                        }
                    }
                }
                if hits > 0 {
                    walls.over(i, light.shaded(light.wall_shade(wall)), hits as f64 / (SS * SS) as f64);
                }
            }
        }
    }
    walls.blur(style.seam_blur * scale);

    let stops: Vec<[f64; 3]> = (0..DISH_STOPS)
        .map(|k| light.shaded(light.face_shade(k as f64 / (DISH_STOPS - 1) as f64, face.3)))
        .collect();
    let gradient = |t: f64| -> [f64; 3] {
        let at = t.clamp(0.0, 1.0) * (DISH_STOPS - 1) as f64;
        let k = (at.floor() as usize).min(DISH_STOPS - 2);
        let f = at - k as f64;
        [0, 1, 2].map(|c| stops[k][c] + (stops[k + 1][c] - stops[k][c]) * f)
    };

    let mut out = Layer::new(w, h);
    let silhouette = (0.0, 0.0, wp, hp);
    let grip_x = [wp / 2.0 - GRIP_GAP / 2.0 - EDGE, wp / 2.0 + GRIP_GAP / 2.0];
    let grip_y = hp / 2.0 - GRIP_LINE / 2.0;
    for y in 0..h {
        for x in 0..w {
            let (px, py) = ((x as f64 + 0.5) * pixel, (y as f64 + 0.5) * pixel);
            let i = y * w + x;
            out.over(i, style.socket, 1.0);
            let wall = walls.px[(y + grow) * walls.w + x + grow];
            if wall[3] > 0.0 {
                let c = [wall[0] / wall[3], wall[1] / wall[3], wall[2] / wall[3]];
                out.over(i, c, wall[3]);
            }
            let f = rounded_coverage(px, py, face, style.face_radius, pixel);
            if f > 0.0 {
                out.over(i, gradient((py - face.1) / face.3), f);
            }
            let inset = (EDGE, EDGE, wp - 2.0 * EDGE, hp - 2.0 * EDGE);
            let band = rounded_coverage(px, py, silhouette, style.corner_radius, pixel)
                - rounded_coverage(px, py, inset, style.corner_radius - EDGE, pixel);
            if band > 0.0 {
                out.over(i, [0.0; 3], band.clamp(0.0, 1.0));
            }
            for gx in grip_x {
                let cover_x = ((px + pixel / 2.0).min(gx + EDGE) - (px - pixel / 2.0).max(gx)).max(0.0) / pixel;
                let cover_y = ((py + pixel / 2.0).min(grip_y + GRIP_LINE) - (py - pixel / 2.0).max(grip_y)).max(0.0) / pixel;
                if cover_x * cover_y > 0.0 {
                    out.over(i, [1.0; 3], (cover_x * cover_y).min(1.0));
                }
            }
            let clip = rounded_coverage(px, py, silhouette, style.corner_radius, pixel);
            let d = &mut out.px[i];
            for c in d.iter_mut() {
                *c *= clip;
            }
        }
    }
    let bytes = out
        .px
        .iter()
        .flat_map(|p| p.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8))
        .collect();
    (w, h, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three walls of the large cap, as the site's generated stylesheet quotes them from the Swift renderer.
    const WALLS_LARGE: [(Wall, f64); 3] = [(Wall::Left, 0.9729), (Wall::Right, 0.7530), (Wall::Bottom, 0.4629)];

    #[test]
    fn the_walls_shade_as_the_swift_light_does() {
        let style = Style::hardened();
        let light = Light { style: &style };
        for (wall, expected) in WALLS_LARGE {
            let got = light.wall_shade(wall);
            assert!((got - expected).abs() < 5e-4, "{wall:?}: {got} against {expected}");
        }
        // The top wall passes 1, unclamped: a dark cap's top lights past its albedo.
        assert!(light.wall_shade(Wall::Top) > 1.4);
        // Twin: without the midtone normalisation the same walls miss the quoted values.
        let mut flat = style;
        flat.midtone_match = 0.0;
        let wrong = Light { style: &flat };
        assert!(WALLS_LARGE.iter().any(|(w, e)| (wrong.wall_shade(*w) - e).abs() > 0.05));
    }

    #[test]
    fn the_dish_is_the_albedo_at_its_middle() {
        let style = Style::knob();
        let light = Light { style: &style };
        let (_, _, _, fh) = style.face_rect(KNOB_THICKNESS, KNOB_LENGTH);
        assert!((light.face_shade(0.5, fh) - 1.0).abs() < 1e-12);
        assert!(light.face_shade(0.0, fh) < 1.0 && light.face_shade(1.0, fh) > 1.0, "lit from above, dark at the top");
        let mut flat = style;
        flat.midtone_match = 0.0;
        assert!((Light { style: &flat }.face_shade(0.5, fh) - 1.0).abs() > 0.1, "twin: the unnormalised face is not the albedo");
    }

    #[test]
    fn the_knob_face_and_walls_sit_where_the_swift_layout_puts_them() {
        let style = Style::knob();
        let face = style.face_rect(KNOB_THICKNESS, KNOB_LENGTH);
        let run = 8.0 * SMALL;
        assert_eq!(face, (run, run, KNOB_THICKNESS - 2.0 * run, KNOB_LENGTH - 2.0 * run));
        // Twin: the key's own thicker lower wall would lower the face's bottom edge.
        let key = Style::hardened().scaled(SMALL);
        assert_ne!(key.face_rect(KNOB_THICKNESS, KNOB_LENGTH), face);
    }

    #[test]
    fn the_raster_shows_each_wall_in_its_own_shade() {
        let style = Style::knob();
        let light = Light { style: &style };
        let scale = 4.0;
        let (w, h, px) = knob_pixels(&style, scale);
        assert_eq!((w, h), (96, 136));
        let at = |x: f64, y: f64| {
            let i = ((y * scale) as usize * w + (x * scale) as usize) * 4;
            px[i] as f64 / 255.0
        };
        // Mid-wall, away from corners and seams: blurred, but within a few levels of its flat shade.
        let left = light.shaded(light.wall_shade(Wall::Left))[0];
        let top = light.shaded(light.wall_shade(Wall::Top))[0];
        assert!((at(2.6, 17.0) - left).abs() < 0.03, "left wall {} against {left}", at(2.6, 17.0));
        assert!((at(12.0, 2.6) - top).abs() < 0.03, "top wall {} against {top}", at(12.0, 2.6));
        // The grip line is white, the face beside it the dish's grey.
        assert!(at(KNOB_THICKNESS / 2.0 + 2.5, KNOB_LENGTH / 2.0) > 0.95);
        assert!(at(KNOB_THICKNESS / 2.0, KNOB_LENGTH / 2.0) < 0.3);
        // Twin: the two walls the light treats differently do not come out equal.
        assert!((at(2.6, 17.0) - at(KNOB_THICKNESS - 2.6, 17.0)).abs() > 0.02);
        // Outside the rounded corner nothing is drawn.
        assert_eq!(px[3], 0);
    }
}
