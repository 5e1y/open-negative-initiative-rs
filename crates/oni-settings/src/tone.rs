//! Lighting: the RGB curve's sliders, each channel's filter, the scan's black and white points,
//! and the scalar replicas the migrations fit and fold through.

use crate::axis::pipeline;
use crate::levels::LevelsChannel;
use serde::Serialize;

type V3 = [f32; 3];

pub mod color_density;
pub mod contrast;
mod read;

/// One curve's sliders, each brightening or steepening to the right.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct Tone {
    pub exposure: f32,
    pub contrast: f32,
    pub blacks: f32,
    pub shadows: f32,
    pub highlights: f32,
    pub whites: f32,
    pub mids: f32,
}

impl Tone {
    pub const NEUTRAL: Tone =
        Tone { exposure: 0.0, contrast: 0.0, blacks: 0.0, shadows: 0.0, highlights: 0.0, whites: 0.0, mids: 0.0 };

    pub fn exposure(exposure: f32) -> Tone {
        Tone { exposure, ..Tone::NEUTRAL }
    }

    pub fn is_neutral(&self) -> bool {
        *self == Tone::NEUTRAL
    }

    fn toe(p: f32, travel: f32) -> f32 {
        let r = 1.0 - p;
        if travel < 0.0 { travel * p * r * r * r } else { 2.5 * travel * p * r * r * r * r * r }
    }

    pub const PUSH_REACH: f32 = 0.35;
    pub const PUSH_SOFTNESS: f32 = 0.15;

    fn pushed(p: f32, travel: f32) -> f32 {
        if travel == 0.0 {
            return p;
        }
        let e = Self::PUSH_SOFTNESS * Self::PUSH_REACH;
        let b = -2.0 * e + (Self::PUSH_REACH + 2.0 * e) * travel.abs().sqrt();
        let h = |x: f32| {
            let d = x - b;
            let s = (d * d + e * e).sqrt();
            if d >= 0.0 { (d + s) / 2.0 } else { e * e / (2.0 * (s - d)) }
        };
        let condensed = |x: f32| (h(x) - h(0.0)) / (h(1.0) - h(0.0));
        if travel < 0.0 { condensed(p) } else { 1.0 - condensed(1.0 - p) }
    }

    /// One pass up to a quarter of travel, a second pass bending what the first left past it.
    pub fn quartered(p: f32, travel: f32, highlights: bool) -> f32 {
        let pass = |p: f32, t: f32| {
            if highlights {
                p + t * 4.0 * p * p * p * (1.0 - p)
            } else {
                p + t * 4.0 * p * (1.0 - p) * (1.0 - p) * (1.0 - p)
            }
        };
        if travel.abs() <= 0.25 {
            return pass(p, travel);
        }
        let first = if travel > 0.0 { 0.25 } else { -0.25 };
        pass(pass(p, first), travel - first)
    }

    /// The kernel's `shaped`, display value in and out; `stops` apart so the Hunt ratio can omit it.
    pub fn shaped(&self, value: f32, stops: f32) -> f32 {
        let held = value.clamp(0.0, 1.0);
        let mut p = contrast::apply(held, self.contrast, Print::CONTRAST_PIVOT) + (value - held);
        if stops != 0.0 && p > 0.0 {
            let top = p.min(1.0);
            let g = libm::exp2f(stops);
            let v = libm::powf(top, Print::DISPLAY_GAMMA);
            p = libm::powf(v * g / (1.0 + v * (g - 1.0)), 1.0 / Print::DISPLAY_GAMMA) + (p - top);
        }
        let bends = self.blacks != 0.0
            || self.shadows != 0.0
            || self.highlights != 0.0
            || self.whites != 0.0
            || self.mids != 0.0;
        if bends && (0.0..=1.0).contains(&p) {
            p = if self.blacks < 0.0 { Self::pushed(p, self.blacks) } else { p + Self::toe(p, self.blacks) };
            p = if self.whites > 0.0 { Self::pushed(p, self.whites) } else { p - Self::toe(1.0 - p, -self.whites) };
            p = Self::quartered(p, self.shadows, false);
            p = Self::quartered(p, self.highlights, true);
            p += self.mids * 16.0 * p * p * (1.0 - p) * (1.0 - p);
        }
        p
    }

