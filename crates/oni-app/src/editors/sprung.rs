//! `DS.Motion.settle` on the editors' knobs: a value arriving from elsewhere springs the knob there,
//! while one under the hand, scrolled or on a photo just opened is placed at once.

use crate::motion::Spring;
use std::time::Instant;

/// One knob's shown place, on whatever axis its editor draws it.
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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
}
