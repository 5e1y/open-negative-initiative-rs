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
            let Some(g) = s.gpu.as_mut() else { return };
            let Some(view) = before.view().cloned() else { return };
            let e = before.extent();
            let reuse = g.before.as_ref().filter(|(t, _)| t.width() == e.w && t.height() == e.h).map(|(t, _)| t.clone());
            let (target, imported) = match reuse {
                Some(t) => (t, None),
                None => {
                    let t = display::Gpu::target(&g.device, e.w, e.h);
                    (t.clone(), Image::try_from(t).ok())
                }
            };
            let k = zebra(&s, Params::new(screen, e.w, e.h, s.gpu.as_ref().map_or(0, |g| g.seed)), placed, e.w, canvas);
            let Some(g) = s.gpu.as_mut() else { return };
            g.kernel.run(&g.device, &g.queue, &view, &k, &target);
            if g.before.as_ref().is_none_or(|(_, kept)| kept.id() != before.id()) {
                s.befores += 1;
            }
            s.before_placed = placed;
            let Some(g) = s.gpu.as_mut() else { return };
            g.before = Some((target, before));
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
            s.split = if s.split.is_some() { None } else { Some(0.5) };
            // The before is a texture the size of the view: Compare off lets it go.
            if s.split.is_none()
                && let Some(g) = s.gpu.as_mut()
            {
                g.before = None;
            }
            s.split
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
