//! The plots over a histogram: the picture's, the levels' and curves' inputs, each traced at its size.

use super::*;
use crate::editors::Plot;
use crate::histogram::PlotSize;
use oni_measure::histogram::Histogram;

impl Ctl {
    /// `histogram` traced `w` by `h` points at the window's scale; nothing while the plot is not laid out.
    pub(crate) fn traced(&self, histogram: &Histogram, focus: Option<usize>, w: f64, h: f64) -> Image {
        let Some(window) = self.window() else { return Image::default() };
        if w < 1.0 || h < 1.0 {
            return Image::default();
        }
        let size = PlotSize::of(w as f32, h as f32, window.window().scale_factor());
        premultiplied(&crate::histogram::raster(histogram, focus, size), size.width, size.height)
    }

    /// The levels' and the manual curves' plots over their input distributions, each with its own
    /// channel in focus; drawn again only when a reading lands, a channel tab moves or a plot is laid out.
    pub(crate) fn publish_inputs(&self) {
        let Some(w) = self.window() else { return };
        let focus = {
            let g = self.editors.gestures.borrow();
            (g.levels_channel().histogram_channel(), g.curve_channel().histogram_channel())
        };
        let inputs = {
            let mut s = self.state.borrow_mut();
            if s.inputs_drawn == Some(focus) {
                return;
            }
            s.inputs_drawn = Some(focus);
            s.inputs.clone()
        };
        let plots = w.global::<Plots>();
        let (levels, curves) = (crate::editors::size(self, Plot::LevelsHistogram), crate::editors::size(self, Plot::Curves));
        match inputs {
            Some(i) => {
                plots.set_levels(self.traced(&i.levels, focus.0, levels.w, levels.h));
                plots.set_curves(self.traced(&i.curves, focus.1, curves.w, curves.h));
            }
            None => {
                plots.set_levels(Image::default());
                plots.set_curves(Image::default());
            }
        }
    }

    /// A plot laid out at a new size is traced again at it.
    pub(crate) fn plot_resized(&self, plot: Plot) {
        match plot {
            Plot::Curves | Plot::LevelsHistogram => {
                self.state.borrow_mut().inputs_drawn = None;
                self.publish_inputs();
            }
            Plot::Saturation => self.state.borrow_mut().histogram_dirty = true,
            _ => {}
        }
    }
}
