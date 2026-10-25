//! The Scanner face: the keypad's clicks, its own finishing, and the retired Scanner Emulation a
//! document written before it left still carries, folded on the way in.

use crate::cg::Point;
use crate::curves::{Curve, CurveSet, SaturationCurve};
use crate::levels::LevelsChannel;
use crate::settings::PipelineSettings;
use crate::spectrogram::Spectrogram;
use crate::blocks::Vignette;
use crate::tolerant;
use crate::tone::{Print, Tone, color_density};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

type V3 = [f32; 3];

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LookModel {
    #[default]
    Oni,
    Noritsu,
    Frontier,
}

impl LookModel {
    pub const ALL: [LookModel; 3] = [Self::Oni, Self::Noritsu, Self::Frontier];
}

/// The retired Scanner Emulation as a stored document carries it.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct ScannerLook {
    pub model: LookModel,
    pub amount: f32,
}

impl<'de> Deserialize<'de> for ScannerLook {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let map = tolerant::object(Value::deserialize(d)?, "ScannerLook")?;
        let model = tolerant::opt::<LookModel, D::Error>(&map, "model").ok().flatten().unwrap_or_default();
        let amount = tolerant::opt::<f32, D::Error>(&map, "amount")?.unwrap_or(0.0);
        Ok(ScannerLook { model, amount })
    }
}

/// A look as the machine's capture then its own Lighting, offsets from rest.
pub struct Recipe {
    pub master: Tone,
    pub red: Tone,
    pub green: Tone,
    pub blue: Tone,
    pub density: f32,
    pub capture: [V3; 3],
    pub low: V3,
    pub high: V3,
}

