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
    /// Steps in one unit of the screen.
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

/// Each letter's steps; a sidecar of format 21 and under counts whole units, `migrate`'s to read.
impl Serialize for ScannerClicks {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let map: Map<String, Value> = Self::KEYS.into_iter().zip(self.all()).map(|(k, steps)| (k.to_owned(), Value::from(steps))).collect();
        map.serialize(serializer)
    }
}

/// Tolerant of absent or unreadable keys, and a count past its range lands on its stop.
impl<'de> Deserialize<'de> for ScannerClicks {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let map = tolerant::object(Value::deserialize(d)?, "ScannerClicks")?;
        let read = |key: &str| tolerant::integer(map.get(key)).unwrap_or(0);
        Ok(ScannerClicks::new(read("d"), read("c"), read("m"), read("y"), read("w")))
    }
}

/// The Color tab's finishing on the Scanner face, apart so a gesture on one face never moves the other.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
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
        let past: ScannerClicks = serde_json::from_str(r#"{"d":50,"c":-60,"w":21}"#).unwrap();
        assert_eq!(past, ScannerClicks::new(48, -48, 0, 0, 20));
        // Twin: counts on and inside the stops read as written, in steps, an unknown key ignored.
        let inside: ScannerClicks = serde_json::from_str(r#"{"d":-48,"m":7,"y":-3,"w":-20,"s":20}"#).unwrap();
        assert_eq!((inside.d, inside.m, inside.y, inside.w), (-48, 7, -3, -20));
    }

    #[test]
    fn steps_are_written_as_they_stand_and_read_back_the_same() {
        let odd = ScannerClicks::new(3, -1, 4, -5, 1);
        let written = serde_json::to_value(odd).unwrap();
        assert_eq!(written, serde_json::json!({"d": 3, "c": -1, "m": 4, "y": -5, "w": 1}));
        assert_eq!(serde_json::from_value::<ScannerClicks>(written).unwrap(), odd);
        // Twin: a keypad written in whole units would read half its steps back.
        let halved = serde_json::json!({"d": 1, "c": 0, "m": 2, "y": -2, "w": 0});
        assert_ne!(serde_json::from_value::<ScannerClicks>(halved).unwrap(), odd);
    }
}
