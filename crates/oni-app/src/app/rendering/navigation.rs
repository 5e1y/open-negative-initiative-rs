//! Zoom and pan on the canvas: applied on the preview at once, rendered again once settled.

use super::*;

impl Ctl {
    /// The canvas in drawable pixels, once laid out.
    pub(in crate::app) fn canvas_size(&self) -> Option<(f64, f64)> {
        let w = self.window()?;
        let (c, scale) = (w.global::<Canvas>(), f64::from(w.window().scale_factor()));
        let size = (f64::from(c.get_width()) * scale, f64::from(c.get_height()) * scale);
        (size.0 >= 1.0 && size.1 >= 1.0).then_some(size)
    }

    /// Applies a navigation at once: the picture shown moves with the hand by arithmetic alone, on this
    /// thread, and the render asked refines it; asked again once the view has settled.
    pub(in crate::app) fn navigate(self: &Rc<Self>, change: impl FnOnce(&mut Viewport, (f64, f64), (f64, f64))) {
        let Some(size) = self.canvas_size() else { return };
        let moved = {
            let mut s = self.state.borrow_mut();
            let Some(full) = s.full_size else { return };
            let before = s.viewport;
            change(&mut s.viewport, full, size);
            if s.viewport == before {
                return;
            }
            s.navigated_at = Some(Instant::now());
            s.placed.map(|p| {
                let r = super::resizing::followed((p.x, p.y, p.width, p.height), before.placement(p.full, size).image, s.viewport.placement(p.full, size).image);
                Placed { x: r.0, y: r.1, width: r.2, height: r.3, ..p }
            })
        };
        if let Some(p) = moved.filter(|_| !self.together_shown()) {
            self.lay(Some(p), (0, 0));
        }
        self.request_render();
        let me = Rc::downgrade(self);
        // The timer marks the view settled itself: it can fire a hair before the debounce reads as elapsed.
        self.settle.start(TimerMode::SingleShot, viewport::SETTLE, move || {
            if let Some(me) = me.upgrade() {
                me.state.borrow_mut().navigated_at = None;
                me.request_render();
            }
        });
    }

    /// The bar's − / +: the zoom ladder in the editor.
    pub(in crate::app) fn step_zoom(self: &Rc<Self>, by: i32) {
        let rung = {
            let s = self.state.borrow();
            let (Some(full), Some(size)) = (s.full_size, self.canvas_size()) else { return };
            Viewport::rung(s.viewport.effective_scale(full, size), s.viewport.is_fitted(), by)
        };
        if let Some(rung) = rung {
            self.navigate(|v, full, size| v.step(rung, full, size));
        }
    }
}
