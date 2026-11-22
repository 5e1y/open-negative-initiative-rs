//! Every serialised settings type of Open Negative Initiative, the sidecar's document, each migration
//! a stored version still needs (`migrate`), and the roll marker.

pub mod collation;
pub mod atomic;
pub mod axis;
pub mod blocks;
pub mod cg;
pub mod curves;
pub mod date;
pub mod document;
pub mod dyes;
pub mod exif;
pub mod film;
pub mod frame;
pub mod geometry;
pub mod id;
pub mod lens;
pub mod legacy_print;
pub mod levels;
pub mod migrate;
pub mod modes;
pub mod paper;
pub mod prepare;
mod reading;
pub mod settings;
pub mod sidecar;
pub mod spectrogram;
pub mod tolerant;
pub mod tone;
pub mod virtual_film;
pub mod white;

pub use document::{AutoBalance, Document, Failure};
pub use dyes::DyeSet;
pub use date::Date;
pub use film::{Film, roll_folder};
pub use id::Id;
pub use legacy_print::{LegacyMachine, LegacyPrint};
pub use modes::{ConversionMode, Medal, SourceEncoding};
pub use paper::PaperStock;
pub use reading::Reading;
pub use settings::PipelineSettings;
pub use virtual_film::{VirtualFilm, VirtualStock};
pub use sidecar::{ExifLine, ExifMetadata, LensSpecification, Loaded};
