//! The per-channel windows, each on the density axis, five handles each.

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

    /// True where the handles carry a density: the three per-channel windows.
    pub fn is_density(self) -> bool {
        Self::PER_CHANNEL.contains(&self)
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
        if self.is_density() {
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

    /// Three decimals of density where values are densities, a share of the output otherwise.
    pub fn readout(self, value: f32) -> String {
        if self.is_density() { format!("{value:.3}") } else { format!("{:.1}%", value * 100.0) }
    }
}

/// One set of levels. The two added points are stored normalised between black and white.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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
        if mode == ConversionMode::Positive { Self::POSITIVE_MID } else { 0.67 }
    }

    /// Where the pedestal's veil lands once the positive branch re-inverts; a negative reads 0.
    pub fn resting_black(mode: ConversionMode, ceiling: f32) -> f32 {
        if mode != ConversionMode::Positive {
            return 0.0;
        }
        graduation::window(ConversionMode::Positive)
            .start()
            .max(density_axis::guard_offset() - (ceiling - density_axis::base()))
    }

    pub fn gamma(&self) -> f32 {
        0.5f32.ln() / self.mid.clamp(Self::MID_RANGE.0, Self::MID_RANGE.1).ln()
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

    /// Where a normalised position is held before the logarithm, which diverges at both ends.
    pub const ANCHOR_CLAMP: (f32, f32) = (0.02, 0.98);

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
}

/// The three per-channel windows; their resting white is the top of the density axis.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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
        if channel.is_density() {
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
