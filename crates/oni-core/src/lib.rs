//! The editing model, headless, that the desktop interface and the phone remote both drive: the
//! open frame's store and commands, undo, paste, presets, roll-wide gestures and the phone link.

pub mod balance;
pub mod command;
pub mod fields;
pub mod history;
pub mod knob;
pub mod link;
pub mod parameters;
pub mod preset_parameters;
pub mod presets;
pub mod store;

pub use command::{Command, Scan};
pub use fields::{Family, Field, FieldSet};
pub use knob::{Channel, Handle, Knob};
pub use store::{Applied, Archive, EditorStore, PasteCheck, Surface};
