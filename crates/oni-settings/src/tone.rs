//! Lighting: the RGB curve's sliders, each channel's filter, the scan's black and white points,
//! and the scalar replicas the migrations fit and fold through.

use crate::axis::pipeline;
use crate::levels::LevelsChannel;
use crate::tolerant;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

type V3 = [f32; 3];

/// The contrast slider's transfer function: a normalised logistic and its exact inverse.
pub mod contrast {
    pub const FULL_STEEPNESS: f32 = 15.2;
    const DEAD: f32 = 1e-4;

    pub fn apply(x: f32, travel: f32, pivot: f32) -> f32 {
        if pivot == 0.5 || (travel.abs() * FULL_STEEPNESS) <= DEAD {
            return symmetric(x, travel);
        }
        let warp = 0.5f32.ln() / pivot.ln();
        symmetric(x.clamp(0.0, 1.0).powf(warp), travel).powf(1.0 / warp)
    }

    fn symmetric(x: f32, travel: f32) -> f32 {
        let b = travel.abs() * FULL_STEEPNESS;
        if b <= DEAD {
            return x;
        }
        let low = logistic(0.0, b);
        let high = logistic(1.0, b);
        if travel > 0.0 {
            return (logistic(x, b) - low) / (high - low);
        }
        let g = low + x.clamp(0.0, 1.0) * (high - low);
        (0.5 - (1.0 / g - 1.0).ln() / b).clamp(0.0, 1.0)
    }

    fn logistic(x: f32, b: f32) -> f32 {
        1.0 / (1.0 + (-b * (x - 0.5)).exp())
    }
}

/// One travel raising every channel to a power and undoing it on the luminance alone.
pub mod color_density {
    use super::V3;
    use crate::axis::pipeline;

    pub const RANGE: (f32, f32) = (-1.0, 1.0);
    pub const BASE: f32 = 0.0;
    pub const FULL_POWER: f32 = 2.0;

    pub fn factor(travel: f32) -> f32 {
        FULL_POWER.powf(travel)
    }

    pub fn rendered(rgb: V3, travel: f32) -> V3 {
        let m = factor(travel);
        if m == 1.0 {
            return rgb;
        }
        let p = rgb.map(|c| c.max(0.0).powf(m));
        let y = pipeline::luma(p);
        if y <= 1e-12 {
            return rgb;
        }
        let k = y.powf((1.0 / m) - 1.0);
        p.map(|c| c * k)
    }
}

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
            let g = stops.exp2();
            let v = top.powf(Print::DISPLAY_GAMMA);
            p = (v * g / (1.0 + v * (g - 1.0))).powf(1.0 / Print::DISPLAY_GAMMA) + (p - top);
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
            bypassed: false,
        }
    }
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
    pub const HUNT_REACH: f32 = 4.0;
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

    /// The points as the windows take them: a sum past `1 − MINIMUM_SPAN` is held back.
    pub fn held_points(&self) -> (f32, f32) {
        let limit = 1.0 - Self::MINIMUM_SPAN;
        let (black, white) = (self.black_point, self.white_point);
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
            let t = n[i].max(0.0).powf(1.0 / g);
            out[i] = ((t - black) / (1.0 - black - white)).max(0.0).powf(g);
        }
        out
    }

    pub fn hunt_factor(&self, g: f32) -> f32 {
        (-(self.master().shaped(g, self.exposure) - g)).exp()
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
        p.max(0.0).powf(Self::DISPLAY_GAMMA)
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
                (tone.shaped(g * (1.0 + Self::CONTRAST_STEP), tone.exposure).max(1e-6).powf(Self::DISPLAY_GAMMA)
                    / tone.shaped(g * (1.0 - Self::CONTRAST_STEP), tone.exposure).max(1e-6).powf(Self::DISPLAY_GAMMA))
                .ln()
            };
            let reference = local(base);
            let k = if reference.abs() > 1e-6 { (local(self.master()) / reference).clamp(0.25, 4.0) } else { 1.0 };
            let grey = [self.output(g, LevelsChannel::Linked, true); 3];
            let share = k.powf(follow - 1.0).min(1.0);
            let held: V3 = std::array::from_fn(|i| (grey[i] + (after[i] - grey[i]) * share).max(0.0));
            let lit = pipeline::luma(held);
            if lit > 1e-6 {
                let scale = pipeline::luma(after) / lit;
                after = held.map(|c| c * scale);
            }
        }
        color_density::rendered(after, power.log2() / color_density::FULL_POWER.log2())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_contrast_past_the_widest_track_lands_on_its_stop() {
        let reopened = |c: f32| {
            let stored = Print { contrast: c, ..Print::default() };
            serde_json::from_value::<Print>(serde_json::to_value(stored).unwrap()).unwrap().contrast
        };
        assert_eq!(reopened(1.2), Print::MONOCHROME_CONTRAST_RANGE.1);
        assert_eq!(reopened(-0.9), Print::MONOCHROME_CONTRAST_RANGE.0);
        // Twin: values inside the track survive to the bit.
        assert_eq!(reopened(0.8), 0.8);
        assert_eq!(reopened(-0.3), -0.3);
    }

    #[test]
    fn a_stored_channel_reads_its_exposure_alone_held_to_a_stop() {
        let stored = r#"{"contrast":0.2,"red":{"exposure":2,"contrast":0.3,"mids":0.1},"blue":{"exposure":-0.4,"shadows":0.1}}"#;
        let decoded: Print = serde_json::from_str(stored).unwrap();
        assert_eq!(decoded.red, Tone::exposure(Print::FILTER_RANGE.1));
        assert_eq!(decoded.blue, Tone::exposure(-0.4));
        assert_eq!(decoded.green, Tone::NEUTRAL);
        assert_eq!(decoded.contrast, 0.2);
        assert_eq!(decoded.black_point, Print::RESTING_BLACK_POINT);
        // Twin: a Tone read on its own keeps every slider, so the narrowing above is Print's.
        let tone: Tone = serde_json::from_str(r#"{"exposure":2,"contrast":0.3,"mids":0.1}"#).unwrap();
        assert_eq!(tone.contrast, 0.3);
        assert_eq!(tone.mids, 0.1);
    }

    #[test]
    fn at_rest_the_print_is_the_diagonal() {
        let rest = Print::default();
        for v in [0.0f32, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0] {
            let out = rest.output(v, LevelsChannel::Linked, true).powf(1.0 / Print::DISPLAY_GAMMA);
            assert!((out - v).abs() < 1e-5, "{v} printed {out}");
        }
        // Twin: a stop of exposure leaves the diagonal.
        let lit = Print { exposure: 1.0, ..Print::default() };
        assert!((lit.output(0.5, LevelsChannel::Linked, true).powf(1.0 / 2.2) - 0.5).abs() > 0.05);
    }

    #[test]
    fn contrast_is_monotone_pinned_and_inverted_by_its_opposite() {
        for travel in [0.3f32, -0.3] {
            assert!(contrast::apply(0.0, travel, 0.5).abs() < 1e-6);
            assert!((contrast::apply(1.0, travel, 0.5) - 1.0).abs() < 1e-6);
        }
        let x = 0.37;
        let there = contrast::apply(x, 0.4, 0.5);
        let back = contrast::apply(there, -0.4, 0.5);
        assert!((back - x).abs() < 1e-5, "{back}");
        // Twin: a travel of the same sign does not undo it.
        assert!((contrast::apply(there, 0.4, 0.5) - x).abs() > 0.01);
    }
}
