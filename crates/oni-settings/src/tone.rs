//! Lighting: the RGB curve's sliders, each channel's filter, the scan's black and white points,
//! and the scalar replicas the migrations fit and fold through.

use crate::axis::pipeline;
use crate::levels::LevelsChannel;
use crate::tolerant;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

type V3 = [f32; 3];

pub mod color_density;
pub mod contrast;

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

impl<'de> Deserialize<'de> for Tone {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let map = tolerant::object(Value::deserialize(d)?, "Tone")?;
        let f = |key: &str| -> Result<f32, D::Error> { Ok(tolerant::opt::<f32, D::Error>(&map, key)?.unwrap_or(0.0)) };
        Ok(Tone {
            exposure: f("exposure")?,
            contrast: f("contrast")?,
            blacks: f("blacks")?,
            shadows: f("shadows")?,
            highlights: f("highlights")?,
            whites: f("whites")?,
            mids: f("mids")?,
        })
    }
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
    /// The RGB curve the chroma compensation reads a change of light from.
    #[serde(rename = "huntBase", skip_serializing_if = "Option::is_none")]
    pub hunt_base: Option<Tone>,
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
            hunt_base: None,
            black_point: Print::RESTING_BLACK_POINT,
            white_point: 0.0,
            channel_white: [0.0; 3],
            channel_black: [0.0; 3],
            bypassed: false,
        }
    }
}

fn at_rest(points: &V3) -> bool {
    *points == [0.0; 3]
}

/// Tolerant of absent keys; a contrast or black past the widest track lands on its stop, a channel
/// keeps its exposure alone and the hunt base its two bends.
impl<'de> Deserialize<'de> for Print {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let map = tolerant::object(Value::deserialize(d)?, "Print")?;
        let f = |key: &str, default: f32| -> Result<f32, D::Error> {
            Ok(tolerant::opt::<f32, D::Error>(&map, key)?.unwrap_or(default))
        };
        let t = |key: &str| -> Result<Option<Tone>, D::Error> { tolerant::opt::<Tone, D::Error>(&map, key) };
        let channels = |key: &str, (low, high): (f32, f32)| -> Result<V3, D::Error> {
            Ok(tolerant::opt::<V3, D::Error>(&map, key)?.unwrap_or_default().map(|p| p.clamp(low, high)))
        };
        let filter = |tone: Tone| Tone::exposure(tone.exposure.clamp(Print::FILTER_RANGE.0, Print::FILTER_RANGE.1));
        let hunt_base = t("huntBase")?.map(|base| Tone {
            exposure: base.exposure,
            contrast: base.contrast,
            shadows: base.shadows,
            highlights: base.highlights,
            ..Tone::NEUTRAL
        });
        Ok(Print {
            exposure: f("exposure", 0.0)?,
            contrast: f("contrast", 0.0)?
                .clamp(Print::MONOCHROME_CONTRAST_RANGE.0, Print::MONOCHROME_CONTRAST_RANGE.1),
            shadows: f("shadows", 0.0)?,
            highlights: f("highlights", 0.0)?,
            red: filter(t("red")?.unwrap_or_default()),
            green: filter(t("green")?.unwrap_or_default()),
            blue: filter(t("blue")?.unwrap_or_default()),
            hunt_base,
            black_point: f("blackPoint", Print::RESTING_BLACK_POINT)?
                .clamp(Print::MONOCHROME_POINT_RANGE.0, Print::MONOCHROME_POINT_RANGE.1),
            white_point: f("whitePoint", 0.0)?,
            channel_white: channels("channelWhite", Print::POINT_RANGE)?,
            channel_black: channels("channelBlack", Print::BLACK_POINT_RANGE)?,
            bypassed: false,
        })
    }
}

impl Print {
    pub const EXPOSURE_RANGE: (f32, f32) = (-4.0, 4.0);
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
    pub const FOLLOW: f32 = 0.0;
    pub const CONTRAST_STEP: f32 = 0.05;

    pub fn resting_exposure(mode: crate::modes::ConversionMode) -> f32 {
        if mode == crate::modes::ConversionMode::Positive { 0.0 } else { Self::RESTING_EXPOSURE }
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

    /// What the kernel prints for output `n`, as linear light; `bent` off leaves the RGB curve out.
    pub fn output(&self, n: f32, channel: LevelsChannel, bent: bool) -> f32 {
        let mut p = if bent {
            self.master().shaped(n, self.exposure)
        } else {
            self.hunt_base.map_or(n, |b| b.shaped(n, b.exposure))
        };
        if channel != LevelsChannel::Linked && channel != LevelsChannel::Luma {
            p = Tone::NEUTRAL.shaped(p, self.channel(channel).exposure);
        }
        libm::powf(p.max(0.0), Self::DISPLAY_GAMMA)
    }

    /// The kernel's block on one pixel's window outputs.
    pub fn rendered(&self, n: V3, travel: f32) -> V3 {
        let follow = Self::FOLLOW;
        let lit = |bent: bool| {
            [
                self.output(n[0], LevelsChannel::Red, bent),
                self.output(n[1], LevelsChannel::Green, bent),
                self.output(n[2], LevelsChannel::Blue, bent),
            ]
        };
        let mut after = lit(true);
        if self.filters() != [0.0; 3] {
            let plain = n.map(|v| self.output(v, LevelsChannel::Linked, true));
            let filtered = pipeline::luma(after);
            if filtered > 1e-6 {
                let k = pipeline::luma(plain) / filtered;
                after = after.map(|c| c * k);
            }
        }
        let base = self.hunt_base.unwrap_or(Tone::NEUTRAL);
        let g = (n[0] + n[1] + n[2]) / 3.0;
        let power = color_density::factor(travel) * self.hunt_factor(g);
        if follow < 1.0 && self.master() != base && g > 1e-3 {
            let local = |tone: Tone| {
                let shown = |v: f32| libm::powf(tone.shaped(v, tone.exposure).max(1e-6), Self::DISPLAY_GAMMA);
                libm::logf(shown(g * (1.0 + Self::CONTRAST_STEP)) / shown(g * (1.0 - Self::CONTRAST_STEP)))
            };
            let reference = local(base);
            let k = if reference.abs() > 1e-6 { (local(self.master()) / reference).clamp(0.25, 4.0) } else { 1.0 };
            let grey = [self.output(g, LevelsChannel::Linked, true); 3];
            let share = libm::powf(k, follow - 1.0).min(1.0);
            let held: V3 = std::array::from_fn(|i| (grey[i] + (after[i] - grey[i]) * share).max(0.0));
            let lit = pipeline::luma(held);
            if lit > 1e-6 {
                let scale = pipeline::luma(after) / lit;
                after = held.map(|c| c * scale);
            }
        }
        color_density::rendered(after, libm::log2f(power) / libm::log2f(color_density::FULL_POWER))
    }
}

#[cfg(test)]
mod tests;
