//! What the canvas asks: the view, where the picture lands, and the renderer a worker drives.

use super::*;

/// What the canvas shows: the viewport over its drawable surface, and the frame's full-resolution
/// pixels once built, which a zoom past the preview's ratio reads instead of a stretched preview.
#[derive(Clone)]
pub struct View {
    pub viewport: crate::viewport::Viewport,
    /// The canvas, in drawable pixels.
    pub size: (f64, f64),
    pub full: Option<Source>,
}

/// A view's identity for a kept render: the viewport, the canvas and which full frame.
pub type ViewKey = (crate::viewport::Viewport, (f64, f64), Option<u64>);

impl View {
    pub fn key(&self) -> ViewKey {
        (self.viewport, self.size, self.full.as_ref().map(Source::id))
    }
}

/// Where a render sits on the canvas, in drawable pixels, and what it covers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// The displayed image's size in full-resolution pixels, which the viewport reasons on.
    pub full: (f64, f64),
    /// The render holds the whole image, so its histogram is the picture's.
    pub whole: bool,
    pub at_full_resolution: bool,
}

/// The conversion a canvas copy goes through: settings in, linear Rec. 2020 out.
pub trait Renderer<I>: Send {
    fn render(&mut self, negative: &I, settings: &PipelineSettings) -> Result<Linear, String>;

    /// The levels' and manual curves' input distributions; `None` where nothing measures.
    fn inputs(&mut self, _negative: &I, _settings: &PipelineSettings, _held: bool) -> Option<InputHistograms> {
        None
    }

    /// The same render kept on the GPU for the display kernel, the view's region when given; `None`
    /// where only pixels are made.
    fn resident(&mut self, _negative: &I, _settings: &PipelineSettings, _view: Option<&View>) -> Option<Result<(Source, Option<Placed>), String>> {
        None
    }

    /// The whole picture at the histogram's size when the settings moved since the last; `None` else.
    fn counted(&mut self, _negative: &I, _settings: &PipelineSettings) -> Option<Source> {
        None
    }
}

impl Renderer<Negative> for Shared {
    fn render(&mut self, negative: &Negative, settings: &PipelineSettings) -> Result<Linear, String> {
        self.lock()?.render(negative, settings, None)
    }

    fn resident(&mut self, negative: &Negative, settings: &PipelineSettings, view: Option<&View>) -> Option<Result<(Source, Option<Placed>), String>> {
        Some(self.lock().and_then(|mut e| e.resident(negative, settings, view)))
    }

    fn inputs(&mut self, negative: &Negative, settings: &PipelineSettings, held: bool) -> Option<InputHistograms> {
        self.lock().ok()?.inputs(negative, settings, held)
    }

    fn counted(&mut self, negative: &Negative, settings: &PipelineSettings) -> Option<Source> {
        self.lock().ok()?.counted(negative, settings).ok().flatten()
    }
}
