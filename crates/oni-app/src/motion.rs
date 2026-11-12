//! `DS.Motion.Spec`: the app's springs, evaluated analytically so a Slint property follows the same
//! curve SwiftUI draws instead of an easing that only resembles it; `Sprung`, a knob riding `settle`.

use std::time::Instant;

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

/// One knob's shown place, on whatever axis it is drawn: a value from elsewhere springs it on `settle`,
/// one under the hand, scrolled or on a photo just opened is placed at once.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sprung {
    target: f32,
    from: f32,
    since: Option<Instant>,
    placed: bool,
}

impl Sprung {
    /// Aims the knob at `to`: placed when `jump` or on its first draw, else sprung from where it shows.
    pub fn aim(&mut self, to: f32, jump: bool, now: Instant) {
        if jump || !self.placed {
            *self = Sprung { target: to, from: to, since: None, placed: true };
        } else if to != self.target {
            *self = Sprung { target: to, from: self.at(now), since: Some(now), placed: true };
        }
    }

    pub fn at(&self, now: Instant) -> f32 {
        let Some(since) = self.since else { return self.target };
        let t = now.saturating_duration_since(since).as_secs_f64();
        if t >= Spring::SETTLE.settled_after() {
            return self.target;
        }
        self.from + (self.target - self.from) * Spring::SETTLE.value(t) as f32
    }

    pub fn moving(&self, now: Instant) -> bool {
        self.since.is_some_and(|since| now.saturating_duration_since(since).as_secs_f64() < Spring::SETTLE.settled_after())
    }

    /// One frame of a spring: where the knob shows, or None at rest; the frame it lands on places it.
    pub fn step(&mut self, now: Instant) -> Option<f32> {
        self.since?;
        let at = self.at(now);
        if !self.moving(now) {
            self.since = None;
        }
        Some(at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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

    #[test]
    fn a_value_from_elsewhere_springs_and_one_under_the_hand_is_placed() {
        let t0 = Instant::now();
        let mut k = Sprung::default();
        k.aim(0.2, false, t0);
        assert_eq!(k.at(t0), 0.2, "the first draw places the knob");
        k.aim(0.8, false, t0);
        let early = k.at(t0 + Duration::from_millis(16));
        assert!(early > 0.2 && early < 0.5, "a frame in, the knob has left but not arrived: {early}");
        assert!(k.moving(t0 + Duration::from_millis(16)));
        let peak = (0..60).map(|i| k.at(t0 + Duration::from_millis(16 * i))).fold(0f32, f32::max);
        assert!(peak > 0.8, "settle overshoots its target: {peak}");
        let rest = t0 + Duration::from_secs_f64(Spring::SETTLE.settled_after()) + Duration::from_millis(1);
        assert_eq!(k.at(rest), 0.8);
        assert!(!k.moving(rest));
        // Twin: the same arrival under the hand lands on the first frame.
        let mut held = Sprung::default();
        held.aim(0.2, false, t0);
        held.aim(0.8, true, t0);
        assert_eq!(held.at(t0 + Duration::from_millis(16)), 0.8);
        assert!(!held.moving(t0));
    }

    #[test]
    fn a_new_target_mid_flight_springs_on_from_where_the_knob_shows() {
        let t0 = Instant::now();
        let mut k = Sprung::default();
        k.aim(0.0, false, t0);
        k.aim(1.0, false, t0);
        let mid = t0 + Duration::from_millis(80);
        let shown = k.at(mid);
        k.aim(0.0, false, mid);
        assert_eq!(k.at(mid), shown, "no jump at the turn");
        // Twin: the same target again restarts nothing.
        let before = k;
        k.aim(0.0, false, mid + Duration::from_millis(16));
        assert_eq!(k, before);
    }

    #[test]
    fn a_spring_steps_each_frame_and_its_landing_frame_lays_the_target() {
        let t0 = Instant::now();
        let mut k = Sprung::default();
        k.aim(0.2, false, t0);
        assert_eq!(k.step(t0), None, "a placed knob has no frame to draw");
        k.aim(0.8, false, t0);
        let frame = |i: u64| t0 + Duration::from_millis(16 * i);
        let first = k.step(frame(1)).expect("a sprung knob draws its frames");
        assert!(first > 0.2 && first < 0.8, "{first}");
        let landing = (2..).find(|i| !k.moving(frame(*i))).unwrap();
        for i in 2..landing {
            assert!(k.step(frame(i)).is_some());
        }
        assert_eq!(k.step(frame(landing)), Some(0.8), "the landing frame lays the target exactly");
        assert_eq!(k.step(frame(landing + 1)), None, "then nothing is left to draw");
        // Twin: without the landing frame the last one drawn stands short of the target.
        let mut short = Sprung::default();
        short.aim(0.2, false, t0);
        short.aim(0.8, false, t0);
        assert_ne!(short.at(frame(landing - 1)), 0.8);
    }
}
