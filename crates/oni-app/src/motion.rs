//! `DS.Motion.Spec`: the app's springs, evaluated analytically so a Slint property follows the same
//! curve SwiftUI draws instead of an easing that only resembles it.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    pub response: f64,
    pub damping: f64,
}

impl Spring {
    /// A value arriving from elsewhere: reset, undo, paste, preset.
    pub const SETTLE: Spring = Spring { response: 0.38, damping: 0.72 };
    /// A displacement the eye follows, critically damped.
    pub const TRAVEL: Spring = Spring { response: 0.52, damping: 1.0 };

    /// `Spec.value(at:)`: 0 at the start, 1 at rest.
    pub fn value(&self, t: f64) -> f64 {
        let omega = 2.0 * std::f64::consts::PI / self.response;
        let zeta = self.damping;
        if zeta >= 1.0 {
            return 1.0 - (-omega * t).exp() * (1.0 + omega * t);
        }
        let damped = omega * (1.0 - zeta * zeta).sqrt();
        1.0 - (-zeta * omega * t).exp() * ((damped * t).cos() + (zeta * omega / damped) * (damped * t).sin())
    }

    /// When the envelope has fallen under a thousandth of the travel: past it nothing visible moves.
    pub fn settled_after(&self) -> f64 {
        let omega = 2.0 * std::f64::consts::PI / self.response;
        let rate = if self.damping >= 1.0 { omega } else { self.damping * omega };
        // The critically damped tail carries `1 + ωt`, which a few more e-folds cover.
        1000f64.ln() / rate * if self.damping >= 1.0 { 1.5 } else { 1.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peak(s: Spring) -> f64 {
        (0..4000).map(|k| s.value(k as f64 * 1e-3)).fold(f64::MIN, f64::max)
    }

    #[test]
    fn settle_overshoots_slightly_and_travel_not_at_all() {
        assert_eq!(Spring::SETTLE.value(0.0), 0.0);
        let over = peak(Spring::SETTLE) - 1.0;
        assert!((over - 0.0384).abs() < 5e-4, "settle overshoot {over}");
        assert!(peak(Spring::TRAVEL) <= 1.0 + 1e-12, "twin: the critically damped spring never passes its target");
        for s in [Spring::SETTLE, Spring::TRAVEL] {
            assert!((s.value(s.settled_after()) - 1.0).abs() < 2e-3, "{s:?} still moving at {}", s.settled_after());
        }
    }
}
