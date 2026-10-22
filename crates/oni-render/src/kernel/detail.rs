//! Detail (`chroma.metal`): the separable Gaussian taps, by rows then columns.

use super::*;

/// The arguments `detailRows` and `detailColumns` share, built as `Pipeline.applyDetail` builds them.
/// `.x` of `sigma` and `reach` is the chroma denoise, `.y` the sharpening; radii are pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detail {
    pub sigma: V2,
    pub reach: V2,
    pub disk: f32,
    pub force: f32,
    pub amount: f32,
    pub halo: f32,
}

/// `Pipeline.blurFloor`: a radius at or under it leaves its half of the stage out.
pub const BLUR_FLOOR: f64 = 0.1614;

/// `Pipeline.detailReach`: the taps each side a blur of deviation `sigma` reads, out to 3.5 deviations.
pub fn detail_reach(sigma: f64) -> i32 {
    (3.5 * sigma).ceil() as i32
}

impl Detail {
    /// `applyDetail`'s arguments from the pixel radius and force of the chroma denoise and the pixel
    /// radius, amount and halo of the sharpening; `None` where neither half acts.
    pub fn dosed(chroma: Option<(f64, f32)>, sharpen: Option<(f64, f32, f32)>) -> Option<Self> {
        let chroma = chroma.filter(|c| c.0 > BLUR_FLOOR);
        let sharpen = sharpen.filter(|s| s.0 > BLUR_FLOOR);
        if chroma.is_none() && sharpen.is_none() {
            return None;
        }
        let halo = sharpen.map_or(0.0, |s| s.2);
        let disk = if halo > 0.0 { sharpen.map_or(1.0, |s| s.0).max(1.0).floor() } else { 0.0 };
        Some(Self {
            sigma: [chroma.map_or(0.0, |c| c.0 as f32), sharpen.map_or(0.0, |s| s.0 as f32)],
            reach: [
                chroma.map_or(0, |c| detail_reach(c.0)) as f32,
                sharpen.map_or(0, |s| detail_reach(s.0)) as f32,
            ],
            disk: disk as f32,
            force: chroma.map_or(0.0, |c| c.1),
            amount: sharpen.map_or(0.0, |s| s.1),
            halo,
        })
    }

    /// How far the columns read the rows vertically: `max(reach, disk)`, the rows pass's vertical margin.
    pub fn margin(&self) -> usize {
        self.reach[0].max(self.reach[1]).max(self.disk) as usize
    }

    pub fn rows(&self) -> Vec<V4> {
        vec![v22(self.sigma, self.reach), [self.disk, self.force, self.amount, self.halo]]
    }
}

const SHARPEN_TOE: f32 = 0.01;

fn detail_erf(x: f32) -> f32 {
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
    let poly = ((((1.061_405_4 * t - 1.453_152) * t + 1.421_413_8) * t - 0.284_496_74) * t + 0.254_829_6) * t;
    let v = 1.0 - poly * (-x * x).exp();
    if x >= 0.0 { v } else { -v }
}

pub(super) struct Taps {
    scale: f32,
    norm: f32,
    inner: f32,
}

impl Taps {
    pub(super) fn new(sigma: f32, reach: f32) -> Self {
        let scale = if sigma > 0.0 { std::f32::consts::FRAC_1_SQRT_2 / sigma } else { 0.0 };
        let norm = if sigma > 0.0 { 1.0 / detail_erf((reach + 0.5) * scale) } else { 0.0 };
        Self { scale, norm, inner: detail_erf(0.5 * scale) }
    }

    pub(super) fn centre(&self) -> f32 {
        self.inner * self.norm
    }

    pub(super) fn next(&mut self, k: i32) -> f32 {
        let outer = detail_erf((k as f32 + 0.5) * self.scale);
        let w = 0.5 * (outer - self.inner) * self.norm;
        self.inner = outer;
        w
    }
}

