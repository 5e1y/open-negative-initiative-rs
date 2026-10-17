//! What a hand, a key or a phone asks of the open frame. Steps are relative, so two hands on two
//! devices add up.

use crate::knob::Knob;
use oni_link::scanner::{Effect, Letter, Machine};
use oni_settings::{ConversionMode, PipelineSettings};

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// One knob at a value, clamped to its range.
    Set { knob: Knob, value: f32 },
    /// One knob moved by `by` from wherever it is.
    Step { knob: Knob, by: f32 },
    /// A structured edit a knob cannot carry, a curve or a crop, as the whole settings it leaves.
    Write(Box<PipelineSettings>),
    Mode(ConversionMode),
    /// The bar's reset: back to the imported state, framing, flat-field and dye family kept.
    Reset,
    /// The scanner's keys, as the phone presses them.
    Scan(Scan),
    Undo,
    Redo,
}

/// The Scanner face's keys, the same shapes `oni_link::Command` carries over the wire.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scan {
    Step { letter: Letter, by: i8 },
    Reset { letter: Letter },
    StepEffect { effect: Effect, by: i8 },
    ResetEffect { effect: Effect },
    Machine(Machine),
    /// Walks the open frame's group; the host opens the frame, the store has no list of them.
    Walk { by: i8 },
}

impl From<oni_link::Command> for Command {
    fn from(command: oni_link::Command) -> Command {
        use oni_link::Command as Link;
        match command {
            Link::Step { letter, by } => Command::Scan(Scan::Step { letter, by }),
            Link::Reset { letter } => Command::Scan(Scan::Reset { letter }),
            Link::StepEffect { effect, by } => Command::Scan(Scan::StepEffect { effect, by }),
            Link::ResetEffect { effect } => Command::Scan(Scan::ResetEffect { effect }),
            Link::Machine(machine) => Command::Scan(Scan::Machine(machine)),
            Link::Walk { by } => Command::Scan(Scan::Walk { by }),
            Link::Undo => Command::Undo,
            Link::Redo => Command::Redo,
        }
    }
}

impl Command {
    /// The wire's form of this command, where the phone has a key for it.
    pub fn to_link(&self) -> Option<oni_link::Command> {
        use oni_link::Command as Link;
        Some(match *self {
            Command::Scan(Scan::Step { letter, by }) => Link::Step { letter, by },
            Command::Scan(Scan::Reset { letter }) => Link::Reset { letter },
            Command::Scan(Scan::StepEffect { effect, by }) => Link::StepEffect { effect, by },
            Command::Scan(Scan::ResetEffect { effect }) => Link::ResetEffect { effect },
            Command::Scan(Scan::Machine(machine)) => Link::Machine(machine),
            Command::Scan(Scan::Walk { by }) => Link::Walk { by },
            Command::Undo => Link::Undo,
            Command::Redo => Link::Redo,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_phone_key_crosses_into_the_store_and_back_unchanged() {
        let keys = [
            oni_link::Command::Step { letter: Letter::M, by: -2 },
            oni_link::Command::Reset { letter: Letter::W },
            oni_link::Command::StepEffect { effect: Effect::Chrome, by: 1 },
            oni_link::Command::ResetEffect { effect: Effect::Vignette },
            oni_link::Command::Machine(Machine::Frontier),
            oni_link::Command::Walk { by: 1 },
            oni_link::Command::Undo,
            oni_link::Command::Redo,
        ];
        for key in keys {
            assert_eq!(Command::from(key).to_link(), Some(key));
        }
        // Twin: a desktop-only command has no key on the phone.
        assert_eq!(Command::Reset.to_link(), None);
    }
}
