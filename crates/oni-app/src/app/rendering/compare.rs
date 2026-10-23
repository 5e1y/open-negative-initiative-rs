//! Compare Before/After: the before encoded into its own texture, the split on the canvas.

use super::*;

impl Ctl {
    /// Compare's before into its own texture, laid where its view puts it.
    pub(in crate::app) fn encode_before(&self, before: oni_graph::Source, placed: Option<Placed>) {
        let Some(w) = self.window() else { return };
        let canvas = self.canvas_size();
        let (imported, rect) = {
            let mut s = self.state.borrow_mut();
            let screen = s.screen;
            let Some(view) = before.view().cloned() else { return };
            let e = before.extent();
            let k = zebra(&s, Params::new(screen, e.w, e.h, s.gpu.seed), placed, e.w, canvas);
            let g = &s.gpu;
            let (target, shown) = g.encode(g.before.as_ref().map(|(t, _)| t), (e.w, e.h), &view, &k, (e.w, e.h));
            let imported = shown.ok().flatten();
            if g.before.as_ref().is_none_or(|(_, kept)| kept.id() != before.id()) {
                s.compare.befores += 1;
            }
            s.compare.before_placed = placed;
            s.gpu.before = Some((target, before));
            (imported, placed)
        };
        let c = w.global::<Canvas>();
        if let Some(image) = imported {
            c.set_before(image);
        }
        c.set_before_revision(c.get_before_revision().wrapping_add(1));
        if let Some(p) = rect {
            let scale = f64::from(w.window().scale_factor());
            c.set_before_x((p.x / scale) as f32);
            c.set_before_y((p.y / scale) as f32);
            c.set_before_width((p.width / scale) as f32);
            c.set_before_height((p.height / scale) as f32);
        }
    }

    /// Compare Before/After on and off, the split at half the canvas.
    pub(in crate::app) fn toggle_compare(self: &Rc<Self>) {
        let split = {
            let mut s = self.state.borrow_mut();
            s.compare.split = if s.compare.split.is_some() { None } else { Some(0.5) };
            // The before is a texture the size of the view: Compare off lets it go.
            if s.compare.split.is_none() {
                s.gpu.before = None;
            }
            s.compare.split
        };
        if let Some(w) = self.window() {
            w.global::<Canvas>().set_split(split.unwrap_or(-1.0));
            if split.is_none() {
                w.global::<Canvas>().set_before(Image::default());
            }
        }
        self.request_render();
    }
}
