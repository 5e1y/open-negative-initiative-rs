//! The one place 8 bits appear: linear Rec. 2020 to the screen's space, encoded, dithered and
//! quantised into an Rgba8Unorm texture the canvas imports. `encode` is the kernel's CPU reference.

use bytemuck::{Pod, Zeroable};

mod gpu;
mod params;
mod reference;
mod screen;

pub use gpu::*;
pub use params::*;
pub use reference::*;
pub use screen::*;

#[cfg(test)]
mod tests;
