//! The plots over a histogram: the picture's, the levels' and curves' inputs, each traced at its size.

use super::*;
use crate::editors::Plot;
use crate::histogram::PlotSize;
use oni_measure::histogram::Histogram;

/// The levels' and curves' traces asked last: one overtaken by a later reading is dropped.
static INPUT_TRACES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// `histogram` traced `w` by `h` points at `scale`, `focus` its channel in front, on any thread;
/// nothing for a plot not laid out.
pub(super) fn pixels_in(histogram: &Histogram, focus: Option<usize>, w: f64, h: f64, scale: f32) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
    if w < 1.0 || h < 1.0 {
        return None;
    }
    let size = PlotSize::of(w as f32, h as f32, scale);
    let bytes = crate::histogram::raster(histogram, focus, size);
    Some(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&bytes, size.width as u32, size.height as u32))
}

pub(super) fn pixels(histogram: &Histogram, w: f64, h: f64, scale: f32) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
    pixels_in(histogram, None, w, h, scale)
}

impl Ctl {
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
        let Some(i) = inputs else {
            plots.set_levels(Image::default());
            plots.set_curves(Image::default());
            return;
        };
        // Traced off this thread, 1.5 ms a plot, as the picture's histogram is.
        let (scale, ticket) = (w.window().scale_factor(), INPUT_TRACES.fetch_add(1, Ordering::SeqCst) + 1);
        std::thread::spawn(move || {
            let traced = (pixels_in(&i.levels, focus.0, levels.w, levels.h, scale), pixels_in(&i.curves, focus.1, curves.w, curves.h, scale));
            with_shell(move |c| {
                let Some(w) = c.window().filter(|_| INPUT_TRACES.load(Ordering::SeqCst) == ticket) else { return };
                let plots = w.global::<Plots>();
                plots.set_levels(traced.0.map(Image::from_rgba8_premultiplied).unwrap_or_default());
                plots.set_curves(traced.1.map(Image::from_rgba8_premultiplied).unwrap_or_default());
            });
        });
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
