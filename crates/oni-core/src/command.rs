//! What a hand, a key or a phone asks of the open frame. Steps are relative, so two hands on two
//! devices add up.

use crate::knob::Knob;
use oni_link::scanner::{Effect, Letter, Machine};
use oni_settings::{ConversionMode, Face, PipelineSettings};

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// One knob at a value, clamped to its range.
    Set { knob: Knob, value: f32 },
    /// One knob moved by `by` from wherever it is.
    Step { knob: Knob, by: f32 },
    /// A structured edit a knob cannot carry, a curve or a crop, as the whole settings it leaves.
    Write(Box<PipelineSettings>),
    Mode(ConversionMode),
    /// The face the frame renders by and the Color tab edits: Pro's chain or the Scanner's keypad.
    Face(Face),
    /// A structured edit of the Color tab made on `EditorStore::face_view`, stored by the open face.
    Finish(Box<PipelineSettings>),
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

/// A phone's key as the store's command; a panel slider's step stays the wire's, for the host app
/// that knows its sliders to resolve.
impl TryFrom<oni_link::Command> for Command {
    type Error = oni_link::Command;

    fn try_from(command: oni_link::Command) -> Result<Command, oni_link::Command> {
        use oni_link::Command as Link;
        Ok(match command {
            Link::Step { letter, by } => Command::Scan(Scan::Step { letter, by }),
            Link::Reset { letter } => Command::Scan(Scan::Reset { letter }),
            Link::StepEffect { effect, by } => Command::Scan(Scan::StepEffect { effect, by }),
            Link::ResetEffect { effect } => Command::Scan(Scan::ResetEffect { effect }),
            Link::Machine(machine) => Command::Scan(Scan::Machine(machine)),
            Link::Walk { by } => Command::Scan(Scan::Walk { by }),
            Link::Undo => Command::Undo,
            Link::Redo => Command::Redo,
            Link::Face(face) => Command::Face(face_of(face)),
            Link::StepParam { .. } | Link::ResetParam { .. } | Link::SetParam { .. } | Link::ReleaseParam { .. } => {
                return Err(command);
            }
        })
    }
}

pub fn face_of(face: oni_link::Face) -> Face {
    match face {
        oni_link::Face::Pro => Face::Pro,
        oni_link::Face::Scanner => Face::Scanner,
    }
}

pub fn face_for(face: Face) -> oni_link::Face {
    match face {
        Face::Pro => oni_link::Face::Pro,
        Face::Scanner => oni_link::Face::Scanner,
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
            Command::Face(face) => Link::Face(face_for(face)),
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
            oni_link::Command::Face(oni_link::Face::Scanner),
        ];
        for key in keys {
            assert_eq!(Command::try_from(key).unwrap().to_link(), Some(key));
        }
        // Twin: a desktop-only command has no key on the phone, and a slider's step no store command.
        assert_eq!(Command::Reset.to_link(), None);
        let step = oni_link::Command::StepParam { param: oni_link::Param::Exposure, by: 1 };
        assert_eq!(Command::try_from(step), Err(step));
    }
}
