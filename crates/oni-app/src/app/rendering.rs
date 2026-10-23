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

/// What the canvas asked of the open frame and was given: reset whole when another frame opens or
/// Edit Together closes, so nothing of one frame's renders stands under the next.
#[derive(Default)]
pub(in crate::app) struct Requests {
    pub rendered: u64,
    pub requested: u64,
    /// The levels' and manual curves' input distributions last read, and the focus each was drawn with.
    pub inputs: Option<render::InputHistograms>,
    pub inputs_drawn: Option<(Option<usize>, Option<usize>)>,
    /// What the last render request showed, so a navigation alone asks again.
    pub requested_view: Option<(Viewport, (f64, f64), bool, bool, bool, bool)>,
    /// The settings the last render was asked with, so a crop moved while framing renders nothing.
    pub requested_live: Option<oni_settings::PipelineSettings>,
    /// When the hand last moved the view, which renders lighter until it rests.
    pub hand: Option<Instant>,
}

/// Compare Before/After: the split, a fraction of the canvas's width (none while off), where the
/// before lies, and how many distinct befores were encoded, which a kept one does not add to.
#[derive(Default)]
pub(in crate::app) struct Compare {
    pub split: Option<f32>,
    pub before_placed: Option<Placed>,
    pub befores: usize,
}

