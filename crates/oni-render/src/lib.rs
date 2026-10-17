//! The render core: each GPU kernel beside the scalar reference that judges it.

pub mod cpu;
pub mod gpu;
mod params;

pub use params::{Packed, Stage47};
