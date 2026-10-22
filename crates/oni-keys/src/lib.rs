//! The Scanner face's keys, the one vocabulary the editor's store and the phone's link both speak:
//! the keypad's letters, the finishing keys, the machine and the film channel.

use serde::{Deserialize, Serialize};

/// The keypad's letters: half-steps of density on D, C, M and Y, then warmth on TEMP.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Letter {
    D,
    C,
    M,
    Y,
    W,
}

impl Letter {
    pub const ALL: [Letter; 5] = [Letter::D, Letter::C, Letter::M, Letter::Y, Letter::W];

    pub fn title(self) -> &'static str {
        match self {
            Letter::D => "D",
            Letter::C => "C",
            Letter::M => "M",
            Letter::Y => "Y",
            Letter::W => "TEMP",
        }
    }

    pub fn range(self) -> (i8, i8) {
        match self {
            Letter::W => (-10, 10),
            _ => (-24, 24),
        }
    }
}

/// The finishing keys that stand beside the letters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Effect {
    Contrast,
    Vignette,
    Chrome,
}

impl Effect {
    /// One click, and the range, as the desktop app sets them; its `State` always has the last word.
    pub fn step(self) -> f32 {
        match self {
            Effect::Contrast | Effect::Chrome => 0.05,
            Effect::Vignette => 0.1,
        }
    }

    pub fn range(self, channel: Channel) -> (f32, f32) {
        match (self, channel) {
            (Effect::Contrast, Channel::Monochrome) => (-0.5, 1.0),
            (Effect::Contrast, _) => (-0.5, 0.5),
            (Effect::Vignette, _) => (-2.0, 2.0),
            (Effect::Chrome, _) => (0.0, 1.5),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Machine {
    #[default]
    Oni,
    Frontier,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Channel {
    #[default]
    Negative,
    Positive,
    Monochrome,
}