/// `detailRows`: the source widened by `disk` each side, read clamped to its edges.
pub fn detail_rows(image: &Image, k: &Detail) -> Vec<V4> {
    let (w, h) = (image.width as i64, image.height);
    let disk = k.disk as i64;
    let wide = w + 2 * disk;
    let clamped = |x: i64, y: usize| rgb(image.pixels[y * w as usize + x.clamp(0, w - 1) as usize]);
    let reach = k.reach;
    let mut out = Vec::with_capacity(wide as usize * h);
    for y in 0..h {
        for xw in 0..wide {
            let x = xw - disk;
            let mut chroma_taps = Taps::new(k.sigma[0], reach[0]);
            let mut luma_taps = Taps::new(k.sigma[1], reach[1]);
            let s = clamped(x, y);
            let centre = dot(s, LUMA);
            let cw = chroma_taps.centre();
            let mut chroma = [cw * (s[0] - centre), cw * (s[1] - centre)];
            let mut luma = luma_taps.centre() * centre;
            for d in 1..=(reach[0].max(reach[1]) as i32) {
                let a = clamped(x + d as i64, y);
                let b = clamped(x - d as i64, y);
                let (ya, yb) = (dot(a, LUMA), dot(b, LUMA));
                if d as f32 <= reach[0] {
                    let t = chroma_taps.next(d);
                    chroma[0] += t * (a[0] - ya + b[0] - yb);
                    chroma[1] += t * (a[1] - ya + b[1] - yb);
                }
                if d as f32 <= reach[1] {
                    luma += luma_taps.next(d) * (ya + yb);
                }
            }
            out.push([chroma[0], chroma[1], luma, centre]);
        }
    }
    out
}

/// `detailRows` then `detailColumns`; the columns read the rows clamped vertically, which is
/// what the rows of a clamped source hold past its top and bottom.
pub fn detail(image: &Image, k: &Detail) -> Image {
    let (w, h) = (image.width, image.height);
    if w * h == 0 {
        return image.clone();
    }
    let disk = k.disk as i64;
    let wide = w as i64 + 2 * disk;
    let rows = detail_rows(image, k);
    let row = |x: i64, y: i64| rows[(y.clamp(0, h as i64 - 1) * wide + x + disk) as usize];
    let reach = k.reach;
    image_map(image, |xu, yu, s| {
        let (x, y) = (xu as i64, yu as i64);
        let mut chroma_taps = Taps::new(k.sigma[0], reach[0]);
        let mut luma_taps = Taps::new(k.sigma[1], reach[1]);
        let here = row(x, y);
        let cw = chroma_taps.centre();
        let mut chroma = [cw * here[0], cw * here[1]];
        let mut luma = luma_taps.centre() * here[2];
        for d in 1..=(reach[0].max(reach[1]) as i32) {
            let a = row(x, y + d as i64);
            let b = row(x, y - d as i64);
            if d as f32 <= reach[0] {
                let t = chroma_taps.next(d);
                chroma[0] += t * (a[0] + b[0]);
                chroma[1] += t * (a[1] + b[1]);
            }
            if d as f32 <= reach[1] {
                luma += luma_taps.next(d) * (a[2] + b[2]);
            }
        }
        let mut e = rgb(s);
        if k.force > 0.0 {
            let yl = dot(e, LUMA);
            let blurred = [chroma[0], chroma[1], -(LUMA[0] * chroma[0] + LUMA[1] * chroma[1]) / LUMA[2]];
            let mixed = map(e, |c, v| mix(v - yl, blurred[c], k.force));
            let lost = dot(mixed, LUMA);
            e = map(mixed, |_, v| yl + (v - lost));
        }
        if k.amount > 0.0 {
            let yl = dot(e, LUMA).max(0.0);
            let l = (yl + SHARPEN_TOE).log2();
            let mut sharpened = l + (l - (luma.max(0.0) + SHARPEN_TOE).log2()) * k.amount;
            if k.halo > 0.0 {
                let r = k.disk as i64;
                let (mut lowest, mut highest) = (f32::INFINITY, f32::NEG_INFINITY);
                for dy in -r..=r {
                    let span = ((r * r - dy * dy) as f32).sqrt().floor() as i64;
                    for dx in -span..=span {
                        let v = row(x + dx, y + dy)[3];
                        lowest = lowest.min(v);
                        highest = highest.max(v);
                    }
                }
                let held = clamp(sharpened, (lowest.max(0.0) + SHARPEN_TOE).log2(), (highest.max(0.0) + SHARPEN_TOE).log2());
                sharpened = mix(sharpened, held, k.halo);
            }
            let gain = sharpened.exp2() - SHARPEN_TOE - yl;
            e = map(e, |_, v| v + gain);
        }
        with_alpha(e, s[3])
    })
}
