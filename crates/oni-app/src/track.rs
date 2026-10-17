//! `DSSlider` and `DSTrack`'s arithmetic: where a value sits on the track, what a drag asks for,
//! and the pace at which a dragged value reaches the settings.

use std::ops::RangeInclusive;
use std::time::{Duration, Instant};

/// `DSTrack.fineFactor`: travel per point while Option is held.
pub const FINE: f32 = 0.2;
/// The image's cadence while a value slides, `DS.Motion.recomputedInterval`: one store write and
/// render every 1/30 s, so no cache renews faster; the knob and its label follow every event.
pub const HAND_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / 30);
/// Slint's and AppKit's default double-click interval.
pub const DOUBLE_CLICK: Duration = Duration::from_millis(500);
/// `DSScroll.trackpadTravel`: points of travel per point of trackpad glide.
pub const TRACKPAD_TRAVEL: f32 = 0.25;

/// `DSSlider.position(of:in:centre:)`: piecewise linear about `centre` when it bends the track.
pub fn position(value: f32, range: &RangeInclusive<f32>, centre: Option<f32>) -> f32 {
    let (low, high) = (*range.start(), *range.end());
    if high <= low {
        return 0.0;
    }
    match centre {
        Some(c) if c > low && c < high => {
            if value <= c { 0.5 * (value - low) / (c - low) } else { 0.5 + 0.5 * (value - c) / (high - c) }
        }
        _ => (value - low) / (high - low),
    }
}

/// The exact inverse of `position`, or the knob drifts under the hand one round trip at a time.
pub fn value(position: f32, range: &RangeInclusive<f32>, centre: Option<f32>) -> f32 {
    let (low, high) = (*range.start(), *range.end());
    match centre {
        Some(c) if c > low && c < high => {
            if position <= 0.5 { low + 2.0 * position * (c - low) } else { c + 2.0 * (position - 0.5) * (high - c) }
        }
        _ => low + position * (high - low),
    }
}

/// `DSTrack.asked`, unclamped: the start plus the translation over the span, finer under Option.
pub fn asked(start: f32, translation: f32, span: f32, fine: bool) -> f32 {
    if span <= 0.0 {
        return start;
    }
    start + translation / span * if fine { FINE } else { 1.0 }
}

pub fn clamped(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

/// What `DSSlider.follow` decides for each dragged value: hand it now, or leave it to the trailing hand-off.
#[derive(Debug, Default)]
pub struct Hand {
    handed_at: Option<Instant>,
    trailing: bool,
}

#[derive(Debug, PartialEq)]
pub enum Offer {
    Now,
    /// Schedule one hand-off after this delay; the latest live value wins when it fires.
    After(Duration),
    Wait,
}

impl Hand {
    pub fn offer(&mut self, now: Instant) -> Offer {
        let since = self.handed_at.map(|t| now.duration_since(t));
        match since {
            Some(s) if s < HAND_INTERVAL => {
                if self.trailing {
                    Offer::Wait
                } else {
                    self.trailing = true;
                    Offer::After(HAND_INTERVAL - s)
                }
            }
            _ => {
                self.handed_at = Some(now);
                Offer::Now
            }
        }
    }

    /// The trailing hand-off fired: it counts as a hand-off.
    pub fn trailed(&mut self, now: Instant) {
        self.trailing = false;
        self.handed_at = Some(now);
    }

    /// The release hands the last value itself and drops the pending hand-off, whose timer never fires.
    pub fn released(&mut self) {
        self.trailing = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bent_track_is_its_own_inverse() {
        let range = -4.0..=4.0;
        for centre in [None, Some(1.0), Some(-3.5)] {
            for k in 0..=100 {
                let p = k as f32 / 100.0;
                let back = position(value(p, &range, centre), &range, centre);
                assert!((back - p).abs() < 1e-6, "{centre:?} at {p}: {back}");
            }
        }
        // The bend puts the centre at the middle of the travel.
        assert!((position(1.0, &range, Some(1.0)) - 0.5).abs() < 1e-7);
        // Twin: a straight inverse over a bent track drifts.
        let drifted = position(-4.0 + 0.3 * 8.0, &range, Some(1.0));
        assert!((drifted - 0.3).abs() > 0.05);
    }

    #[test]
    fn a_drag_is_relative_and_option_slows_it() {
        assert_eq!(asked(0.5, 100.0, 200.0, false), 1.0);
        assert!((asked(0.5, 100.0, 200.0, true) - 0.6).abs() < 1e-6);
        assert_eq!(clamped(asked(0.9, 100.0, 200.0, false)), 1.0);
        // Twin: an absolute drag would land on the pointer, not on start plus travel.
        assert_ne!(asked(0.2, 10.0, 200.0, false), 10.0 / 200.0);
    }

    #[test]
    fn values_reach_the_settings_thirty_times_a_second() {
        let t0 = Instant::now();
        let mut hand = Hand::default();
        assert_eq!(hand.offer(t0), Offer::Now);
        let early = t0 + Duration::from_millis(10);
        assert_eq!(hand.offer(early), Offer::After(HAND_INTERVAL - Duration::from_millis(10)));
        assert_eq!(hand.offer(early + Duration::from_millis(5)), Offer::Wait);
        hand.trailed(t0 + HAND_INTERVAL);
        assert_eq!(hand.offer(t0 + 2 * HAND_INTERVAL), Offer::Now);
        // A release drops the pending hand-off: the next drag's early value schedules its own.
        let mut held = Hand::default();
        held.offer(t0);
        held.offer(early);
        held.released();
        assert!(matches!(held.offer(early + Duration::from_millis(1)), Offer::After(_)));
        // Twin: without the release the stale flag leaves that value waiting on a timer that was stopped.
        let mut stale = Hand::default();
        stale.offer(t0);
        stale.offer(early);
        assert_eq!(stale.offer(early + Duration::from_millis(1)), Offer::Wait);
        // Twin: offers a full interval apart are each handed at once, never deferred.
        let mut spaced = Hand::default();
        let deferred = (0..5).filter(|k| spaced.offer(t0 + *k * HAND_INTERVAL) != Offer::Now).count();
        assert_eq!(deferred, 0);
    }
}
