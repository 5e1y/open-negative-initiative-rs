//! De-dust's hands: the brush held as a slider is, and every callback of the tab and the canvas wired to its handler.

use super::*;

impl Ctl {
    pub(in crate::app) fn wire_dedust(me: &Rc<Ctl>, window: &MainWindow) {
        let d = window.global::<crate::Dedust>();
        let p = me.clone();
        d.on_press(move |_| p.brush_press());
        let p = me.clone();
        d.on_drag(move |t, span, fine| p.brush_drag(t, span, fine));
        let p = me.clone();
        d.on_scroll(move |dx, dy, span, fine| p.brush_scroll(dx, dy, span, fine));
        let p = me.clone();
        d.on_step(move |by| p.brush_step(by));
        let p = me.clone();
        d.on_release(move || {
            let dragged = p.state.borrow_mut().brush_start.take().is_some();
            if dragged {
                p.keep_brush();
            }
            p.publish_dedust();
        });
        let p = me.clone();
        d.on_remove(move |i| p.remove_correction(usize::try_from(i).unwrap_or(usize::MAX)));
        let p = me.clone();
        d.on_shown(move |i| p.show_correction(i));
        let p = me.clone();
        d.on_act(move |action| p.act(action));
        let p = me.clone();
        d.on_detect(move || p.detect());
        let p = me.clone();
        d.on_remove_found(move || p.remove_found());
        let p = me.clone();
        d.on_sensitivity_press(move |_| p.sensitivity_press());
        let p = me.clone();
        d.on_sensitivity_drag(move |t, span, fine| p.sensitivity_drag(t, span, fine));
        let p = me.clone();
        d.on_sensitivity_release(move || p.sensitivity_release());
        let p = me.clone();
        d.on_sensitivity_scroll(move |dx, dy, span, fine| p.sensitivity_scroll(dx, dy, span, fine));
        let p = me.clone();
        d.on_sensitivity_step(move |by| p.sensitivity_step(by));
        let c = window.global::<Canvas>();
        let p = me.clone();
        c.on_paint_start(move |x, y| p.paint_at(x, y, true));
        let p = me.clone();
        c.on_paint_move(move |x, y| p.paint_at(x, y, false));
        let p = me.clone();
        c.on_paint_end(move || p.paint_end());
    }

    /// The brush slider, held as `DSSlider` holds a value: a double press resets it to the
    /// default, shown at once rather than on release.
    pub(in crate::app) fn brush_press(&self) -> bool {
        let double = {
            let mut s = self.state.borrow_mut();
            let double = s.brush_pressed_at.is_some_and(|t| t.elapsed() < system::double_click());
            s.brush_pressed_at = Some(Instant::now());
            if double {
                s.brush = oni_settings::frame::Correction::DEFAULT_RADIUS as f32;
                s.brush_start = None;
            } else {
                let (low, high) = oni_settings::frame::Correction::RADIUS_RANGE;
                s.brush_start = Some(((f64::from(s.brush) - low) / (high - low)) as f32);
            }
            double
        };
        if double {
            self.keep_brush();
            self.publish_dedust();
        }
        double
    }

    /// `@AppStorage("correctionBrushRadius")`: the size the hand left, for every photo and launch.
    pub(in crate::app) fn keep_brush(&self) {
        let radius = f64::from(self.state.borrow().brush);
        self.remember(preferences::BRUSH_RADIUS, radius.into());
    }

    /// `DSScrollAdjust` on the brush: its size moved along the track, kept once the scroll rests.
    pub(in crate::app) fn brush_scroll(self: &Rc<Self>, dx: f32, dy: f32, span: f32, fine: bool) -> bool {
        let (low, high) = oni_settings::frame::Correction::RADIUS_RANGE;
        let from = ((f64::from(self.state.borrow().brush) - low) / (high - low)) as f32;
        let Some(at) = self.slider_scrolled(from, dx, dy, span, fine, |c| c.keep_brush()) else { return false };
        self.state.borrow_mut().brush = (low + f64::from(at) * (high - low)) as f32;
        self.publish_dedust();
        true
    }

    /// An arrow on the brush: a pixel a press, kept as the hand's size is.
    pub(in crate::app) fn brush_step(&self, by: i32) {
        let (low, high) = oni_settings::frame::Correction::RADIUS_RANGE;
        let radius = (f64::from(self.state.borrow().brush).round() + f64::from(by)).clamp(low, high);
        self.state.borrow_mut().brush = radius as f32;
        self.keep_brush();
        self.publish_dedust();
    }

    pub(in crate::app) fn brush_drag(&self, translation: f32, span: f32, fine: bool) {
        {
            let mut s = self.state.borrow_mut();
            let Some(start) = s.brush_start else { return };
            let f = track::clamped(track::asked(start, translation, span, fine));
            let (low, high) = oni_settings::frame::Correction::RADIUS_RANGE;
            s.brush = (low + f64::from(f) * (high - low)) as f32;
        }
        self.publish_dedust();
    }
}
