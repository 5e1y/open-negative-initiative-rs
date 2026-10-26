//! A demo's timeline: values and the hand's path as keys in seconds, eased in and out between them,
//! held before the first key and after the last.

/// The cubic that leaves and lands at rest, `smoothstep`.
pub fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Keys of a value over the clip, each `(second, value)`, in order.
#[derive(Clone, Debug, Default)]
pub struct Track<T> {
    keys: Vec<(f32, T)>,
}

/// What a key holds: a number or a point, mixed linearly.
pub trait Mix: Copy {
    fn mix(self, other: Self, f: f32) -> Self;
}

impl Mix for f32 {
    fn mix(self, other: f32, f: f32) -> f32 {
        self + (other - self) * f
    }
}

impl Mix for (f32, f32) {
    fn mix(self, other: (f32, f32), f: f32) -> (f32, f32) {
        (self.0.mix(other.0, f), self.1.mix(other.1, f))
    }
}

impl<T: Mix> Track<T> {
    /// Starts at `value` from the clip's first instant.
    pub fn from(value: T) -> Track<T> {
        Track { keys: vec![(0.0, value)] }
    }

    /// Holds the last value until `second`.
    pub fn hold(mut self, second: f32) -> Track<T> {
        let last = self.last();
        self.keys.push((second, last));
        self
    }

    /// Eases from the last value to `value`, arriving at `second`.
    pub fn to(mut self, second: f32, value: T) -> Track<T> {
        self.keys.push((second, value));
        self
    }

    pub fn last(&self) -> T {
        self.keys.last().expect("a track starts with a key").1
    }

    pub fn at(&self, t: f32) -> T {
        let next = self.keys.iter().position(|(s, _)| *s > t);
        match next {
            None => self.last(),
            Some(0) => self.keys[0].1,
            Some(i) => {
                let ((s0, a), (s1, b)) = (self.keys[i - 1], self.keys[i]);
                a.mix(b, ease((t - s0) / (s1 - s0)))
            }
        }
    }
}

/// When the hand is down: spans of seconds, `[from, until)`.
#[derive(Clone, Debug, Default)]
pub struct Presses(pub Vec<(f32, f32)>);

impl Presses {
    pub fn down(&self, t: f32) -> bool {
        self.0.iter().any(|(a, b)| t >= *a && t < *b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_track_eases_between_keys_and_holds_past_them() {
        let t = Track::from(0.0).hold(1.0).to(2.0, 10.0);
        assert_eq!(t.at(0.5), 0.0);
        assert_eq!(t.at(1.5), 5.0, "half way in time is half way in value, the ease being symmetric");
        assert!(t.at(1.1) < 1.0, "it leaves slowly");
        assert_eq!(t.at(3.0), 10.0);
        // Twin: a linear mix would be at a tenth already.
        assert!((t.at(1.1) - 1.0).abs() > 0.5);
    }

    #[test]
    fn a_press_spans_its_seconds_only() {
        let p = Presses(vec![(1.0, 2.0)]);
        assert!(p.down(1.0) && p.down(1.9));
        assert!(!p.down(0.9) && !p.down(2.0));
    }
}
