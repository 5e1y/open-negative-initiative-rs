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
        let (_, _, size, _) = r.scan.as_ref().filter(|(p, _, _, _)| *p == path)?;
        let rect = fitted_image((size.0 as f64, size.1 as f64), (f64::from(p.get_base_width()), f64::from(p.get_base_height())));
        Some((rect, *size))
    }

    /// A click inside the scan picks the bare film there; the step stays, its ring landing on the
    /// pick, so the hand sees and checks it before NEXT.
    pub(in crate::app) fn base_clicked(self: &Rc<Self>, x: f64, y: f64) {
        let Some((rect, _)) = self.scan_rect() else { return };
        // A scan not laid yet has no size: a click there would pick a point of none.
        if !(rect.2 > 0.0 && rect.3 > 0.0) || x < rect.0 || y < rect.1 || x > rect.0 + rect.2 || y > rect.1 + rect.3 {
            return;
        }
        {
            let mut s = self.state.borrow_mut();
            let radius = base_radius(&s);
            let r = &mut s.prepare;
            let Some(path) = r.base_shown().map(|f| f.path.clone()) else { return };
            r.base = Some(BasePick { path, unit: ((x - rect.0) / rect.2, (y - rect.1) / rect.3), radius });
        }
        if let Some(w) = self.window() {
            let p = w.global::<Preparing>();
            p.set_base_picks(p.get_base_picks() + 1);
        }
        self.publish_prepare();
    }
}

/// The colour the pick sampled, as the scan shows it: its pixels' mean inside the disc.
pub(super) fn sampled(scan: &Image, unit: (f64, f64), radius: f64) -> slint::Color {
    let Some(pixels) = scan.to_rgba8() else { return slint::Color::default() };
    let [r, g, b] = disc_mean(pixels.as_bytes(), pixels.width() as usize, pixels.height() as usize, unit, radius);
    slint::Color::from_rgb_u8(r, g, b)
}

/// The mean of an RGBA picture's pixels whose centres lie in the disc at `unit`, `radius` pixels
/// wide; the pixel under its centre when the disc holds none.
fn disc_mean(rgba: &[u8], width: usize, height: usize, unit: (f64, f64), radius: f64) -> [u8; 3] {
    if width == 0 || height == 0 || rgba.len() < width * height * 4 {
        return [0; 3];
    }
    let (cx, cy) = (unit.0 * width as f64, unit.1 * height as f64);
    let span = |c: f64, end: usize| ((c - radius).floor().max(0.0) as usize, ((c + radius).ceil().max(0.0) as usize).min(end));
    let ((x0, x1), (y0, y1)) = (span(cx, width), span(cy, height));
    let (mut sum, mut count) = ([0u64; 3], 0u64);
    for y in y0..y1 {
        for x in x0..x1 {
            if (x as f64 + 0.5 - cx).powi(2) + (y as f64 + 0.5 - cy).powi(2) > radius * radius {
                continue;
            }
            let at = (y * width + x) * 4;
            for (s, v) in sum.iter_mut().zip(&rgba[at..at + 3]) {
                *s += u64::from(*v);
            }
            count += 1;
        }
    }
    if count == 0 {
        let at = ((cy as usize).min(height - 1) * width + (cx as usize).min(width - 1)) * 4;
        return [rgba[at], rgba[at + 1], rgba[at + 2]];
    }
    sum.map(|s| ((s + count / 2) / count) as u8)
}

#[cfg(test)]
mod tests {
    use super::disc_mean;

    #[test]
    fn a_pick_reads_the_mean_of_its_own_disc_alone() {
        // A 20 x 10 picture: orange film at the left half, a blue picture at the right.
        let (w, h) = (20, 10);
        let rgba: Vec<u8> = (0..w * h).flat_map(|i| if i % w < 10 { [200, 120, 60, 255] } else { [20, 40, 220, 255] }).collect();
        assert_eq!(disc_mean(&rgba, w, h, (0.2, 0.5), 3.0), [200, 120, 60], "a disc on the film reads the film");
        // Twin: the same disc on the picture reads the picture, and one across both reads neither.
        assert_eq!(disc_mean(&rgba, w, h, (0.8, 0.5), 3.0), [20, 40, 220]);
        let across = disc_mean(&rgba, w, h, (0.5, 0.5), 3.0);
        assert!(across != [200, 120, 60] && across != [20, 40, 220], "{across:?}");
        // A disc too small to hold a pixel's centre reads the pixel under it; one past the edge, its share.
        assert_eq!(disc_mean(&rgba, w, h, (0.01, 0.01), 0.1), [200, 120, 60]);
        assert_eq!(disc_mean(&rgba, w, h, (1.0, 1.0), 4.0), [20, 40, 220]);
    }
}
