//! The Scanner face (`scanner.metal`): density, the machine's output, its encoding.

use super::*;

/// `scannerDensity(s, gain, pedestal, logGuard, median, sepR, sepG, sepB, scale, shift, reachedLow,
/// reachedHigh, low, span, invert, dmax)`, as `ScannerRender.apply` passes them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScannerDensity {
    pub gain: V3,
    pub pedestal: f32,
    pub log_guard: f32,
    pub median: V3,
    pub sep_r: V3,
    pub sep_g: V3,
    pub sep_b: V3,
    pub scale: f32,
    pub shift: V3,
    pub reached_low: V3,
    pub reached_high: V3,
    pub low: f32,
    pub span: f32,
    pub invert: f32,
    pub dmax: f32,
}

impl ScannerDensity {
    pub fn rows(&self) -> Vec<V4> {
        vec![
            v3(self.gain, self.pedestal),
            v3(self.median, self.log_guard),
            v3(self.sep_r, self.scale),
            v3(self.sep_g, self.low),
            v3(self.sep_b, self.span),
            v3(self.shift, self.invert),
            v3(self.reached_low, self.dmax),
            v3(self.reached_high, 0.0),
        ]
    }
}

/// `scannerOutput(code, warmth, warmthShare, lift, contrast)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScannerOutput {
    pub warmth_share: f32,
    pub lift: f32,
    pub contrast: f32,
}

/// `ScannerRender.lift`: the warmth looks' moves stored raised by it.
pub const SCANNER_LIFT: f32 = 0.5;

/// `scannerEncode(s, root)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScannerEncode {
    pub root: f32,
}

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

const SCANNER_DENSITY: Shader = shader!("scanner_density");

const SCANNER_OUTPUT: Shader = shader!("scanner_output");

const SCANNER_ENCODE: Shader = shader!("scanner_encode");

impl Gpu {
    pub fn scanner_density(&self, image: &Image, k: &ScannerDensity) -> Image {
        self.per_pixel(SCANNER_DENSITY, |_| k.rows(), &[image], &[], 0)
    }

    pub fn scanner_output(&self, code: &Image, warmth: &Image, k: &ScannerOutput) -> Image {
        self.per_pixel(SCANNER_OUTPUT, |_| vec![[k.warmth_share, k.lift, k.contrast, 0.0]], &[code, warmth], &[], 0)
    }

    pub fn scanner_encode(&self, image: &Image, k: &ScannerEncode) -> Image {
        self.per_pixel(SCANNER_ENCODE, |_| vec![[k.root, 0.0, 0.0, 0.0]], &[image], &[], 0)
    }
}
