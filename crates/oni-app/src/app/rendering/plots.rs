//! The levels' and curves' plots over their input distributions, each with its channel in focus.

use super::*;

impl Ctl {
    /// The levels' and the manual curves' plots over their input distributions, each with its own
    /// channel in focus; drawn again only when a reading lands or a channel tab moves.
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
        let (pw, ph) = HISTOGRAM;
        let plots = w.global::<Plots>();
        match inputs {
            Some(i) => {
                plots.set_levels(premultiplied(&render::InputHistograms::raster(&i.levels, focus.0, pw, ph), pw, ph));
                plots.set_curves(premultiplied(&render::InputHistograms::raster(&i.curves, focus.1, pw, ph), pw, ph));
            }
            None => {
                plots.set_levels(Image::default());
                plots.set_curves(Image::default());
            }
        }
    }
}