    /// Every slider multiplied and held on its own track.
    pub fn scaled(&self, a: f32) -> Tone {
        let held = |v: f32, r: (f32, f32)| (v * a).clamp(r.0, r.1);
        Tone {
            exposure: held(self.exposure, Print::EXPOSURE_RANGE),
            contrast: held(self.contrast, Print::CONTRAST_RANGE),
            blacks: held(self.blacks, Print::END_RANGE),
            shadows: held(self.shadows, Print::ZONE_RANGE),
            highlights: held(self.highlights, Print::ZONE_RANGE),
            whites: held(self.whites, Print::END_RANGE),
            mids: held(self.mids, Print::ZONE_RANGE),
        }
    }
}

/// Lighting: the window's output on a display gamma, bent by the RGB curve, then each channel's exposure.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Print {
    pub exposure: f32,
    pub contrast: f32,
    pub shadows: f32,
    pub highlights: f32,
    pub red: Tone,
    pub green: Tone,
    pub blue: Tone,
    #[serde(rename = "blackPoint")]
    pub black_point: f32,
    #[serde(rename = "whitePoint")]
    pub white_point: f32,
    /// Red's, green's and blue's own white points, laid on `white_point` in its units.
    #[serde(rename = "channelWhite", skip_serializing_if = "at_rest")]
    pub channel_white: V3,
    /// Each channel's own black point, laid on `black_point` in its units.
    #[serde(rename = "channelBlack", skip_serializing_if = "at_rest")]
    pub channel_black: V3,
    /// Each channel's own mids, bent after Lighting's curve as its Mids slider bends.
    #[serde(rename = "channelMid", skip_serializing_if = "at_rest")]
    pub channel_mid: V3,
    /// Each channel's own contrast, turned about Lighting's pivot after its curve.
    #[serde(rename = "channelContrast", skip_serializing_if = "at_rest")]
    pub channel_contrast: V3,
    /// The colour the print lays in answer to Lighting's tones: Hunt's density, the points' and Contrast's saturation
    /// curves and the points' colour hold; off, the sliders bend each channel's tone alone.
    #[serde(rename = "colourCompensation", skip_serializing_if = "is_on")]
    pub colour_compensation: bool,
    /// A paper's print brightened between its own Dmax and white, both held: an odds gain at `EXPOSURE_GEAR` times its
    /// stops, as ONI's print takes Exposure; none on ONI's print, whose Exposure is that curve.
    #[serde(skip_serializing_if = "at_zero")]
    pub brightness: f32,
    /// A paper's Dmax laid on pure black, its white kept: black point compensation, for a print on a screen's black.
    #[serde(rename = "trueBlacks", skip_serializing_if = "is_off")]
    pub true_blacks: bool,
    /// Off for a measurement upstream; never serialised.
    #[serde(skip)]
    pub bypassed: bool,
}

impl Default for Print {
    fn default() -> Self {
        Print {
            exposure: 0.0,
            contrast: 0.0,
            shadows: 0.0,
            highlights: 0.0,
            red: Tone::NEUTRAL,
            green: Tone::NEUTRAL,
            blue: Tone::NEUTRAL,
            black_point: Print::RESTING_BLACK_POINT,
            white_point: 0.0,
            channel_white: [0.0; 3],
            channel_black: [0.0; 3],
            channel_mid: [0.0; 3],
            channel_contrast: [0.0; 3],
            colour_compensation: true,
            brightness: 0.0,
            true_blacks: false,
            bypassed: false,
        }
    }
}

fn at_rest(points: &V3) -> bool {
    *points == [0.0; 3]
}

fn is_on(on: &bool) -> bool {
    *on
}

fn is_off(on: &bool) -> bool {
    !*on
}

fn at_zero(v: &f32) -> bool {
    *v == 0.0
}

