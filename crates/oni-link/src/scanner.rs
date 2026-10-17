//! The Scanner face as the phone sees it: the keys it can press and the screen it draws.

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

/// What a key asks for. A step is relative, so two hands on two devices add up.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    Step { letter: Letter, by: i8 },
    Reset { letter: Letter },
    StepEffect { effect: Effect, by: i8 },
    ResetEffect { effect: Effect },
    Machine(Machine),
    Walk { by: i8 },
    Undo,
    Redo,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clicks {
    pub d: i8,
    pub c: i8,
    pub m: i8,
    pub y: i8,
    pub w: i8,
}

impl Clicks {
    pub fn get(&self, letter: Letter) -> i8 {
        match letter {
            Letter::D => self.d,
            Letter::C => self.c,
            Letter::M => self.m,
            Letter::Y => self.y,
            Letter::W => self.w,
        }
    }

    pub fn set(&mut self, letter: Letter, value: i8) {
        let (lo, hi) = letter.range();
        let value = value.clamp(lo, hi);
        match letter {
            Letter::D => self.d = value,
            Letter::C => self.c = value,
            Letter::M => self.m = value,
            Letter::Y => self.y = value,
            Letter::W => self.w = value,
        }
    }
}

/// The scanner's screen: everything the phone draws, sent whole after every change.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// Bumped by the desktop on every change, from whatever hand.
    pub revision: u64,
    /// The last of this phone's presses the desktop has applied.
    pub applied: u64,
    /// The open frame, one-based, and the count of its group; `None` with no photo open.
    pub frame: Option<(u16, u16)>,
    pub machine: Machine,
    pub channel: Channel,
    pub clicks: Clicks,
    pub contrast: f32,
    pub vignette: f32,
    /// Absent in black and white, where Chrome has no key.
    pub chrome: Option<f32>,
    pub remeasure: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

impl State {
    /// The screen as it will read once `command` lands, so the phone answers the finger at once.
    pub fn predicted(&self, command: Command) -> State {
        let mut next = self.clone();
        match command {
            Command::Step { letter, by } => next.clicks.set(letter, self.clicks.get(letter).saturating_add(by)),
            Command::Reset { letter } => next.clicks.set(letter, 0),
            Command::StepEffect { effect, by } => {
                if let Some(value) = next.effect_mut(effect) {
                    let (lo, hi) = effect.range(self.channel);
                    let clicks = (*value / effect.step()).round() + f32::from(by);
                    *value = (clicks * effect.step()).clamp(lo, hi);
                }
            }
            Command::ResetEffect { effect } => {
                if let Some(value) = next.effect_mut(effect) {
                    *value = 0.0;
                }
            }
            Command::Machine(machine) => next.machine = machine,
            Command::Walk { .. } | Command::Undo | Command::Redo => {}
        }
        next
    }

    fn effect_mut(&mut self, effect: Effect) -> Option<&mut f32> {
        match effect {
            Effect::Contrast => Some(&mut self.contrast),
            Effect::Vignette => Some(&mut self.vignette),
            Effect::Chrome => self.chrome.as_mut(),
        }
    }
}
