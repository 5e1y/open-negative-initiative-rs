//! The Scanner face: the keypad's clicks and its own finishing.

use crate::curves::{CurveSet, SaturationCurve};
use crate::spectrogram::Spectrogram;
use crate::blocks::Vignette;
use crate::tolerant;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

/// The keypad's steps on D, C, M and Y, then warmth: counts, never densities. A step is half the
/// unit the screen reads, so a density step is a quarter of the machine's measured click.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScannerClicks {
    pub d: i64,
    pub c: i64,
    pub m: i64,
    pub y: i64,
    pub w: i64,
}

impl ScannerClicks {
    /// Steps in one unit of the screen, the unit Swift's sidecar counts in.
    pub const PER_UNIT: i64 = 2;
    pub const DENSITY_RANGE: (i64, i64) = (-24 * Self::PER_UNIT, 24 * Self::PER_UNIT);
    pub const WARMTH_RANGE: (i64, i64) = (-10 * Self::PER_UNIT, 10 * Self::PER_UNIT);
    const KEYS: [&'static str; 5] = ["d", "c", "m", "y", "w"];

    pub fn new(d: i64, c: i64, m: i64, y: i64, w: i64) -> Self {
        let mut out = ScannerClicks { d, c, m, y, w };
        out.clamp();
        out
    }

    fn clamp(&mut self) {
        let (lo, hi) = Self::DENSITY_RANGE;
        self.d = self.d.clamp(lo, hi);
        self.c = self.c.clamp(lo, hi);
        self.m = self.m.clamp(lo, hi);
        self.y = self.y.clamp(lo, hi);
        self.w = self.w.clamp(Self::WARMTH_RANGE.0, Self::WARMTH_RANGE.1);
    }

    pub fn is_neutral(&self) -> bool {
        *self == ScannerClicks::default()
    }

    fn all(&self) -> [i64; 5] {
        [self.d, self.c, self.m, self.y, self.w]
    }
}

/// Swift's whole units under each letter, toward zero, and the half step left over under `halves`,
/// which Swift ignores; a keypad on whole units writes what Swift writes.
impl Serialize for ScannerClicks {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = Map::new();
        let mut halves = Map::new();
        for (key, steps) in Self::KEYS.into_iter().zip(self.all()) {
            map.insert(key.into(), Value::from(steps / Self::PER_UNIT));
            if steps % Self::PER_UNIT != 0 {
                halves.insert(key.into(), Value::from(steps % Self::PER_UNIT));
            }
        }
        if !halves.is_empty() {
            map.insert("halves".into(), Value::Object(halves));
        }
        map.serialize(serializer)
    }
}

/// Tolerant of absent or unreadable keys, and a count past its range lands on its stop.
impl<'de> Deserialize<'de> for ScannerClicks {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let map = tolerant::object(Value::deserialize(d)?, "ScannerClicks")?;
        let halves = map.get("halves").and_then(Value::as_object);
        let read = |key: &str| {
            let half = halves.and_then(|h| tolerant::integer(h.get(key))).unwrap_or(0).clamp(-1, 1);
            tolerant::integer(map.get(key)).unwrap_or(0).saturating_mul(Self::PER_UNIT).saturating_add(half)
        };
        Ok(ScannerClicks::new(read("d"), read("c"), read("m"), read("y"), read("w")))
    }
}

/// The Color tab's finishing on the Scanner face, apart so a gesture on one face never moves the other.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ScannerFinish {
    pub contrast: f32,
    #[serde(rename = "manualCurves")]
    pub manual_curves: CurveSet,
    pub spectrogram: Spectrogram,
    pub saturation: SaturationCurve,
    pub vignette: Vignette,
    #[serde(rename = "secretSauce")]
    pub secret_sauce: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_keypad_reads_absent_keys_at_rest_and_clamps_past_its_stops() {
        let absent: ScannerClicks = serde_json::from_str("{}").unwrap();
        assert_eq!(absent, ScannerClicks::default());
        let past: ScannerClicks = serde_json::from_str(r#"{"d":25,"c":-30,"w":11}"#).unwrap();
        assert_eq!(past, ScannerClicks::new(48, -48, 0, 0, 20));
        // Twin: counts on and inside the stops read as written, in steps, a retired key ignored.
        let inside: ScannerClicks = serde_json::from_str(r#"{"d":-24,"m":7,"y":-3,"w":-10,"s":20}"#).unwrap();
        assert_eq!((inside.d, inside.m, inside.y, inside.w), (-48, 14, -6, -20));
        assert_ne!(inside.m, 7, "a stored unit is two steps, so an old sidecar reads unchanged on screen");
    }

    #[test]
    fn a_half_step_survives_a_sidecar_and_whole_units_write_as_swift_does() {
        let halves = ScannerClicks::new(3, -1, 4, -5, 1);
        let written = serde_json::to_value(halves).unwrap();
        assert_eq!(written, serde_json::json!({"d": 1, "c": 0, "m": 2, "y": -2, "w": 0, "halves": {"d": 1, "c": -1, "y": -1, "w": 1}}));
        assert_eq!(serde_json::from_value::<ScannerClicks>(written).unwrap(), halves);
        // Twin: on whole units the object is Swift's own, no `halves`, and reads back the same.
        let whole = ScannerClicks::new(6, -2, 0, 48, -20);
        let written = serde_json::to_value(whole).unwrap();
        assert_eq!(written, serde_json::json!({"d": 3, "c": -1, "m": 0, "y": 24, "w": -10}));
        assert_eq!(serde_json::from_value::<ScannerClicks>(written).unwrap(), whole);
        // And a `halves` past one step is held to one, so it never reads as a unit of its own.
        let wide: ScannerClicks = serde_json::from_str(r#"{"d":1,"halves":{"d":5,"c":-1.5}}"#).unwrap();
        assert_eq!((wide.d, wide.c), (3, 0));
    }
}