impl Print {
    pub const EXPOSURE_RANGE: (f32, f32) = (-4.0, 4.0);
    pub const BRIGHTNESS_RANGE: (f32, f32) = (-4.0, 4.0);
    pub const FILTER_RANGE: (f32, f32) = (-1.5, 1.5);
    pub const TEMPERATURE_AXIS: V3 = [0.5, 0.0, -0.5];
    pub const TINT_AXIS: V3 = [1.0 / 3.0, -2.0 / 3.0, 1.0 / 3.0];
    pub const TEMPERATURE_RANGE: (f32, f32) = (-2.0, 2.0);
    pub const TINT_RANGE: (f32, f32) = (-1.5, 1.5);
    pub const CONTRAST_RANGE: (f32, f32) = (-0.5, 0.5);
    pub const END_RANGE: (f32, f32) = (-1.0, 1.0);
    pub const ZONE_RANGE: (f32, f32) = (-0.25, 0.25);
    pub const QUARTER_RANGE: (f32, f32) = (-0.5, 0.5);
    pub const DISPLAY_GAMMA: f32 = 2.2;
    /// Contrast turns on 0.18 of light on the display gamma, the kernel's `ContrastPivot`.
    pub const CONTRAST_PIVOT: f32 = 0.45865645;
    pub const POINT_RANGE: (f32, f32) = (-1.0 / 3.0, 1.0 / 3.0);
    pub const BLACK_POINT_RANGE: (f32, f32) = (-0.2, 0.2);
    pub const MONOCHROME_POINT_RANGE: (f32, f32) = (-0.5, 0.5);
    pub const MONOCHROME_CONTRAST_RANGE: (f32, f32) = (-0.5, 1.0);
    pub const MINIMUM_SPAN: f32 = 0.02;
    pub const POINT_SATURATION_GAIN: f32 = 2.25;
    pub const RESTING_BLACK_POINT: f32 = 0.0;
    /// A negative's exposure at rest: the roll's window holds its brightest frame. A slide rests at 0.
    pub const RESTING_EXPOSURE: f32 = 1.5;
    pub const HUNT_EXPONENT: f32 = 0.19;

    /// The print's odds gain takes Exposure's distance from its rest twice: near the rest the grey moves about
    /// a stop per stop, and the curve alone tapers the ends.
    pub const EXPOSURE_GEAR: f32 = 2.0;

    pub fn resting_exposure(mode: crate::modes::ConversionMode) -> f32 {
        if mode.reads_positive() { 0.0 } else { Self::RESTING_EXPOSURE }
    }

    /// The stops the print's curve takes for Exposure `exposure` resting at `rest`.
    pub fn curve_stops(exposure: f32, rest: f32) -> f32 {
        rest + Self::EXPOSURE_GEAR * (exposure - rest)
    }

    /// The Exposure whose curve takes `stops`, resting at `rest`: `curve_stops` undone.
    pub fn exposure_at(stops: f32, rest: f32) -> f32 {
        rest + (stops - rest) / Self::EXPOSURE_GEAR
    }

    /// This print with its Exposure in the curve's stops, as the kernel and its replicas read them.
    pub fn on_curve(&self, rest: f32) -> Print {
        Print { exposure: Self::curve_stops(self.exposure, rest), ..*self }
    }

    pub fn filters(&self) -> V3 {
        [self.red.exposure, self.green.exposure, self.blue.exposure]
    }

    pub fn set_filters(&mut self, f: V3) {
        self.red.exposure = f[0];
        self.green.exposure = f[1];
        self.blue.exposure = f[2];
    }

    pub fn master(&self) -> Tone {
        Tone {
            exposure: self.exposure,
            contrast: self.contrast,
            shadows: self.shadows,
            highlights: self.highlights,
            ..Tone::NEUTRAL
        }
    }

    pub fn set_master(&mut self, t: Tone) {
        self.exposure = t.exposure;
        self.contrast = t.contrast;
        self.shadows = t.shadows;
        self.highlights = t.highlights;
    }

    pub fn channel(&self, channel: LevelsChannel) -> Tone {
        match channel {
            LevelsChannel::Red => self.red,
            LevelsChannel::Green => self.green,
            LevelsChannel::Blue => self.blue,
            LevelsChannel::Linked | LevelsChannel::Luma => self.master(),
        }
    }

    /// What a channel's own pass bends after the RGB curve: its filter, its contrast and its mids.
    pub fn channel_tone(&self, channel: LevelsChannel) -> Tone {
        let Some(c) = LevelsChannel::PER_CHANNEL.iter().position(|p| *p == channel) else { return Tone::NEUTRAL };
        Tone { contrast: self.channel_contrast[c], mids: self.channel_mid[c], ..Tone::exposure(self.channel(channel).exposure) }
    }

