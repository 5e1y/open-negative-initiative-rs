//! The screen gamut zebra: the pixels of the proofed picture this screen cannot show, striped as
//! a camera's zebra, which a wider screen, a recent phone, shows more saturated than the canvas.

use moxcms::ColorProfile;

use crate::curve::Curve;
use crate::pcs::{self, Mat3};
use crate::working::WORKING_TO_PCS;

/// A hundredth past the display's range: under it a pixel is rounding, not a colour lost.
pub const TOLERANCE: f32 = 0.01;
/// In drawable pixels, so the stripes keep one size on screen whatever the zoom; a third of each
/// period is tint, which keeps the picture readable between them.
pub const PERIOD: f32 = 9.0;
pub const DUTY: f32 = 0.34;
/// One period every two seconds: slow enough to read as a mark, not as a flicker.
pub const SECONDS_PER_PERIOD: f64 = 2.0;
pub const FRAMES_PER_SECOND: u32 = 30;

/// The key's own tint, sRGB-encoded, so the stripes and the key can never differ.
pub const LIME: [f32; 3] = [0xC8 as f32 / 255.0, 0xF0 as f32 / 255.0, 0x3C as f32 / 255.0];

/// A truncated remainder, as the Swift clock reads it: a time under zero gives a phase under zero.
pub fn phase(time: f64) -> f32 {
    ((time / SECONDS_PER_PERIOD) % 1.0) as f32 * PERIOD
}

/// The tint in working light, as the kernel receives it.
pub fn tint() -> [f32; 4] {
    let srgb = ColorProfile::new_srgb();
    let decode = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    let xyz = pcs::mul_f32(&srgb.rgb_to_xyz_matrix().v, LIME.map(decode));
    let working = pcs::mul_f32(&crate::working::PCS_TO_WORKING, xyz);
    [working[0], working[1], working[2], 1.0]
}

/// A display read as its profile's matrix and curves, extended past [0, 1] as the system extends
/// a screen's space: the curve mirrored under zero and continued past one.
#[derive(Debug, Clone, PartialEq)]
pub struct Display {
    pub from_pcs: Mat3,
    /// Per channel, the linear values whose encoding is `−TOLERANCE` and `1 + TOLERANCE`.
    pub low: [f32; 3],
    pub high: [f32; 3],
}

impl Display {
    /// `None` for a display profile that is not matrix and curves.
    pub fn new(profile: &ColorProfile) -> Option<Display> {
        if !profile.is_matrix_shaper() {
            return None;
        }
        let from_pcs = pcs::inverse(&profile.rgb_to_xyz_matrix().v)?;
        let mut low = [0.0; 3];
        let mut high = [0.0; 3];
        for (c, trc) in [&profile.red_trc, &profile.green_trc, &profile.blue_trc].iter().enumerate() {
            let curve = Curve::new(trc.as_ref()?)?;
            let tolerance = TOLERANCE as f64;
            low[c] = -curve.eval(tolerance) as f32;
            high[c] = match &curve {
                // A sampled curve stops at one: continued along its last slope.
                Curve::Sampled(samples) if samples.len() > 1 => {
                    let h = 1.0 / (samples.len() - 1) as f64;
                    let slope = (curve.eval(1.0) - curve.eval(1.0 - h)) / h;
                    (curve.eval(1.0) + slope * tolerance) as f32
                }
                _ => curve.eval(1.0 + tolerance) as f32,
            };
        }
        Some(Display { from_pcs, low, high })
    }

    /// Working light to the display's linear light.
    pub fn from_working(&self) -> Mat3 {
        pcs::matmul(&self.from_pcs, &WORKING_TO_PCS)
    }

    pub fn beyond_linear(&self, linear: [f32; 3]) -> bool {
        (0..3).any(|c| linear[c] < self.low[c] || linear[c] > self.high[c])
    }

    pub fn beyond_xyz(&self, xyz: [f32; 3]) -> bool {
        self.beyond_linear(pcs::mul_f32(&self.from_pcs, xyz))
    }

    pub fn beyond_working(&self, rgb: [f32; 3]) -> bool {
        self.beyond_linear(pcs::mul_f32(&self.from_working(), rgb))
    }
}

/// True where the stripe's tint falls; `x` and `y` are drawable pixel centres, y up.
pub fn stripe(x: f32, y: f32, phase: f32) -> bool {
    (x + y + phase) % PERIOD < DUTY * PERIOD
}

/// The zebra laid over a working-space RGBA drawable, row 0 at its top.
pub fn striped(pixels: &mut [f32], width: usize, display: &Display, phase: f32) {
    let m = display.from_working();
    let tint = tint();
    let height = pixels.len() / 4 / width.max(1);
    for (i, p) in pixels.chunks_exact_mut(4).enumerate() {
        let (col, row) = (i % width, i / width);
        let shown = pcs::mul_f32(&m, [p[0], p[1], p[2]]);
        if !display.beyond_linear(shown) {
            continue;
        }
        if stripe(col as f32 + 0.5, (height - 1 - row) as f32 + 0.5, phase) {
            p.copy_from_slice(&tint);
        }
    }
}