const IDENTITY_ROWS: [V3; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn tone(exposure: f32, contrast: f32, blacks: f32, shadows: f32, highlights: f32, whites: f32, mids: f32) -> Tone {
    Tone { exposure, contrast, blacks, shadows, highlights, whites, mids }
}

impl ScannerLook {
    pub const FOLD_POINTS: usize = 16;

    pub fn is_active(&self) -> bool {
        self.amount != 0.0
    }

    pub fn recipe(model: LookModel) -> Recipe {
        match model {
            LookModel::Oni => Recipe {
                master: tone(0.0, 0.0, -0.093, 0.019, 0.0, 0.0, 0.0),
                red: tone(0.0, 0.0, 0.158, -0.0391, -0.0492, 0.0, 0.0261),
                green: Tone::NEUTRAL,
                blue: tone(0.0, 0.0, 0.045, -0.0075, -0.0227, 0.0, 0.0034),
                density: 0.0,
                capture: IDENTITY_ROWS,
                low: [0.035; 3],
                high: [1.0, 1.0, 0.9861],
            },
            LookModel::Frontier => Recipe {
                master: tone(0.0, 0.13, -0.02, 0.0, -0.03, 0.0, 0.0),
                red: tone(0.0, 0.0, 0.0, -0.045, 0.045, 0.0, 0.0),
                green: Tone::NEUTRAL,
                blue: tone(0.0, 0.0, 0.08, 0.06, -0.07, 0.0, 0.0),
                density: 0.15,
                capture: [[0.9074, 0.0862, 0.0064], [0.0030, 0.9950, 0.0020], [0.0095, 0.0390, 0.9515]],
                low: [0.0; 3],
                high: [1.0; 3],
            },
            LookModel::Noritsu => Recipe {
                master: tone(0.0, -0.06, 0.09, 0.0, 0.0, -0.07, 0.03),
                red: tone(0.14, 0.0, 0.0, 0.0, 0.03, 0.0, 0.0),
                green: tone(0.0, 0.0, 0.0, 0.03, -0.03, 0.0, 0.0),
                blue: tone(-0.19, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
                density: -0.12,
                capture: [[1.0081, -0.0081, 0.0], [-0.0070, 1.0111, -0.0040], [0.0, -0.0020, 1.0020]],
                low: [0.0; 3],
                high: [1.0; 3],
            },
        }
    }

    fn dose(&self) -> f32 {
        if self.is_active() { self.amount } else { 0.0 }
    }

    /// The look's own curves at this amount, each slider held on its track.
    pub fn curves(&self) -> [Tone; 4] {
        let r = Self::recipe(self.model);
        let a = self.dose();
        [r.master.scaled(a), r.red.scaled(a), r.green.scaled(a), r.blue.scaled(a)]
    }

    pub fn capture_rows(&self) -> [V3; 3] {
        let c = Self::recipe(self.model).capture;
        let a = self.dose();
        std::array::from_fn(|r| std::array::from_fn(|k| IDENTITY_ROWS[r][k] + a * (c[r][k] - IDENTITY_ROWS[r][k])))
    }

    pub fn ends(&self) -> (V3, V3) {
        let r = Self::recipe(self.model);
        let a = self.dose();
        (r.low.map(|v| a * v), r.high.map(|v| 1.0 + a * (v - 1.0)))
    }

    pub fn captured(&self, n: V3) -> V3 {
        if !self.is_active() {
            return n;
        }
        self.capture_rows().map(|row| row[0] * n[0] + row[1] * n[1] + row[2] * n[2])
    }

    /// The curves and ends on display values, over the hand's print.
    pub fn graded_display(&self, p: V3) -> V3 {
        if !self.is_active() {
            return p;
        }
        let [m, r, g, b] = self.curves();
        let (low, high) = self.ends();
        let one = |v: f32, own: Tone| own.shaped(m.shaped(v, m.exposure), own.exposure);
        let shaped = [one(p[0], r), one(p[1], g), one(p[2], b)];
        std::array::from_fn(|i| low[i] + (high[i] - low[i]) * shaped[i])
    }

    /// The same on linear light, as the kernel carries it.
    pub fn graded(&self, light: V3) -> V3 {
        if !self.is_active() {
            return light;
        }
        let g = Print::DISPLAY_GAMMA;
        let p = self.graded_display(light.map(|v| v.max(0.0).powf(1.0 / g)));
        p.map(|v| v.max(0.0).powf(g))
    }

    /// The settings with this look carried by their own fields: each channel's curve composed after
    /// it, its Color Density added on its track.
    pub fn folded(&self, settings: &PipelineSettings) -> PipelineSettings {
        if !self.is_active() {
            return settings.clone();
        }
        let mut out = settings.clone();
        let r = Self::recipe(self.model);
        out.density = (settings.density + self.amount * r.density).clamp(color_density::RANGE.0, color_density::RANGE.1);
        let g = Print::DISPLAY_GAMMA;
        for (index, channel) in LevelsChannel::PER_CHANNEL.into_iter().enumerate() {
            let hand = settings.manual_curves.get(channel).evaluated();
            let points = (0..=Self::FOLD_POINTS)
                .map(|i| {
                    let x = (i as f32 / Self::FOLD_POINTS as f32).powf(g);
                    let looked = self.graded([x; 3])[index];
                    Point::new(f64::from(x), hand.value(f64::from(looked)).clamp(0.0, 1.0))
                })
                .collect();
            *out.manual_curves.get_mut(channel) = Curve { points };
        }
        out
    }

    /// The active look a stored document's settings carry, if any.
    pub fn stored(settings: Option<&Value>) -> Option<ScannerLook> {
        let raw = settings?.as_object()?.get("look")?;
        let look: ScannerLook = serde_json::from_value(raw.clone()).ok()?;
        look.is_active().then_some(look)
    }
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

    #[test]
    fn an_unknown_look_model_reads_as_oni() {
        let look: ScannerLook = serde_json::from_str(r#"{"model":"agfa","amount":0.5}"#).unwrap();
        assert_eq!(look, ScannerLook { model: LookModel::Oni, amount: 0.5 });
        let frontier: ScannerLook = serde_json::from_str(r#"{"model":"frontier"}"#).unwrap();
        assert_eq!(frontier.model, LookModel::Frontier);
        assert!(!frontier.is_active());
    }

    #[test]
    fn a_folded_look_grades_the_curves_as_its_replica_did() {
        let probe = PipelineSettings::default();
        for model in LookModel::ALL {
            let look = ScannerLook { model, amount: 1.0 };
            let folded = look.folded(&probe);
            let mut worst: f32 = 0.0;
            let mut dropped: f32 = 0.0;
            for i in 0..=100 {
                let v = i as f32 / 100.0;
                let want = look.graded([v; 3]);
                for (c, channel) in LevelsChannel::PER_CHANNEL.into_iter().enumerate() {
                    let got = folded.manual_curves.get(channel).value(f64::from(v)) as f32;
                    let display = |x: f32| x.max(0.0).powf(1.0 / Print::DISPLAY_GAMMA);
                    worst = worst.max((display(got) - display(want[c].clamp(0.0, 1.0))).abs());
                    dropped = dropped.max((display(v) - display(want[c].clamp(0.0, 1.0))).abs());
                }
            }
            assert!(worst < 0.01, "{model:?} folds within {worst}");
            // Twin: leaving the look out parts the render by far more than the fold's own error.
            assert!(dropped > 4.0 * worst, "{model:?}: dropped {dropped} against {worst}");
        }
    }
}
