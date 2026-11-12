//! Every serialised settings type of Open Negative Initiative, the sidecar codec, each migration a
//! stored version still needs (`migrate`), and the roll marker, written as the Swift app writes them.

pub mod collation;
pub mod atomic;
pub mod axis;
pub mod blocks;
pub mod cg;
pub mod codec;
pub mod curves;
pub mod date;
pub mod exif;
pub mod film;
pub mod frame;
pub mod geometry;
pub mod id;
pub mod json;
mod kept;
pub mod levels;
pub mod migrate;
pub mod modes;
pub mod prepare;
mod reading;
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
pub use reading::Reading;
pub use settings::PipelineSettings;
pub use sidecar::{ExifMetadata, Loaded, RollDraft, TypedExif};
