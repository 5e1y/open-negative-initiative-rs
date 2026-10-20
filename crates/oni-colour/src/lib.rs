//! ONI's colour management: working and export spaces, their ICC profiles, our own BPC and clip,
//! the proof, Paper Correction and the screen gamut zebra.

pub mod colorsync;
pub mod curve;
pub mod gamut;
pub mod icc_lut;
pub mod legs;
pub mod paper;
pub mod paper_source;
pub mod pcs;
pub mod profile;
pub mod proof;
pub mod source;
pub mod space;
pub mod working;

pub use gamut::Display;
pub use legs::{Back, Leg, Legs, Quantised};
pub use paper::PaperCorrection;
pub use paper_source::{InstallRecord, PaperSource, Reach};
pub use profile::OutputProfile;
pub use proof::{Cube, Proof};
pub use space::Space;
