//! The Scanner face as the phone sees it: the keys it can press and the screen it draws.

use serde::{Deserialize, Serialize};

use crate::panel::{Face, Panel, Param};

pub use oni_keys::{Channel, Effect, Letter, Machine};

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
    /// The open photo's face; protocol 2 onward, as the two below.
    Face(Face),
    /// A panel slider moved by `by` hundredths of its track, as the desktop's arrow keys move it.
    StepParam { param: Param, by: i8 },
    /// A panel slider back where a double click returns it.
    ResetParam { param: Param },
    /// A panel slider's knob placed at `position` on its track, 0 to 1, by a hand holding it until
    /// `ReleaseParam`; protocol 3 onward, as `ReleaseParam`.
    SetParam { param: Param, position: f32 },
    /// The hand off the slider: its moves since its first `SetParam` make one step of the history.
    ReleaseParam { param: Param },
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
    /// Out of this struct's encoding, so a protocol 1 phone still reads it; `ToPhone::Screen` carries it.
    #[serde(skip)]
    pub panel: Panel,
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
            Command::Face(face) => next.panel.face = face,
            // The knob follows the hand at once; its readout is the desktop's to write.
            Command::SetParam { param, position } => {
                if let Some(row) = next.panel.rows.iter_mut().find(|row| row.param == param) {
                    row.position = position.clamp(0.0, 1.0);
                }
            }
            // A step's knob and readout are the desktop's to place, so a step waits for its screen.
            Command::StepParam { .. } | Command::ResetParam { .. } | Command::ReleaseParam { .. } => {}
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
