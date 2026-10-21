//! The canvas's render loop: asking a render, landing it, the full-resolution region, zoom, compare.

use super::*;

mod asking;
mod navigation;
mod landing;
mod compare;
mod gamut;
mod pace;
mod plots;
mod placeholder;
mod resizing;
pub(in crate::app) use resizing::resizing;

