//! `Viewport` and `CanvasEvents`: what part of the image is looked at, a scale relative to full
//! resolution and an offset in drawable pixels, and how the wheel, the trackpad and a drag move it.

/// A width and a height, in the unit the caller names.
pub type Size = (f64, f64);

/// Past pixel-exact, since dialling in a denoise can call for more than 100 %.
pub const RANGE: (f64, f64) = (0.02, 4.0);
/// The ladder the bar's − / + walk, ascending; the top rung is the range's ceiling.
pub const ZOOM_STEPS: [f64; 5] = [0.25, 0.5, 1.0, 2.0, RANGE.1];
/// Debounce after the last resize before the canvas renders at its new size.
pub const SETTLE: std::time::Duration = std::time::Duration::from_millis(150);
/// The zoom kept from one photo to the next, a choice in Settings; off, each photo opens fitted.
pub static KEEP_ZOOM: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rung {
    Fit,
    Scale(f64),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Viewport {
    /// `None` fits the window; otherwise drawable pixels per pixel of the full image.
    pub scale: Option<f64>,
    /// The image centre's offset from the view's centre, in drawable pixels, y down.
    pub offset: Size,
}

fn clamped(v: f64) -> f64 {
    v.clamp(RANGE.0, RANGE.1)
}

impl Viewport {
    pub fn is_fitted(&self) -> bool {
        self.scale.is_none()
    }

    /// Strictly above or below, so a free zoom cannot skip its neighbour; fit is the floor below the smallest rung.
    pub fn rung(current: f64, fitted: bool, delta: i32) -> Option<Rung> {
        if delta == 0 {
            return None;
        }
        if delta > 0 {
            return ZOOM_STEPS.iter().find(|&&s| s > current).map(|&s| Rung::Scale(s));
        }
        if fitted {
            return None;
        }
        Some(ZOOM_STEPS.iter().rev().find(|&&s| s < current).map_or(Rung::Fit, |&s| Rung::Scale(s)))
    }

    pub fn step(&mut self, rung: Rung, full: Size, view: Size) {
        match rung {
            Rung::Fit => self.fit(),
            Rung::Scale(target) => {
                if self.effective_scale(full, view) > 0.0 {
                    self.zoom_to(target, (view.0 / 2.0, view.1 / 2.0), full, view);
                }
            }
        }
    }

    /// Clamped even when fitted: on a tiny window over 47 Mpx the fitted scale drops below the floor.
    pub fn effective_scale(&self, full: Size, view: Size) -> f64 {
        if let Some(s) = self.scale {
            return s;
        }
        if full.0 <= 0.0 || full.1 <= 0.0 {
            return 1.0;
        }
        clamped((view.0 / full.0).min(view.1 / full.1))
    }

    pub fn zoom_by(&mut self, factor: f64, around: (f64, f64), full: Size, view: Size) {
        self.zoom_to(self.effective_scale(full, view) * factor, around, full, view);
    }

    /// To an absolute scale, so a rung lands on its number rather than an ulp beside it.
    pub fn zoom_to(&mut self, wanted: f64, around: (f64, f64), full: Size, view: Size) {
        let current = self.effective_scale(full, view);
        let target = clamped(wanted);
        if target == current {
            return;
        }
        let from_centre = (around.0 - view.0 / 2.0, around.1 - view.1 / 2.0);
        let ratio = target / current;
        self.offset = ((self.offset.0 - from_centre.0) * ratio + from_centre.0, (self.offset.1 - from_centre.1) * ratio + from_centre.1);
        self.scale = Some(target);
        self.clamp(full, view);
    }

    pub fn pan(&mut self, by: Size, full: Size, view: Size) {
        self.offset = (self.offset.0 + by.0, self.offset.1 + by.1);
        self.clamp(full, view);
    }

    /// The scale is in full-resolution pixels so it survives a change of frame; a pan does not.
    /// Another photo opened: fitted, or at the same zoom recentred when the zoom is kept.
    pub fn next_photo(&mut self, keep: bool) {
        if keep {
            self.recentre();
        } else {
            self.fit();
        }
    }

    pub fn recentre(&mut self) {
        self.offset = (0.0, 0.0);
    }

    pub fn fit(&mut self) {
        *self = Viewport::default();
    }

    /// Pixel-exact, and back to fitted only from exactly there.
    pub fn toggle_hundred(&mut self, full: Size, view: Size) {
        let rung = if self.scale == Some(1.0) { Rung::Fit } else { Rung::Scale(1.0) };
        self.step(rung, full, view);
    }

    /// The image's centre in a top-origin view.
    pub fn centre(view: Size, offset: Size) -> (f64, f64) {
        (view.0 / 2.0 + offset.0, view.1 / 2.0 + offset.1)
    }

    /// Clamps to the overflow `(full × s − view) / 2`, zero once the image fits.
    pub fn clamp(&mut self, full: Size, view: Size) {
        let s = self.effective_scale(full, view);
        let limit = ((full.0 * s - view.0).max(0.0) / 2.0, (full.1 * s - view.1).max(0.0) / 2.0);
        self.offset = (self.offset.0.clamp(-limit.0, limit.0), self.offset.1.clamp(-limit.1, limit.1));
    }
}

/// One dose per device: a wheel counts notches and a touch surface counts points.
pub const WHEEL_ZOOM_PER_NOTCH: f64 = 1.10;
pub const TRACKPAD_ZOOM_PER_POINT: f64 = 1.0025;

pub fn zoom_factor(delta: f64, precise: bool) -> f64 {
    (if precise { TRACKPAD_ZOOM_PER_POINT } else { WHEEL_ZOOM_PER_NOTCH }).powf(delta)
}

/// A wheel always zooms; a touch surface pans unless ⌘ is held.
pub fn zooms(phased: bool, command: bool) -> bool {
    command || !phased
}

/// In drawable pixels; both axes keep their device's direction, so the image follows the hand.
pub fn pan_step(dx: f64, dy: f64, backing: f64) -> Size {
    (dx * backing, dy * backing)
}

/// A wheel event as the platform gave it: notches from a wheel, points from a touch surface,
/// whose phases Slint's own event drops; the touch surface's precise deltas stand for its phases.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wheel {
    pub dx: f64,
    pub dy: f64,
    pub precise: bool,
}

