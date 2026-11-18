//! The per-channel windows, each on the density axis, five handles each; and a digital photo's own
//! five windows on the picture, laid in the curves' tables.

use crate::axis::{density_axis, graduation, pipeline};
use crate::modes::ConversionMode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LevelsChannel {
    Luma,
    Linked,
    Red,
    Green,
    Blue,
}

impl LevelsChannel {
    pub const ALL: [LevelsChannel; 5] = [Self::Luma, Self::Linked, Self::Red, Self::Green, Self::Blue];
    pub const PER_CHANNEL: [LevelsChannel; 3] = [Self::Red, Self::Green, Self::Blue];

    pub fn label(self) -> &'static str {
        match self {
            Self::Luma => "Luma",
            Self::Linked => "RGB",
            Self::Red => "R",
            Self::Green => "G",
            Self::Blue => "B",
        }
    }

    /// The Levels editor's tabs on a digital photo, in the Curves' order.
    pub const PICTURE: [LevelsChannel; 5] = [Self::Linked, Self::Luma, Self::Red, Self::Green, Self::Blue];

    /// True where the handles carry a density: a film's three per-channel windows. A digital photo has
    /// no density, so its windows lie on the picture, 0 to 1.
    pub fn is_density(self, mode: ConversionMode) -> bool {
        mode.is_film() && Self::PER_CHANNEL.contains(&self)
    }

    pub fn histogram_channel(self) -> Option<usize> {
        match self {
            Self::Red => Some(0),
            Self::Green => Some(1),
            Self::Blue => Some(2),
            Self::Luma | Self::Linked => None,
        }
    }

    /// The domain the track expresses: on a density, the graduation window, which begins under zero.
    pub fn domain(self, mode: ConversionMode) -> (f32, f32) {
        if self.is_density(mode) {
            let w = graduation::window(mode);
            (*w.start(), *w.end())
        } else {
            (0.0, 1.0)
        }
    }

    /// Where a value sits on the track, 0 to 1: the graduation's own fraction.
    pub fn position_of_value(self, value: f32, mode: ConversionMode) -> f32 {
        let (low, high) = self.domain(mode);
        (value - low) / (high - low)
    }

    pub fn value_at_position(self, position: f32, mode: ConversionMode) -> f32 {
        let (low, high) = self.domain(mode);
        low + position * (high - low)
    }
}

/// One set of levels. The two added points are stored normalised between black and white.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Levels {
    pub black: f32,
    pub white: f32,
    pub mid: f32,
    pub shadows: f32,
    pub highlights: f32,
}

impl Default for Levels {
    fn default() -> Self {
        Self { black: 0.0, white: 1.0, mid: 0.5, shadows: 0.25, highlights: 0.75 }
    }
}

impl Levels {
    pub const EPSILON: f32 = 0.03;
    pub const MID_RANGE: (f32, f32) = (0.15, 0.85);
    pub const SHADOW_RANGE: (f32, f32) = (0.05, 0.45);
    pub const HIGHLIGHT_RANGE: (f32, f32) = (0.55, 0.95);
    /// Undoes stage 5's logarithm, which a positive carries too.
    pub const POSITIVE_MID: f32 = 0.67;

    pub fn with_ends(black: f32, white: f32) -> Self {
        Self { black, white, ..Self::default() }
    }

    /// The window that reads the density axis without placing anything on it.
    pub fn on_axis() -> Self {
        Self::with_ends(0.0, pipeline::DMAX)
    }

    pub fn resting_mid(mode: ConversionMode) -> f32 {
        if mode.reads_positive() { Self::POSITIVE_MID } else { 0.67 }
    }

    /// Where the pedestal's veil lands once the positive branch re-inverts; a negative reads 0.
    pub fn resting_black(mode: ConversionMode, ceiling: f32) -> f32 {
        if !mode.reads_positive() {
            return 0.0;
        }
        graduation::window(ConversionMode::Positive)
            .start()
            .max(density_axis::guard_offset() - (ceiling - density_axis::base()))
    }

    pub fn gamma(&self) -> f32 {
        libm::logf(0.5) / libm::logf(self.mid.clamp(Self::MID_RANGE.0, Self::MID_RANGE.1))
    }

    pub fn shadow_ordinate(&self) -> f32 {
        0.5 - self.shadows.clamp(Self::SHADOW_RANGE.0, Self::SHADOW_RANGE.1)
    }

    pub fn highlight_ordinate(&self) -> f32 {
        1.5 - self.highlights.clamp(Self::HIGHLIGHT_RANGE.0, Self::HIGHLIGHT_RANGE.1)
    }

    /// True where the two added points sit on their base ordinates, the exact identity.
    pub fn is_window_neutral(&self) -> bool {
        self.shadow_ordinate() == 0.25 && self.highlight_ordinate() == 0.75
    }

    /// The value that renders as mid-grey, not where the median's handle is drawn.
    pub fn display_mid(&self) -> f32 {
        self.black + self.mid * (self.white - self.black)
    }

    pub fn display_shadows(&self) -> f32 {
        self.black + self.shadows * (self.white - self.black)
    }

    pub fn display_highlights(&self) -> f32 {
        self.black + self.highlights * (self.white - self.black)
    }

    /// The quarter point at a value, held to its travel; a closed window leaves it.
    pub fn set_display_shadows(&mut self, v: f32) {
        let span = self.white - self.black;
        if span > 0.0 {
            self.shadows = ((v - self.black) / span).max(Self::SHADOW_RANGE.0).min(Self::SHADOW_RANGE.1);
        }
    }

    pub fn set_display_highlights(&mut self, v: f32) {
        let span = self.white - self.black;
        if span > 0.0 {
            self.highlights = ((v - self.black) / span).max(Self::HIGHLIGHT_RANGE.0).min(Self::HIGHLIGHT_RANGE.1);
        }
    }

