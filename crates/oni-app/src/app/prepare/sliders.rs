//! The Flat-Field step's sliders and picker.

use super::*;

impl Ctl {
    // MARK: - The sliders

    pub(in crate::app) fn slider_value(r: &Roll, radius: f64, i: usize) -> f32 {
        match i {
            AMOUNT => r.flat.amount,
            TEMPERATURE => r.flat.temperature,
            TINT => r.flat.tint,
            PICKER => radius as f32,
            _ => r.at as f32,
        }
    }

    /// Where a slider's knob sits: the flat-field's on their own tracks, the others linear.
    pub(in crate::app) fn slider_fraction(r: &Roll, radius: f64, i: usize) -> f32 {
        let v = Self::slider_value(r, radius, i);
        match i {
            AMOUNT | TEMPERATURE | TINT => position(flat_field(i), v),
            PICKER => ((f64::from(v) - pipette::RADIUS_RANGE.0) / (pipette::RADIUS_RANGE.1 - pipette::RADIUS_RANGE.0)) as f32,
            _ => {
                let last = measured(&r.frames).len().saturating_sub(1).max(1);
                v / last as f32
            }
        }
    }

    /// `DSSlider`'s press: a second within the double-click interval resets it to its base.
    pub(in crate::app) fn slider_press(self: &Rc<Self>, i: usize) -> bool {
        let now = Instant::now();
        let double = {
            let mut s = self.state.borrow_mut();
            let radius = base_radius(&s);
            let r = &mut s.prepare;
            let double = r.last_press.is_some_and(|(j, at)| j == i && now.duration_since(at) <= crate::system::double_click());
            r.last_press = Some((i, now));
            r.slide = (!double).then(|| (i, Self::slider_fraction(r, radius, i)));
            double
        };
        if double {
            let base = match i {
                AMOUNT => FlatField::default().amount,
                PICKER => roll::BASE_RADIUS as f32,
                _ => 0.0,
            };
            self.slider_set(i, base);
        }
        double
    }

    pub(in crate::app) fn slider_drag(self: &Rc<Self>, i: usize, translation: f32, span: f32, fine: bool) {
        let Some((held, start)) = self.state.borrow().prepare.slide else { return };
        if held != i {
            return;
        }
        let at = crate::track::clamped(crate::track::asked(start, translation, span, fine));
        let value = match i {
            AMOUNT | TEMPERATURE | TINT => stored_at(flat_field(i), at),
            PICKER => (pipette::RADIUS_RANGE.0 + f64::from(at) * (pipette::RADIUS_RANGE.1 - pipette::RADIUS_RANGE.0)) as f32,
            _ => {
                let last = measured(&self.state.borrow().prepare.frames).len().saturating_sub(1).max(1);
                at * last as f32
            }
        };
        self.slider_set(i, value);
    }

    pub(in crate::app) fn slider_set(self: &Rc<Self>, i: usize, value: f32) {
        {
            let mut s = self.state.borrow_mut();
            match i {
                AMOUNT => s.prepare.flat.amount = value,
                TEMPERATURE => s.prepare.flat.temperature = value,
                TINT => s.prepare.flat.tint = value,
                PICKER => {
                    let mut prefs = crate::preferences::Preferences::load(&s.support);
                    let _ = prefs.set(roll::BASE_RADIUS_KEY, serde_json::Value::from(f64::from(value)));
                }
                SCRUBBER => {
                    let r = &mut s.prepare;
                    let last = measured(&r.frames).len().saturating_sub(1);
                    r.at = (value.round().max(0.0) as usize).min(last);
                }
                _ => {}
            }
        }
        self.publish_prepare();
    }
}