thread_local! {
    static WHEEL: std::cell::Cell<Option<Wheel>> = const { std::cell::Cell::new(None) };
}

/// Noted by the window's event hook just before Slint dispatches the same event to the canvas.
pub fn note_wheel(wheel: Wheel) {
    WHEEL.with(|w| w.set(Some(wheel)));
}

pub fn take_wheel() -> Option<Wheel> {
    WHEEL.with(|w| w.take())
}

/// `AppBar.zoomLabel`.
pub fn zoom_label(scale: f64, at_full_resolution: bool) -> String {
    let percent = format!("{:.0}%", scale * 100.0);
    if at_full_resolution { percent } else { format!("{percent} · {}", oni_i18n::tr("preview")) }
}

/// Where the image lands in a `view`, and the part of it the view shows, in drawable pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// The whole image's top-left corner in the view, and its size there.
    pub image: (f64, f64, f64, f64),
    /// The visible part, in the image's own drawable pixels from its corner; empty when off screen.
    pub visible: (f64, f64, f64, f64),
}

impl Placement {
    /// Whether a picture laid at `rect`, in view pixels, covers all the image the view shows: a pan that
    /// keeps it so needs no render, the picture moved being the one a render would give.
    pub fn covered_by(&self, rect: (f64, f64, f64, f64)) -> bool {
        let (x, y) = (self.image.0 + self.visible.0, self.image.1 + self.visible.1);
        let (w, h) = (self.visible.2, self.visible.3);
        const SLACK: f64 = 0.5;
        rect.0 <= x + SLACK && rect.1 <= y + SLACK && rect.0 + rect.2 >= x + w - SLACK && rect.1 + rect.3 >= y + h - SLACK
    }
}

