//! What a hand, a key or a phone asks of the open frame. Steps are relative, so two hands on two
//! devices add up.

use crate::knob::Knob;
use oni_keys::{Effect, Letter, Machine};
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

/// The Scanner face's keys, the same shapes the phone link's commands carry over the wire.
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
