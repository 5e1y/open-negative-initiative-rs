//! The JPEG quality slider, held as a slider is.

use super::*;

impl Ctl {
    pub(super) fn quality_fraction(q: f64) -> f32 {
        ((q - QUALITY.0) / (QUALITY.1 - QUALITY.0)) as f32
    }

    pub(super) fn set_quality(self: &Rc<Self>, q: f64, weigh: bool) {
        self.export.borrow_mut().settings.jpeg_quality = q.clamp(QUALITY.0, QUALITY.1);
        self.export_changed(weigh);
    }
}