impl Viewport {
    pub fn placement(&self, full: Size, view: Size) -> Placement {
        let s = self.effective_scale(full, view);
        let (w, h) = (full.0 * s, full.1 * s);
        let (cx, cy) = Viewport::centre(view, self.offset);
        let (x, y) = (cx - w / 2.0, cy - h / 2.0);
        let (x0, y0) = ((-x).max(0.0), (-y).max(0.0));
        let (x1, y1) = ((view.0 - x).min(w), (view.1 - y).min(h));
        Placement { image: (x, y, w, h), visible: (x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)) }
    }
}

#[cfg(test)]
mod checks;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn another_photo_opens_fitted_unless_the_zoom_is_kept() {
        let zoomed = Viewport { scale: Some(1.0), offset: (120.0, -40.0) };
        let mut v = zoomed;
        v.next_photo(false);
        assert!(v.is_fitted() && v.offset == (0.0, 0.0));
        // Twin: kept, the zoom stays and only the centre returns.
        let mut v = zoomed;
        v.next_photo(true);
        assert_eq!(v, Viewport { scale: Some(1.0), offset: (0.0, 0.0) });
    }

    const RAW: Size = (8368.0, 5584.0);
    const AWKWARD: Size = (3810.0, 2540.0);

    #[test]
    fn a_zoom_holds_the_pixel_under_the_pointer() {
        let view = (1801.0, 1201.0);
        let anchor = (1403.0, 907.0);
        let mut held = Viewport { scale: Some(0.5), offset: (120.0, -75.0) };
        let s0 = held.effective_scale(RAW, view);
        let c0 = Viewport::centre(view, held.offset);
        let grabbed = ((anchor.0 - c0.0) / s0, (anchor.1 - c0.1) / s0);
        held.zoom_by(1.7, anchor, RAW, view);
        let s1 = held.effective_scale(RAW, view);
        let c1 = Viewport::centre(view, held.offset);
        let landed = (c1.0 + grabbed.0 * s1, c1.1 + grabbed.1 * s1);
        assert!((landed.0 - anchor.0).abs() < 1e-6 && (landed.1 - anchor.1).abs() < 1e-6, "{landed:?}");
        // Twin: read bottom-origin, the same pixel walks off the pointer.
        let flipped_before = view.1 / 2.0 + 75.0;
        let flipped_after = view.1 / 2.0 - held.offset.1;
        let drift = (flipped_after + (anchor.1 - flipped_before) / s0 * s1 - anchor.1).abs();
        assert!(drift > 1.0, "{drift}");
    }

    #[test]
    fn a_double_click_lands_on_a_hundred_then_back_on_fit() {
        let mut canvas = Viewport::default();
        canvas.toggle_hundred(RAW, AWKWARD);
        assert_eq!(canvas.scale, Some(1.0), "on 100 %, not an ulp beside it");
        // Twin: the multiplied rung misses 1.0 on this window.
        let fitted = Viewport::default().effective_scale(RAW, AWKWARD);
        assert_ne!(fitted * (1.0 / fitted), 1.0);
        canvas.toggle_hundred(RAW, AWKWARD);
        assert!(canvas.is_fitted() && canvas.offset == (0.0, 0.0), "the second returns to fit");
        // From 400 % pushed into a corner a double click still asks for pixels.
        let mut deep = Viewport::default();
        deep.step(Rung::Scale(RANGE.1), RAW, AWKWARD);
        deep.pan((40_000.0, 40_000.0), RAW, AWKWARD);
        deep.toggle_hundred(RAW, AWKWARD);
        assert_eq!(deep.scale, Some(1.0));
        let limit = ((RAW.0 - AWKWARD.0) / 2.0, (RAW.1 - AWKWARD.1) / 2.0);
        assert!(deep.offset.0.abs() <= limit.0 + 1e-3 && deep.offset.1.abs() <= limit.1 + 1e-3);
        // Twin: a rule reading "fitted" instead of "pixel-exact" answers fit from 400 %.
        let mut by_fit = Viewport { scale: Some(RANGE.1), offset: (0.0, 0.0) };
        if by_fit.is_fitted() { by_fit.step(Rung::Scale(1.0), RAW, AWKWARD) } else { by_fit.fit() }
        assert!(by_fit.is_fitted());
    }

    #[test]
    fn the_ladder_steps_strictly_and_fit_is_its_floor() {
        assert_eq!(Viewport::rung(0.3, false, 1), Some(Rung::Scale(0.5)));
        assert_eq!(Viewport::rung(0.5, false, 1), Some(Rung::Scale(1.0)), "strictly above");
        assert_eq!(Viewport::rung(0.3, false, -1), Some(Rung::Scale(0.25)));
        assert_eq!(Viewport::rung(0.2, false, -1), Some(Rung::Fit), "below the smallest rung is fit");
        assert_eq!(Viewport::rung(0.2, true, -1), None, "nothing below fit");
        assert_eq!(Viewport::rung(4.0, false, 1), None, "nothing above the ceiling");
        // Twin: a non-strict step would stay on 0.5.
        assert_ne!(Viewport::rung(0.5, false, 1), Some(Rung::Scale(0.5)));
    }

    #[test]
    fn walking_the_roll_keeps_the_scale_and_drops_the_pan() {
        let mut walked = Viewport { scale: Some(1.0), offset: (120.0, -80.0) };
        walked.recentre();
        assert_eq!(walked, Viewport { scale: Some(1.0), offset: (0.0, 0.0) });
        // Twin: fitting loses the scale.
        let mut refit = walked;
        refit.fit();
        assert!(refit.is_fitted());
    }

    #[test]
    fn the_wheel_zooms_by_notches_and_a_trackpad_by_points() {
        assert!((zoom_factor(1.0, false) - 1.10).abs() < 1e-12);
        assert!((zoom_factor(4.0, true) - 1.0025f64.powi(4)).abs() < 1e-12);
        assert!(zooms(false, false) && zooms(true, true) && !zooms(true, false), "a touch surface pans unless ⌘");
        // Sixteen notches take a fitted 8368 px frame on a 2000 px window to about 100 %.
        let fitted = Viewport::default().effective_scale(RAW, (2000.0, 1335.0));
        assert!((fitted * zoom_factor(16.0, false) - 1.0).abs() < 0.12);
        // Twin: the trackpad's dose on a wheel notch barely moves.
        assert!(zoom_factor(16.0, true) < 1.05);
    }

    #[test]
    fn the_placement_shows_the_part_of_the_image_inside_the_view() {
        let view = (1000.0, 600.0);
        let fitted = Viewport::default().placement((2000.0, 1000.0), view);
        assert_eq!(fitted.image, (0.0, 50.0, 1000.0, 500.0));
        assert_eq!(fitted.visible, (0.0, 0.0, 1000.0, 500.0));
        let zoomed = Viewport { scale: Some(1.0), offset: (500.0, 0.0) }.placement((2000.0, 1000.0), view);
        assert_eq!(zoomed.image, (0.0, -200.0, 2000.0, 1000.0));
        assert_eq!(zoomed.visible, (0.0, 200.0, 1000.0, 600.0), "the left half, its middle rows");
        // Twin: a placement ignoring the offset shows the middle instead.
        let centred = Viewport { scale: Some(1.0), offset: (0.0, 0.0) }.placement((2000.0, 1000.0), view);
        assert_ne!(centred.visible, zoomed.visible);
        assert_eq!(zoom_label(1.0, true), "100%");
        assert_eq!(zoom_label(0.25, false), "25% · preview");
    }
}
