//! The reference: slow, scalar, written to be read beside the kernel it judges.

use crate::{
    AreaHistogram, ColorClamp, ColorCube, ColorMatrix, CorrectionBiasField, CorrectionCost, Curves, Detail,
    ExposureAdjust, FlatField, Image, LinearGradient, RadialGradient, Rect, SaturationCurve, ScannerDensity,
    ScannerEncode, ScannerOutput, ScreenGamutZebra, Spectrogram, Stage47, Texture, TextureCoeff, Vignette,
};

const LUMA: [f32; 3] = [0.2627, 0.6780, 0.0593];
const DISPLAY_GAMMA: f32 = 2.2;
const CONTRAST_WARP: f32 = 0.889_272_9;
const CONTRAST_STEP: f32 = 0.05;

type V3 = [f32; 3];

/// Metal's `clamp`, `fmin(fmax(x, low), high)`: a NaN takes the low bound, and crossed bounds
/// never panic as `f32::clamp` does.
fn clamp(x: f32, low: f32, high: f32) -> f32 {
    x.max(low).min(high)
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn map(a: V3, f: impl Fn(usize, f32) -> f32) -> V3 {
    [f(0, a[0]), f(1, a[1]), f(2, a[2])]
}

fn contrasted(x: f32, travel: f32) -> f32 {
    let b = travel.abs() * 15.2;
    if b <= 1e-4 {
        return x;
    }
    let mut t = clamp(x, 0.0, 1.0).powf(CONTRAST_WARP);
    let low = 1.0 / (1.0 + (b * 0.5).exp());
    let high = 1.0 / (1.0 + (-b * 0.5).exp());
    if travel > 0.0 {
        t = (1.0 / (1.0 + (-b * (t - 0.5)).exp()) - low) / (high - low);
    } else {
        let g = low + t * (high - low);
        t = clamp(0.5 - (1.0 / g - 1.0).ln() / b, 0.0, 1.0);
    }
    t.powf(1.0 / CONTRAST_WARP)
}

fn apply_levels(e: f32, lv: V3) -> f32 {
    ((e - lv[0]) / (lv[1] - lv[0]).max(1e-4)).max(0.0).powf(lv[2])
}

fn apply_window(e: f32, lv: V3, c1: f32, c3: f32) -> f32 {
    let t = apply_levels(e, lv);
    if c1 == 0.25 && c3 == 0.75 {
        return t;
    }
    let u = t.min(1.0);
    let v = 1.0 - u;
    let curve = c1 * (4.0 * u * v * v * v) + 0.5 * (6.0 * u * u * v * v) + c3 * (4.0 * u * u * u * v) + u * u * u * u;
    curve + (t - u)
}

fn quarter_pass(p: f32, t: f32, high: bool) -> f32 {
    if high {
        p + t * 4.0 * p * p * p * (1.0 - p)
    } else {
        p + t * 4.0 * p * (1.0 - p) * (1.0 - p) * (1.0 - p)
    }
}

fn quartered(p: f32, travel: f32, high: bool) -> f32 {
    if travel.abs() <= 0.25 {
        return quarter_pass(p, travel, high);
    }
    let first = if travel > 0.0 { 0.25 } else { -0.25 };
    quarter_pass(quarter_pass(p, first, high), travel - first, high)
}

fn shaped(p_in: f32, stops: f32, contrast: f32, zones: [f32; 2]) -> f32 {
    let held = clamp(p_in, 0.0, 1.0);
    let mut p = contrasted(held, contrast) + (p_in - held);
    if stops != 0.0 && p > 0.0 {
        let top = p.min(1.0);
        let g = stops.exp2();
        let v = top.powf(DISPLAY_GAMMA);
        p = (v * g / (1.0 + v * (g - 1.0))).powf(1.0 / DISPLAY_GAMMA) + (p - top);
    }
    if zones != [0.0, 0.0] && (0.0..=1.0).contains(&p) {
        p = quartered(p, zones[0], false);
        p = quartered(p, zones[1], true);
    }
    p
}

fn printed_rgb(n: f32, stops: f32, contrast: f32, zones: [f32; 2]) -> f32 {
    shaped(n, stops, contrast, zones).max(0.0).powf(DISPLAY_GAMMA)
}

fn printed(n: f32, stops: f32, contrast: f32, zones: [f32; 2], own: f32) -> f32 {
    let p = shaped(n, stops, contrast, zones);
    shaped(p, own, 0.0, [0.0, 0.0]).max(0.0).powf(DISPLAY_GAMMA)
}

/// Stages 4 to 7 on one pixel.
pub fn stage47(s: [f32; 4], k: &Stage47) -> [f32; 4] {
    let mut density = k.density;
    let t = map([s[0], s[1], s[2]], |c, x| (x + k.pedestal) * k.gain[c]);
    let d = map(t, |_, x| -x.max(k.log_guard).log10());
    let mut e = if k.invert > 0.5 { d } else { map(d, |_, x| k.window - x) };
    let lv = [k.red, k.green, k.blue];

    let film = e;
    e = map(e, |c, x| apply_window(x, lv[c], k.low_points[c], k.high_points[c]));
    if k.decompression != 1.0 || k.film_share != 0.0 {
        let place = map(film, |c, x| (x - k.balance_low[c]) / k.balance_span[c]);
        let mean = (place[0] + place[1] + place[2]) / 3.0;
        let neutral = map(k.balance_low, |c, x| x + mean * k.balance_span[c]);
        let toned = map(neutral, |c, x| apply_window(x, lv[c], k.low_points[c], k.high_points[c]));
        let held = map(toned, |c, x| x + k.decompression * (e[c] - x) + k.film_share * (film[c] - neutral[c]));
        let shift = dot(e, LUMA) - dot(held, LUMA);
        e = map(held, |_, x| x + shift);
    }
    e = [dot(k.sep_r, e), dot(k.sep_g, e), dot(k.sep_b, e)];
    if k.print_on > 0.5 {
        let (ps, pc, pz) = (k.print_stops, k.print_contrast, k.print_zones);
        let windowed = e;
        e = map(e, |c, x| printed(x, ps, pc, pz, k.own_stops[c]));
        if k.own_stops != [0.0; 3] {
            let plain = map(windowed, |_, x| printed_rgb(x, ps, pc, pz));
            let filtered = dot(e, LUMA);
            if filtered > 1e-6 {
                let f = dot(plain, LUMA) / filtered;
                e = map(e, |_, x| x * f);
            }
        }
        let bent = ps != k.base_stops || pc != k.base_contrast || pz != k.base_zones;
        if k.hunt > 0.0 {
            let g = (windowed[0] + windowed[1] + windowed[2]) / 3.0;
            density *= (-k.hunt * (shaped(g, ps, pc, pz) - g)).exp();
        }
        if k.follow < 1.0 && bent {
            let g = (windowed[0] + windowed[1] + windowed[2]) / 3.0;
            if g > 1e-3 {
                let (up, down) = (g * (1.0 + CONTRAST_STEP), g * (1.0 - CONTRAST_STEP));
                let through = |x: f32, s: f32, c: f32, z: [f32; 2]| shaped(x, s, c, z).max(1e-6).powf(DISPLAY_GAMMA);
                let curve = (through(up, ps, pc, pz) / through(down, ps, pc, pz)).ln();
                let base = (through(up, k.base_stops, k.base_contrast, k.base_zones)
                    / through(down, k.base_stops, k.base_contrast, k.base_zones))
                .ln();
                let r = if base.abs() > 1e-6 { clamp(curve / base, 0.25, 4.0) } else { 1.0 };
                let grey = printed_rgb(g, ps, pc, pz);
                let keep = r.powf(k.follow - 1.0).min(1.0);
                let held = map(e, |_, x| (grey + (x - grey) * keep).max(0.0));
                let lit = dot(held, LUMA);
                if lit > 1e-6 {
                    let f = dot(e, LUMA) / lit;
                    e = map(held, |_, x| x * f);
                }
            }
        }
    }
    if density != 1.0 {
        let pw = map(e, |_, x| x.max(0.0).powf(density));
        let p = dot(pw, LUMA);
        if p > 1e-12 {
            let f = p.powf(1.0 / density - 1.0);
            e = map(pw, |_, x| x * f);
        }
    }
    [e[0], e[1], e[2], s[3]]
}

type V2 = [f32; 2];
type V4 = [f32; 4];

fn rgb(p: V4) -> V3 {
    [p[0], p[1], p[2]]
}

fn with_alpha(c: V3, a: f32) -> V4 {
    [c[0], c[1], c[2], a]
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
    let t = clamp((x - low) / (high - low), 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn image_map(image: &Image, f: impl Fn(usize, usize, V4) -> V4) -> Image {
    let pixels = (0..image.height)
        .flat_map(|y| (0..image.width).map(move |x| (x, y)))
        .map(|(x, y)| f(x, y, image.at(x, y)))
        .collect();
    image.with_pixels(pixels)
}

// MARK: - pipeline.metal

pub fn output_clip(s: V4) -> V4 {
    with_alpha(map(rgb(s), |_, x| clamp(x, 0.0, 1.0)), s[3])
}

pub fn flat_field(s: V4, f: V4, k: &FlatField) -> V4 {
    let m = if k.per_channel > 0.5 { map(rgb(f), |_, x| x.max(0.05)) } else { [f[1].max(0.05); 3] };
    let d = map(m, |c, x| mix(1.0, x, k.amount).powf(k.exponent[c]));
    with_alpha(map(rgb(s), |c, x| x / d[c]), s[3])
}

// MARK: - curves.metal

const LUMA_SCALE_MAX: f32 = 8.0;

/// A table read linearly at `v * (size - 1)`: Core Image's sample at `mix(0.5, size - 0.5, v)`.
fn table(lut: &[V4], v: f32, c: usize) -> f32 {
    let n = lut.len();
    let f = clamp(v, 0.0, 1.0) * (n - 1) as f32;
    let k = (f.floor() as usize).min(n - 1);
    mix(lut[k][c], lut[(k + 1).min(n - 1)][c], f - k as f32)
}

fn apply_curve(lut: &[V4], v: f32, c: usize) -> f32 {
    if v <= 1.0 { table(lut, v, c) } else { table(lut, 1.0, c) + (v - 1.0) }
}

pub fn curves(s: V4, k: &Curves) -> V4 {
    let mut e = rgb(s);
    if k.bent[0] > 0.5 {
        e = map(e, |c, x| apply_curve(&k.rgb_lut, x, c + 1));
    }
    if k.bent[1] > 0.5 {
        e = map(e, |_, x| apply_curve(&k.rgb_lut, x, 0));
    }
    let y = dot(e, LUMA);
    if k.bent[2] > 0.5 && y > 1e-4 {
        let f = clamp(apply_curve(&k.luma_lut, y, 0) / y, 0.0, LUMA_SCALE_MAX);
        e = map(e, |_, x| x * f);
    }
    with_alpha(e, s[3])
}

// MARK: - chroma.metal

pub fn saturation_curve(s: V4, k: &SaturationCurve) -> V4 {
    let y = dot(rgb(s), LUMA);
    let x = clamp(y.max(0.0).powf(1.0 / 2.2), 0.0, 1.0);
    let xs = [0.0, k.knots[0], k.knots[1], k.knots[2], 1.0];
    let ys = [k.lows[0], k.lows[1], k.lows[2], k.lows[3], k.high];
    let ms = [0.0, k.bends[0], k.bends[1], k.bends[2], 0.0];
    let i = if x < xs[1] { 0 } else if x < xs[2] { 1 } else if x < xs[3] { 2 } else { 3 };
    let h = (xs[i + 1] - xs[i]).max(1e-6);
    let t = x - xs[i];
    let b = (ys[i + 1] - ys[i]) / h - h * (2.0 * ms[i] + ms[i + 1]) / 6.0;
    let value = ys[i] + b * t + ms[i] / 2.0 * t * t + (ms[i + 1] - ms[i]) / (6.0 * h) * t * t * t;
    let factor = (2.0 * value).max(0.0);
    let chroma = map(rgb(s), |_, c| (c - y) * factor);
    let lost = dot(chroma, LUMA);
    with_alpha(map(chroma, |_, c| y + (c - lost)), s[3])
}

pub fn texture_moments(luma: V4) -> V4 {
    [luma[0], luma[0] * luma[0], 0.0, 1.0]
}

pub fn texture_coeff(m: V4, k: &TextureCoeff) -> V4 {
    let mean = m[0];
    let variance = (m[1] - mean * mean).max(0.0);
    let edge = k.relative * mean.max(0.0);
    let eps = (edge * edge).max(1e-8);
    let a = variance / (variance + eps);
    [a, (1.0 - a) * mean, 0.0, 1.0]
}

pub fn texture(s: V4, narrow_luma: V4, coeff: V4, k: &Texture) -> V4 {
    let y = dot(rgb(s), LUMA);
    let n = narrow_luma[0];
    let base = coeff[0] * n + coeff[1];
    let shaped = y + (n - base) * k.gain;
    with_alpha(map(rgb(s), |_, c| shaped.max(0.0) + (c - y)), s[3])
}

const SHARPEN_TOE: f32 = 0.01;

fn detail_erf(x: f32) -> f32 {
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
    let poly = ((((1.061_405_4 * t - 1.453_152) * t + 1.421_413_8) * t - 0.284_496_74) * t + 0.254_829_6) * t;
    let v = 1.0 - poly * (-x * x).exp();
    if x >= 0.0 { v } else { -v }
}

struct Taps {
    scale: f32,
    norm: f32,
    inner: f32,
}

impl Taps {
    fn new(sigma: f32, reach: f32) -> Self {
        let scale = if sigma > 0.0 { std::f32::consts::FRAC_1_SQRT_2 / sigma } else { 0.0 };
        let norm = if sigma > 0.0 { 1.0 / detail_erf((reach + 0.5) * scale) } else { 0.0 };
        Self { scale, norm, inner: detail_erf(0.5 * scale) }
    }

    fn centre(&self) -> f32 {
        self.inner * self.norm
    }

    fn next(&mut self, k: i32) -> f32 {
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

const BAND_CENTRE: [f32; 8] = [0.0, 30.0, 60.0, 120.0, 180.0, 240.0, 270.0, 300.0];
const BAND_SPREAD: f32 = 1.8;
const LUMA_STOPS: f32 = 2.0;

fn band_weight(hue: f32, i: usize) -> f32 {
    let centre = BAND_CENTRE[i];
    let mut delta = hue - centre;
    if delta > 180.0 {
        delta -= 360.0;
    }
    if delta < -180.0 {
        delta += 360.0;
    }
    let neighbour = if delta >= 0.0 { (i + 1) & 7 } else { (i + 7) & 7 };
    let mut span = (BAND_CENTRE[neighbour] - centre).abs();
    if span > 180.0 {
        span = 360.0 - span;
    }
    smoothstep(0.0, 1.0, (1.0 - delta.abs() / (span * BAND_SPREAD)).max(0.0))
}

fn dot4(a: V4, b: V4) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

pub fn spectrogram(s: V4, k: &Spectrogram) -> V4 {
    let src = rgb(s);
    let y = dot(src, LUMA);
    let chroma = map(src, |_, c| c - y);
    let a = dot(chroma, [1.0, -0.5, -0.5]);
    let b = dot(chroma, [0.0, 0.866_025_4, -0.866_025_4]);
    let presence = smoothstep(0.0, 0.10 + 0.25 * y.max(0.0), (a * a + b * b).sqrt());
    if presence <= 0.0 {
        return s;
    }
    let mut hue = b.atan2(a) * 57.295_78;
    if hue < 0.0 {
        hue += 360.0;
    }
    let mut w_low: V4 = std::array::from_fn(|i| band_weight(hue, i));
    let mut w_high: V4 = std::array::from_fn(|i| band_weight(hue, i + 4));
    let total = w_low.iter().sum::<f32>() + w_high.iter().sum::<f32>();
    if total > 0.0 {
        w_low = w_low.map(|w| w / total);
        w_high = w_high.map(|w| w / total);
    }
    let shift = (dot4(w_low, k.hue_low) + dot4(w_high, k.hue_high)) * presence;
    let boost = (dot4(w_low, k.sat_low) + dot4(w_high, k.sat_high)) * presence;
    let lift = (dot4(w_low, k.luma_low) + dot4(w_high, k.luma_high)) * presence;

    let axis = [0.577_350_26_f32; 3];
    let angle = shift * 0.017_453_292;
    let (sn, c) = angle.sin_cos();
    let cross = [
        axis[1] * src[2] - axis[2] * src[1],
        axis[2] * src[0] - axis[0] * src[2],
        axis[0] * src[1] - axis[1] * src[0],
    ];
    let along = dot(axis, src);
    let turned = map(src, |i, v| v * c + cross[i] * sn + axis[i] * along * (1.0 - c));
    let restore = y - dot(turned, LUMA);
    let turned = map(turned, |_, v| v + restore);
    let gain = (1.0 + boost).max(0.0);
    let light = (lift * LUMA_STOPS).exp2();
    with_alpha(map(turned, |_, v| (y + (v - y) * gain) * light), s[3])
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

// MARK: - gamut.metal

pub fn screen_gamut_zebra(picture: &Image, shown: &Image, k: &ScreenGamutZebra) -> Image {
    image_map(picture, |x, y, p| {
        let seen = shown.at(x, y);
        let beyond = (0..3).any(|c| seen[c] < -k.tolerance || seen[c] > 1.0 + k.tolerance);
        if !beyond {
            return p;
        }
        let at = picture.ci_coord(x, y);
        if (at[0] + at[1] + k.phase) % k.period < k.duty * k.period { k.tint } else { p }
    })
}

// MARK: - rgbPasses.metal

pub fn rgb_passes(red: V4, green: V4, blue: V4) -> V4 {
    [red[0], green[1], blue[2], 1.0]
}

// MARK: - scanner.metal

pub fn scanner_density(s: V4, k: &ScannerDensity) -> V4 {
    let t = map(rgb(s), |c, x| (x + k.pedestal) * k.gain[c]);
    let density = map(t, |_, x| -x.max(k.log_guard).log10());
    let e = if k.invert > 0.5 { density } else { map(density, |_, x| k.dmax - x) };
    let d = map(e, |c, x| x - k.median[c]);
    let sep = [dot(k.sep_r, d), dot(k.sep_g, d), dot(k.sep_b, d)];
    let x = map(sep, |c, v| k.scale * v + k.shift[c]);
    let held = map(x, |c, v| clamp(v, k.reached_low[c], k.reached_high[c]));
    with_alpha(map(held, |_, v| clamp((v - k.low) / k.span, 0.0, 1.0)), s[3])
}

pub fn scanner_output(code: V4, warmth: V4, k: &ScannerOutput) -> V4 {
    let c = map(rgb(code), |i, v| clamp(v + (warmth[i] - k.lift) * k.warmth_share, 0.0, 1.0));
    let linear = map(c, |_, v| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) });
    let mut out = [
        dot(linear, [0.627_404, 0.329_283, 0.043_313]),
        dot(linear, [0.069_097, 0.919_540, 0.011_362]),
        dot(linear, [0.016_391, 0.088_013, 0.895_595]),
    ];
    if k.contrast != 0.0 {
        out = map(out, |_, v| contrasted(v.powf(1.0 / DISPLAY_GAMMA), k.contrast).powf(DISPLAY_GAMMA));
    }
    with_alpha(out, code[3])
}

pub fn scanner_encode(s: V4, k: &ScannerEncode) -> V4 {
    let src = rgb(s);
    let l = [
        dot(src, [1.660_49, -0.587_641, -0.072_850]),
        dot(src, [-0.124_550, 1.132_9, -0.008_349]),
        dot(src, [-0.018_150, -0.100_579, 1.118_73]),
    ];
    let code = map(l, |_, v| {
        let c = clamp(v, 0.0, 1.0);
        if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
    });
    with_alpha(map(code, |_, v| v.max(0.0).powf(1.0 / k.root)), s[3])
}

// MARK: - correctionSearch.metal

/// Core Image's linear sample at `p` in its own frame; transparent outside the extent.
pub fn sampled(image: &Image, p: V2) -> V3 {
    let f = [p[0] - image.origin[0] - 0.5, image.height as f32 - (p[1] - image.origin[1]) - 0.5];
    let base = [f[0].floor(), f[1].floor()];
    let t = [f[0] - base[0], f[1] - base[1]];
    let (x, y) = (base[0] as i64, base[1] as i64);
    let texel = |x: i64, y: i64| {
        if x < 0 || y < 0 || x >= image.width as i64 || y >= image.height as i64 {
            [0.0; 3]
        } else {
            rgb(image.at(x as usize, y as usize))
        }
    };
    let (a, b, c, d) = (texel(x, y), texel(x + 1, y), texel(x, y + 1), texel(x + 1, y + 1));
    map([0.0; 3], |i, _| mix(mix(a[i], b[i], t[0]), mix(c[i], d[i], t[0]), t[1]))
}

/// The cost map on the extent (0, 0, `grid_size`, `grid_size`), top row first.
pub fn correction_cost(source: &Image, k: &CorrectionCost, grid_size: usize) -> Image {
    let grid = Image::new(grid_size, grid_size, vec![[0.0; 4]; grid_size * grid_size]);
    image_map(&grid, |gx, gy, _| {
        let at = grid.ci_coord(gx, gy);
        let offset = [k.window_origin[0] + at[0] * k.step, k.window_origin[1] + at[1] * k.step];
        let cand = [k.dest_centre[0] + offset[0], k.dest_centre[1] + offset[1]];
        let hs = k.half_size;
        if (offset[0] * offset[0] + offset[1] * offset[1]).sqrt() < k.min_radius
            || cand[0] - hs[0] < k.extent_min[0]
            || cand[0] + hs[0] > k.extent_max[0]
            || cand[1] - hs[1] < k.extent_min[1]
            || cand[1] + hs[1] > k.extent_max[1]
        {
            return [f32::INFINITY, offset[0], offset[1], 1.0];
        }
        let n = k.grid_n as i32;
        let hole = k.hole;
        let inside = |p: V2| p[0] >= hole[0] && p[0] <= hole[2] && p[1] >= hole[1] && p[1] <= hole[3];
        let mut accum = [0.0f32; 3];
        let mut counted = 0;
        for j in 0..n {
            for i in 0..n {
                let t = [(i as f32 + 0.5) / n as f32 * 2.0 - 1.0, (j as f32 + 0.5) / n as f32 * 2.0 - 1.0];
                let cell = [t[0] * hs[0], t[1] * hs[1]];
                let dest_pos = [k.dest_centre[0] + cell[0], k.dest_centre[1] + cell[1]];
                let src_pos = [cand[0] + cell[0], cand[1] + cell[1]];
                if inside(dest_pos) || inside(src_pos) {
                    continue;
                }
                let d = sampled(source, dest_pos);
                let s = sampled(source, src_pos);
                for c in 0..3 {
                    let diff = d[c] - s[c];
                    accum[c] += (diff * diff) / (d[c] * d[c] + 0.001);
                }
                counted += 1;
            }
        }
        if counted == 0 {
            return [f32::INFINITY, offset[0], offset[1], 1.0];
        }
        [(accum[0] + accum[1] + accum[2]) / counted as f32, offset[0], offset[1], 1.0]
    })
}

pub fn correction_bias_field(patch: &Image, k: &CorrectionBiasField) -> Image {
    image_map(patch, |x, y, p| {
        let pos = patch.ci_coord(x, y);
        let bias = map(k.base, |c, b| b + k.grad_x[c] * (pos[0] - k.origin[0]) + k.grad_y[c] * (pos[1] - k.origin[1]));
        with_alpha(map(rgb(p), |c, v| v + bias[c]), p[3])
    })
}

// MARK: - Core Image built-ins

pub fn color_matrix(s: V4, k: &ColorMatrix) -> V4 {
    let rows = [k.r_vector, k.g_vector, k.b_vector, k.a_vector];
    std::array::from_fn(|c| dot4(s, rows[c]) + k.bias_vector[c])
}

pub fn color_clamp(s: V4, k: &ColorClamp) -> V4 {
    std::array::from_fn(|c| clamp(s[c], k.min_components[c], k.max_components[c]))
}

pub fn exposure_adjust(s: V4, k: &ExposureAdjust) -> V4 {
    let g = k.ev.exp2();
    with_alpha(map(rgb(s), |_, v| v * g), s[3])
}

pub fn blend_with_mask(image: V4, background: V4, mask: V4) -> V4 {
    std::array::from_fn(|c| mix(background[c], image[c], mask[1]))
}

pub fn source_over(image: V4, background: V4) -> V4 {
    std::array::from_fn(|c| image[c] + background[c] * (1.0 - image[3]))
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

fn area(image: &Image, rect: Rect) -> impl Iterator<Item = V4> + '_ {
    (rect.y..rect.y + rect.height).flat_map(move |y| (rect.x..rect.x + rect.width).map(move |x| image.at(x, y)))
}

/// `CIAreaAverage`, summed in double precision.
pub fn area_average(image: &Image, rect: Rect) -> V4 {
    let mut sum = [0f64; 4];
    for p in area(image, rect) {
        for c in 0..4 {
            sum[c] += p[c] as f64;
        }
    }
    let n = (rect.width * rect.height) as f64;
    sum.map(|v| (v / n) as f32)
}

pub fn area_maximum(image: &Image, rect: Rect) -> V4 {
    area(image, rect).fold([f32::NEG_INFINITY; 4], |m, p| std::array::from_fn(|c| m[c].max(p[c])))
}

/// `CIAreaHistogram`'s bin for `v`: `floor(v * bins)`, held on the first and the last.
pub fn histogram_bin(v: f32, bins: usize) -> usize {
    clamp((v * bins as f32).floor(), 0.0, (bins - 1) as f32) as usize
}

/// The whole-number counts behind `area_histogram`.
pub fn histogram_counts(image: &Image, rect: Rect, bins: usize) -> Vec<[u64; 4]> {
    let mut counts = vec![[0u64; 4]; bins];
    for p in area(image, rect) {
        for c in 0..4 {
            counts[histogram_bin(p[c], bins)][c] += 1;
        }
    }
    counts
}

pub fn area_histogram(image: &Image, rect: Rect, k: &AreaHistogram) -> Vec<V4> {
    let n = (rect.width * rect.height) as f64;
    histogram_counts(image, rect, k.count)
        .into_iter()
        .map(|c| c.map(|v| (v as f64 / n * k.scale as f64) as f32))
        .collect()
}