    pub fn set_black(&mut self, v: f32) {
        self.black = v.min(self.white - Self::EPSILON);
    }

    pub fn set_white(&mut self, v: f32) {
        self.white = v.max(self.black + Self::EPSILON);
    }

    /// The window on a value of the picture: nothing under the black, the film window's `B(t^γ)` up to
    /// the white, then a line at the slope it reached, so highlights past the white keep their gradation.
    pub fn transfer(&self, x: f64) -> f64 {
        if *self == Self::default() {
            return x;
        }
        let (black, white) = (f64::from(self.black), f64::from(self.white));
        if x <= black {
            return x.min(0.0);
        }
        if x > white {
            return 1.0 + self.end_slope() * (x - white);
        }
        let t = libm::pow((x - black) / (white - black).max(1e-4), self.gamma_f64());
        if self.is_window_neutral() {
            return t;
        }
        let (c1, c3) = (f64::from(self.shadow_ordinate()), f64::from(self.highlight_ordinate()));
        let v = 1.0 - t;
        c1 * 4.0 * t * v * v * v + 0.5 * 6.0 * t * t * v * v + c3 * 4.0 * t * t * t * v + t * t * t * t
    }

    /// The slope the window reaches at its white, `γ / (w − b) · 4(1 − c3)`: 1 at rest.
    pub fn end_slope(&self) -> f64 {
        if *self == Self::default() {
            return 1.0;
        }
        let span = f64::from(self.white - self.black).max(1e-4);
        let bend = if self.is_window_neutral() { 1.0 } else { 4.0 * (1.0 - f64::from(self.highlight_ordinate())) };
        self.gamma_f64() / span * bend
    }

    fn gamma_f64(&self) -> f64 {
        libm::log(0.5) / libm::log(f64::from(self.mid.clamp(Self::MID_RANGE.0, Self::MID_RANGE.1)))
    }
}

/// A digital photo's own windows on the picture, each resting at `Levels::default()`: RGB on all three
/// channels, Luma as one gain on the three, R, G and B each on its own.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PictureLevels {
    pub linked: Levels,
    pub luma: Levels,
    pub red: Levels,
    pub green: Levels,
    pub blue: Levels,
}

impl PictureLevels {
    pub fn get(&self, channel: LevelsChannel) -> Levels {
        match channel {
            LevelsChannel::Linked => self.linked,
            LevelsChannel::Luma => self.luma,
            LevelsChannel::Red => self.red,
            LevelsChannel::Green => self.green,
            LevelsChannel::Blue => self.blue,
        }
    }

    pub fn get_mut(&mut self, channel: LevelsChannel) -> &mut Levels {
        match channel {
            LevelsChannel::Linked => &mut self.linked,
            LevelsChannel::Luma => &mut self.luma,
            LevelsChannel::Red => &mut self.red,
            LevelsChannel::Green => &mut self.green,
            LevelsChannel::Blue => &mut self.blue,
        }
    }

    pub fn set(&mut self, channel: LevelsChannel, levels: Levels) {
        *self.get_mut(channel) = levels;
    }

    pub fn is_neutral(&self) -> bool {
        *self == Self::default()
    }

    /// The windows a hand moved off their rest.
    pub fn touched(&self) -> Vec<LevelsChannel> {
        LevelsChannel::PICTURE.into_iter().filter(|c| self.get(*c) != Levels::default()).collect()
    }
}

/// The three per-channel windows; their resting white is the top of the density axis.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LevelsSet {
    pub red: Levels,
    pub green: Levels,
    pub blue: Levels,
}

impl Default for LevelsSet {
    fn default() -> Self {
        let rest = Levels::with_ends(0.0, pipeline::DMAX);
        Self { red: rest, green: rest, blue: rest }
    }
}

impl LevelsSet {
    pub fn get(&self, channel: LevelsChannel) -> Levels {
        match channel {
            LevelsChannel::Red => self.red,
            LevelsChannel::Green => self.green,
            LevelsChannel::Blue => self.blue,
            LevelsChannel::Linked | LevelsChannel::Luma => Levels::default(),
        }
    }

    pub fn get_mut(&mut self, channel: LevelsChannel) -> Option<&mut Levels> {
        match channel {
            LevelsChannel::Red => Some(&mut self.red),
            LevelsChannel::Green => Some(&mut self.green),
            LevelsChannel::Blue => Some(&mut self.blue),
            LevelsChannel::Linked | LevelsChannel::Luma => None,
        }
    }

    pub fn set(&mut self, channel: LevelsChannel, levels: Levels) {
        if let Some(window) = self.get_mut(channel) {
            *window = levels;
        }
    }

    /// The per-channel windows a hand moved off their rest.
    pub fn touched(&self, mode: ConversionMode, ceiling: f32) -> Vec<LevelsChannel> {
        LevelsChannel::PER_CHANNEL.into_iter().filter(|c| self.get(*c) != Self::resting(*c, mode, ceiling)).collect()
    }

    pub fn windows(&self) -> [Levels; 3] {
        [self.red, self.green, self.blue]
    }

    pub fn resting(channel: LevelsChannel, mode: ConversionMode, ceiling: f32) -> Levels {
        let mut out = LevelsSet::default().get(channel);
        if LevelsChannel::PER_CHANNEL.contains(&channel) {
            out.black = Levels::resting_black(mode, ceiling);
        }
        out
    }

    pub fn neutral(mode: ConversionMode, ceiling: f32) -> LevelsSet {
        let mut out = LevelsSet::default();
        for channel in LevelsChannel::PER_CHANNEL {
            if let Some(window) = out.get_mut(channel) {
                *window = Self::resting(channel, mode, ceiling);
            }
        }
        out
    }
}
