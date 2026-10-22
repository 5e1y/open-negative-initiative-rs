//! The render graph: chains of WGSL stages over rgba32float sources, evaluated by region on wgpu.
//! `README.md` gives the model and the API.

pub mod cpu;
mod graph;
mod rect;
mod stage;
pub mod stages;
mod texture;
pub use texture::{IDLE_RENDERS, Precision, STEP};

pub use graph::{Cache, Error, Graph, Image, Source, Stats, TEXTURE_SIDE, Uploader};
pub use rect::Rect;
pub use stage::{MAX_AUX, Stage};
