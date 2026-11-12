//! The retired Scanner Emulation and the retired colour curves a document written before they left
//! still carries, each folded into the photo's own Curves on the way in.

use crate::cg::Point;
use crate::curves::{Curve, CurveSet};
use crate::levels::LevelsChannel;
use crate::settings::PipelineSettings;
use crate::tolerant;
use crate::tone::{Print, Tone, color_density, contrast};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

type V3 = [f32; 3];

/// Display values a fold samples its composed curve at.
pub const FOLD_POINTS: usize = 16;

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
            let points = (0..=FOLD_POINTS)
                .map(|i| {
                    let x = (i as f32 / FOLD_POINTS as f32).powf(g);
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

impl CurveSet {
    /// The retired colour curves a stored document carries under `key`, channels and contrast retired.
    pub fn retired(settings: Option<&Value>, key: &str) -> Option<CurveSet> {
        let raw = settings?.as_object()?.get(key)?;
        let mut set: CurveSet = serde_json::from_value(raw.clone()).ok()?;
        set.red = Curve::default();
        set.green = Curve::default();
        set.blue = Curve::default();
        set.contrast = 0.0;
        (!set.is_neutral()).then_some(set)
    }

    /// These curves with `colour`'s linked and luma tables run after them, sampled on display values.
    pub fn folding(&self, colour: &CurveSet) -> CurveSet {
        let mut out = self.clone();
        let xs: Vec<f64> = (0..=FOLD_POINTS)
            .map(|i| (i as f64 / FOLD_POINTS as f64).powf(f64::from(Print::DISPLAY_GAMMA)))
            .collect();
        let held = |v: f64| v.clamp(0.0, 1.0);
        if !colour.linked.is_neutral() {
            let mine = self.linked.evaluated();
            let theirs = colour.linked.evaluated();
            out.linked = Curve {
                points: xs
                    .iter()
                    .map(|&x| {
                        let bent = mine.value(x) as f32;
                        let contrasted =
                            if self.contrast != 0.0 { contrast::apply(bent, self.contrast, 0.5) } else { bent };
                        Point::new(x, held(theirs.value(f64::from(contrasted))))
                    })
                    .collect(),
            };
            out.contrast = 0.0;
        }
        if !colour.luma.is_neutral() {
            let mine = self.luma.evaluated();
            let theirs = colour.luma.evaluated();
            out.luma = Curve { points: xs.iter().map(|&x| Point::new(x, held(theirs.value(mine.value(x))))).collect() };
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folded_colour_curve_runs_after_the_hand_s() {
        let hand = CurveSet {
            linked: Curve { points: vec![Point::new(0.0, 0.0), Point::new(0.2, 0.26), Point::new(1.0, 1.0)] },
            contrast: 0.2,
            ..CurveSet::default()
        };
        let colour = CurveSet {
            linked: Curve { points: vec![Point::new(0.0, 0.0), Point::new(0.3, 0.42), Point::new(1.0, 1.0)] },
            ..CurveSet::default()
        };
        let folded = hand.folding(&colour);
        assert_eq!(folded.contrast, 0.0);
        let composed = |x: f64| {
            let bent = contrast::apply(hand.linked.value(x) as f32, hand.contrast, 0.5);
            colour.linked.value(f64::from(bent)).clamp(0.0, 1.0)
        };
        let mut worst: f64 = 0.0;
        let mut unfolded: f64 = 0.0;
        for i in 0..=200 {
            let x = i as f64 / 200.0;
            worst = worst.max((folded.linked.value(x) - composed(x)).abs());
            unfolded = unfolded.max((hand.linked.value(x) - composed(x)).abs());
        }
        assert!(worst < 0.01, "the fold departs from the two passes by {worst}");
        // Twin: the hand's curve alone is far from the two passes, so the fold is doing work.
        assert!(unfolded > 5.0 * worst && unfolded > 0.02, "{unfolded}");
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
