//! What a hand, a key or a phone asks of the open frame. Steps are relative, so two hands on two
//! devices add up.

use crate::knob::Knob;
use oni_keys::{Effect, Letter, Machine};
use oni_settings::{ConversionMode, Face, PipelineSettings};
use std::sync::Arc;

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
    /// One edit laid on each frame through its own settings: a reset lands on each one's own rest.
    Each(Each),
    /// The bar's reset: back to the imported state, framing, flat-field and dye family kept.
    Reset,
    /// The scanner's keys, as the phone presses them.
    Scan(Scan),
    Undo,
    Redo,
}

/// An edit of the stored settings, read off the frame it lands on; two are equal when they are one.
#[derive(Clone)]
pub struct Each(Arc<dyn Fn(&PipelineSettings) -> PipelineSettings + Send + Sync>);

impl Each {
    pub fn new(edit: impl Fn(&PipelineSettings) -> PipelineSettings + Send + Sync + 'static) -> Each {
        Each(Arc::new(edit))
    }

    pub fn on(&self, settings: &PipelineSettings) -> PipelineSettings {
        (self.0)(settings)
    }
}

impl PartialEq for Each {
    fn eq(&self, other: &Each) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for Each {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Each")
    }
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
