//! Every serialised settings type of Open Negative Initiative, the sidecar codec with each migration
//! a stored version still needs, and the roll marker, written as the Swift app writes them.

pub mod collation;
pub mod axis;
pub mod blocks;
pub mod cg;
pub mod codec;
pub mod curves;
pub mod date;
pub mod density_migration;
pub mod film;
pub mod frame;
pub mod frontier_lab;
pub mod geometry;
pub mod id;
pub mod json;
pub mod legacy;
pub mod levels;
pub mod lighting_catch_up;
pub mod modes;
pub mod scanner;
pub mod settings;
pub mod sidecar;
pub mod spectrogram;
pub mod tolerant;
pub mod tone;

pub use codec::{Decoded, Failure, SettingsDocument};
pub use date::Date;
pub use film::{Film, roll_folder};
pub use id::Id;
pub use modes::{ConversionMode, Face, Medal, ScannerMachine, SourceEncoding};
pub use settings::PipelineSettings;
pub use sidecar::{ExifMetadata, Loaded, RollDraft};
