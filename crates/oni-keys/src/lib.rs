//! The Scanner face's keys, the one vocabulary the editor's store and the phone's link both speak:
//! the keypad's letters, the finishing keys, the machine and the film channel.

use serde::{Deserialize, Serialize};

/// The keypad's letters: density on D, C, M and Y, then warmth on TEMP, each counted in steps of half
/// the unit the desktop's screen reads.
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

    /// In steps: ±24 units on density, ±10 on TEMP.
    pub fn range(self) -> (i8, i8) {
        match self {
            Letter::W => (-20, 20),
            _ => (-48, 48),
        }
    }
}

/// Steps in one unit of the desktop's screen.
pub const STEPS_PER_UNIT: i64 = 2;

/// `LabScreen.spelled`: a letter's steps in the screen's unit, signed, a half step as ".5", zero bare.
pub fn spelled(steps: i64) -> String {
    let sign = match steps.signum() {
        0 => return "0".to_string(),
        -1 => "−",
        _ => "+",
    };
    let (whole, part) = (steps.unsigned_abs() / STEPS_PER_UNIT as u64, steps % STEPS_PER_UNIT != 0);
    format!("{sign}{whole}{}", if part { ".5" } else { "" })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_reads_half_a_unit_and_three_read_one_and_a_half() {
        assert_eq!([0, 1, -1, 3, -3, 2, 48, -20].map(spelled), ["0", "+0.5", "−0.5", "+1.5", "−1.5", "+1", "+24", "−10"]);
        // Twin: the steps read bare would show a whole unit for one press.
        assert_ne!(spelled(2), "+2");
    }
}
