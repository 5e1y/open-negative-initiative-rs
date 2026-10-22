//! The vignette (`chroma.metal`).

use super::*;

/// `Vignette.midpoint`: flat inside this share of the half-diagonal.
pub const VIGNETTE_MIDPOINT: f32 = 0.3;

/// `vignette(s, centre, halfSide, dose, hunt)`, in Core Image coordinates; `dose` is (stops, midpoint).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vignette {
    pub centre: V2,
    pub half_side: V2,
    pub dose: V2,
    pub hunt: f32,
}

impl Vignette {
    /// `Pipeline.applyVignette` on an extent: centred, the half sides its own, hunt 0 in monochrome.
    pub fn on_extent(origin: V2, width: usize, height: usize, stops: f32, hunt: f32) -> Self {
        let (w, h) = (width as f32, height as f32);
        Self {
            centre: [origin[0] + w / 2.0, origin[1] + h / 2.0],
            half_side: [w / 2.0, h / 2.0],
            dose: [stops, VIGNETTE_MIDPOINT],
            hunt,
        }
    }

    pub fn rows(&self) -> Vec<V4> {
        vec![v22(self.centre, self.half_side), [self.dose[0], self.dose[1], self.hunt, 0.0]]
    }
}

pub fn vignette(image: &Image, k: &Vignette) -> Image {
    image_map(image, |x, y, s| {
        let at = image.ci_coord(x, y);
        let d = [(at[0] - k.centre[0]) / k.half_side[0], (at[1] - k.centre[1]) / k.half_side[1]];
        let r = (d[0] * d[0] + d[1] * d[1]).sqrt() * 0.707_106_77;
        let t = ((r - k.dose[1]) / (1.0 - k.dose[1])).max(0.0);
        let f = (-k.dose[0] * t * t).exp2();
        let mut e = map(rgb(s), |_, v| v * f);
        let m = clamp(f, 0.25, 4.0).powf(-k.hunt);
        if m != 1.0 {
            let p = map(e, |_, v| v.max(0.0).powf(m));
            let yl = dot(p, LUMA);
            if yl > 1e-12 {
                let g = yl.powf(1.0 / m - 1.0);
                e = map(p, |_, v| v * g);
            }
        }
        with_alpha(e, s[3])
    })
}
