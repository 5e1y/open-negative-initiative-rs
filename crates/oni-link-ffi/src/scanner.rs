//! The keys and the screen as Swift and Kotlin see them, mirrored from `oni-link`.

use crate::panel::{Face, Panel, Param};

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum Letter {
    D,
    C,
    M,
    Y,
    /// Warmth, titled TEMP on the keypad.
    W,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum Effect {
    Contrast,
    Vignette,
    Chrome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum Machine {
    Oni,
    Frontier,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum Channel {
    Negative,
    Positive,
    Monochrome,
}

/// `SetMachine` rather than `Machine`: a Kotlin variant named after its field's type shadows that type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum Command {
    Step { letter: Letter, by: i8 },
    Reset { letter: Letter },
    StepEffect { effect: Effect, by: i8 },
    ResetEffect { effect: Effect },
    SetMachine { machine: Machine },
    Walk { by: i8 },
    Undo,
    Redo,
    SetFace { face: Face },
    /// A panel slider moved by `by` hundredths of its track, as the desktop's arrow keys move it.
    StepParam { param: Param, by: i8 },
    /// A panel slider back where a double click on the desktop returns it.
    ResetParam { param: Param },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, uniffi::Record)]
pub struct Clicks {
    pub d: i8,
    pub c: i8,
    pub m: i8,
    pub y: i8,
    pub w: i8,
}

/// The open frame, one-based, and the count of its group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct Frame {
    pub index: u16,
    pub count: u16,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct ScannerState {
    pub revision: u64,
    pub applied: u64,
    pub frame: Option<Frame>,
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

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct DisplayState {
    pub state: ScannerState,
    pub connected: bool,
    /// The last round trip to the desktop and back, in milliseconds.
    pub round_trip_ms: Option<f64>,
    /// The open photo's face and the sliders of the panel's tabs on it.
    pub panel: Panel,
    /// A press sent and not yet answered: what the screen shows of it is the phone's guess.
    pub unanswered: bool,
}

impl From<Letter> for oni_link::Letter {
    fn from(letter: Letter) -> Self {
        match letter {
            Letter::D => oni_link::Letter::D,
            Letter::C => oni_link::Letter::C,
            Letter::M => oni_link::Letter::M,
            Letter::Y => oni_link::Letter::Y,
            Letter::W => oni_link::Letter::W,
        }
    }
}

impl From<Effect> for oni_link::Effect {
    fn from(effect: Effect) -> Self {
        match effect {
            Effect::Contrast => oni_link::Effect::Contrast,
            Effect::Vignette => oni_link::Effect::Vignette,
            Effect::Chrome => oni_link::Effect::Chrome,
        }
    }
}

impl From<Machine> for oni_link::Machine {
    fn from(machine: Machine) -> Self {
        match machine {
            Machine::Oni => oni_link::Machine::Oni,
            Machine::Frontier => oni_link::Machine::Frontier,
        }
    }
}

impl From<oni_link::Machine> for Machine {
    fn from(machine: oni_link::Machine) -> Self {
        match machine {
            oni_link::Machine::Oni => Machine::Oni,
            oni_link::Machine::Frontier => Machine::Frontier,
        }
    }
}

impl From<oni_link::Channel> for Channel {
    fn from(channel: oni_link::Channel) -> Self {
        match channel {
            oni_link::Channel::Negative => Channel::Negative,
            oni_link::Channel::Positive => Channel::Positive,
            oni_link::Channel::Monochrome => Channel::Monochrome,
        }
    }
}

impl From<Command> for oni_link::Command {
    fn from(command: Command) -> Self {
        match command {
            Command::Step { letter, by } => oni_link::Command::Step { letter: letter.into(), by },
            Command::Reset { letter } => oni_link::Command::Reset { letter: letter.into() },
            Command::StepEffect { effect, by } => oni_link::Command::StepEffect { effect: effect.into(), by },
            Command::ResetEffect { effect } => oni_link::Command::ResetEffect { effect: effect.into() },
            Command::SetMachine { machine } => oni_link::Command::Machine(machine.into()),
            Command::Walk { by } => oni_link::Command::Walk { by },
            Command::Undo => oni_link::Command::Undo,
            Command::Redo => oni_link::Command::Redo,
            Command::SetFace { face } => oni_link::Command::Face(face.into()),
            Command::StepParam { param, by } => oni_link::Command::StepParam { param: param.into(), by },
            Command::ResetParam { param } => oni_link::Command::ResetParam { param: param.into() },
        }
    }
}

impl From<oni_link::Clicks> for Clicks {
    fn from(c: oni_link::Clicks) -> Self {
        Clicks { d: c.d, c: c.c, m: c.m, y: c.y, w: c.w }
    }
}

impl From<oni_link::State> for ScannerState {
    fn from(s: oni_link::State) -> Self {
        ScannerState {
            revision: s.revision,
            applied: s.applied,
            frame: s.frame.map(|(index, count)| Frame { index, count }),
            machine: s.machine.into(),
            channel: s.channel.into(),
            clicks: s.clicks.into(),
            contrast: s.contrast,
            vignette: s.vignette,
            chrome: s.chrome,
            remeasure: s.remeasure,
            can_undo: s.can_undo,
            can_redo: s.can_redo,
        }
    }
}

impl From<&oni_link::Display> for DisplayState {
    fn from(d: &oni_link::Display) -> Self {
        DisplayState {
            state: d.state.clone().into(),
            connected: d.connected,
            round_trip_ms: d.round_trip.map(|trip| trip.as_secs_f64() * 1000.0),
            panel: (&d.state.panel).into(),
            unanswered: d.unanswered,
        }
    }
}