    /// Whether a channel's own pass moves anything, so its curve differs from the RGB one.
    pub fn channel_moved(&self, channel: LevelsChannel) -> bool {
        !self.channel_tone(channel).is_neutral()
    }

    /// Lighting's pair as a window takes it.
    pub fn held_points(&self) -> (f32, f32) {
        Self::held_points_of(self.black_point, self.white_point)
    }

    /// Each channel's points as its window takes them, its own laid on the global pair and held; a
    /// black and white print shows no channel, so it reads the global pair alone.
    pub fn channel_points(&self, mono: bool) -> [(f32, f32); 3] {
        if mono {
            return [self.held_points(); 3];
        }
        std::array::from_fn(|c| Self::held_points_of(self.black_point + self.channel_black[c], self.white_point + self.channel_white[c]))
    }

    /// No point moves any window: the global pair at zero and, on colour, every channel's own.
    pub fn points_at_rest(&self, mono: bool) -> bool {
        self.black_point == 0.0 && self.white_point == 0.0 && (mono || (at_rest(&self.channel_white) && at_rest(&self.channel_black)))
    }

    /// `black` and `white` as a window takes them: a sum past `1 − MINIMUM_SPAN` is held back.
    pub fn held_points_of(black: f32, white: f32) -> (f32, f32) {
        let limit = 1.0 - Self::MINIMUM_SPAN;
        if black + white <= limit {
            return (black, white);
        }
        if black > 0.0 && white > 0.0 {
            let share = limit / (black + white);
            return (black * share, white * share);
        }
        if black > white { (limit - white, white) } else { (black, limit - black) }
    }

    /// The points on a window output `n` of gamma `g`: the catch-up's replica.
    pub fn pointed(&self, n: V3, gammas: V3) -> V3 {
        if self.black_point == 0.0 && self.white_point == 0.0 {
            return n;
        }
        let (black, white) = self.held_points();
        let mut out = n;
        for i in 0..3 {
            let g = gammas[i].max(1e-3);
            let t = libm::powf(n[i].max(0.0), 1.0 / g);
            out[i] = libm::powf(((t - black) / (1.0 - black - white)).max(0.0), g);
        }
        out
    }

    pub fn hunt_factor(&self, g: f32) -> f32 {
        libm::expf(-(self.master().shaped(g, self.exposure) - g))
    }

    /// What the kernel prints for output `n`, as linear light, its Exposure in the curve's stops (`on_curve`).
    pub fn output(&self, n: f32, channel: LevelsChannel) -> f32 {
        self.printed(n, channel, true)
    }

    /// `output` with or without the channel's filter, which the luma is held against.
    fn printed(&self, n: f32, channel: LevelsChannel, filtered: bool) -> f32 {
        let mut p = self.master().shaped(n, self.exposure);
        if channel != LevelsChannel::Linked && channel != LevelsChannel::Luma {
            let own = self.channel_tone(channel);
            p = own.shaped(p, if filtered { own.exposure } else { 0.0 });
        }
        libm::powf(p.max(0.0), Self::DISPLAY_GAMMA)
    }

    /// The kernel's block on one pixel's window outputs.
    pub fn rendered(&self, n: V3, travel: f32) -> V3 {
        let mut after: V3 = std::array::from_fn(|c| self.output(n[c], LevelsChannel::PER_CHANNEL[c]));
        if self.filters() != [0.0; 3] {
            let plain: V3 = std::array::from_fn(|c| self.printed(n[c], LevelsChannel::PER_CHANNEL[c], false));
            let filtered = pipeline::luma(after);
            if filtered > 1e-6 {
                let k = pipeline::luma(plain) / filtered;
                after = after.map(|c| c * k);
            }
        }
        let g = (n[0] + n[1] + n[2]) / 3.0;
        let hunt = if self.colour_compensation { self.hunt_factor(g) } else { 1.0 };
        let power = color_density::factor(travel) * hunt;
        color_density::rendered(after, libm::log2f(power) / libm::log2f(color_density::FULL_POWER))
    }
}

#[cfg(test)]
mod tests;
