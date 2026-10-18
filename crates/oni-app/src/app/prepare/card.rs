//! The card: each frame in its scan's shape, and the base picked on its scan.

use super::*;

impl Ctl {
    // MARK: - The card and the base's scan

    pub(in crate::app) fn card_box(&self) -> Option<(f64, f64, f64, f64)> {
        let w = self.window()?;
        let p = w.global::<Preparing>();
        let r = &self.state.borrow().prepare;
        let aspect = r.shown_frame().and_then(|f| f.aspect).unwrap_or(r.aspect);
        Some(fitted(aspect, (f64::from(p.get_card_width()), f64::from(p.get_card_height()))))
    }

    pub(in crate::app) fn window_grabbed(self: &Rc<Self>, handle: i32) {
        let (Some(handle), Some(b)) = (Handle::from_index(handle), self.card_box()) else { return };
        let r = &mut self.state.borrow_mut().prepare;
        r.window_gesture = Some((handle, r.window, (b.2, b.3)));
    }

    /// `MeasureWindowOverlay.bounded`: the window kept inside the whole scan, never under 5 %.
    pub(in crate::app) fn window_dragged(self: &Rc<Self>, dx: f64, dy: f64) {
        {
            let r = &mut self.state.borrow_mut().prepare;
            let Some((handle, from, size)) = r.window_gesture else { return };
            r.window = frames::bounded(frames::apply(handle, from, size, (dx, dy)));
        }
        self.publish_card();
    }

    /// The scan's rectangle in the base area, and the scan's own size in pixels.
    pub(in crate::app) fn scan_rect(&self) -> Option<((f64, f64, f64, f64), (usize, usize))> {
        let w = self.window()?;
        let p = w.global::<Preparing>();
        let r = &self.state.borrow().prepare;
        let path = r.base_shown()?.path.clone();
        let (_, _, size) = r.scan.as_ref().filter(|(p, _, _)| *p == path)?;
        let rect = fitted_image((size.0 as f64, size.1 as f64), (f64::from(p.get_base_width()), f64::from(p.get_base_height())));
        Some((rect, *size))
    }

    /// A click inside the scan picks the bare film there, then the roll goes on to its flags.
    pub(in crate::app) fn base_clicked(self: &Rc<Self>, x: f64, y: f64) {
        let Some((rect, _)) = self.scan_rect() else { return };
        if x < rect.0 || y < rect.1 || x > rect.0 + rect.2 || y > rect.1 + rect.3 {
            return;
        }
        {
            let mut s = self.state.borrow_mut();
            let radius = base_radius(&s);
            let r = &mut s.prepare;
            let Some(path) = r.base_shown().map(|f| f.path.clone()) else { return };
            r.base = Some(BasePick { path, unit: ((x - rect.0) / rect.2, (y - rect.1) / rect.3), radius });
        }
        self.step_to(Step::Flags);
    }
}
