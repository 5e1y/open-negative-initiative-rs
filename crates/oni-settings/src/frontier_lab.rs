//! The Frontier Look Lab's edits, each value's distance from the measured machine's: a frame with none
//! renders the measured table, and a refit of the machine's values keeps every edit's meaning.

use crate::tolerant;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// The colour stage's hue sectors, every 30° of output Oklab hue from 0°.
pub const SECTORS: usize = 12;
/// The grey cast's lightness nodes, 0.1 to 0.9.
pub const GREYS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontierLab {
    pub hue: [f32; SECTORS],
    pub twist: [f32; SECTORS],
    pub chroma: [f32; SECTORS],
    pub saturation_by_l: f32,
    pub matrix: [f32; 6],
    pub paper_gain: [f32; 2],
    pub paper_offset: [f32; 2],
    pub grey_a: [f32; GREYS],
    pub grey_b: [f32; GREYS],
    pub character: f32,
}

/// One edited value, by the array it sits in and its place there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabValue {
    Hue(usize),
    Twist(usize),
    Chroma(usize),
    SaturationByL,
    Matrix(usize),
    PaperGain(usize),
    PaperOffset(usize),
    GreyA(usize),
    GreyB(usize),
    Character,
}

impl LabValue {
    /// The distance each value may travel from the machine's: the plan's ranges, centred on Frontier.
    pub fn range(self) -> (f32, f32) {
        match self {
            LabValue::Hue(_) => (-20.0, 20.0),
            LabValue::Twist(_) => (-15.0, 15.0),
            LabValue::Chroma(_) => (-0.5, 0.5),
            LabValue::SaturationByL => (-0.3, 0.3),
            LabValue::Matrix(_) => (-0.3, 0.3),
            LabValue::PaperGain(_) | LabValue::PaperOffset(_) => (-0.2, 0.2),
            LabValue::GreyA(_) | LabValue::GreyB(_) => (-0.03, 0.03),
            LabValue::Character => (-1.0, 0.0),
        }
    }
}

impl FrontierLab {
    pub fn is_neutral(&self) -> bool {
        *self == FrontierLab::default()
    }

    pub fn get(&self, v: LabValue) -> f32 {
        match v {
            LabValue::Hue(i) => self.hue[i],
            LabValue::Twist(i) => self.twist[i],
            LabValue::Chroma(i) => self.chroma[i],
            LabValue::SaturationByL => self.saturation_by_l,
            LabValue::Matrix(i) => self.matrix[i],
            LabValue::PaperGain(i) => self.paper_gain[i],
            LabValue::PaperOffset(i) => self.paper_offset[i],
            LabValue::GreyA(i) => self.grey_a[i],
            LabValue::GreyB(i) => self.grey_b[i],
            LabValue::Character => self.character,
        }
    }

    /// One value set through its range; a value that is not a number leaves it at the machine's.
    pub fn set(&mut self, v: LabValue, value: f32) {
        let (low, high) = v.range();
        let value = if value.is_finite() { value.clamp(low, high) } else { 0.0 };
        let slot = match v {
            LabValue::Hue(i) => &mut self.hue[i],
            LabValue::Twist(i) => &mut self.twist[i],
            LabValue::Chroma(i) => &mut self.chroma[i],
            LabValue::SaturationByL => &mut self.saturation_by_l,
            LabValue::Matrix(i) => &mut self.matrix[i],
            LabValue::PaperGain(i) => &mut self.paper_gain[i],
            LabValue::PaperOffset(i) => &mut self.paper_offset[i],
            LabValue::GreyA(i) => &mut self.grey_a[i],
            LabValue::GreyB(i) => &mut self.grey_b[i],
            LabValue::Character => &mut self.character,
        };
        *slot = value;
    }
}

/// Read back through the ranges; an absent, malformed or wrongly sized key rests on the machine's values.
impl<'de> Deserialize<'de> for FrontierLab {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let map = tolerant::object::<D::Error>(Value::deserialize(d)?, "FrontierLab")?;
        let mut out = FrontierLab::default();
        let array = |key: &str, n: usize| -> Vec<f32> {
            let read = map.get(key).and_then(|v| v.as_array()).filter(|a| a.len() == n);
            read.map_or(vec![0.0; n], |a| a.iter().map(|v| tolerant::number_f32(Some(v)).unwrap_or(0.0)).collect())
        };
        let scalar = |key: &str| tolerant::number_f32(map.get(key)).unwrap_or(0.0);
        let arrays: [(&str, usize, fn(usize) -> LabValue); 8] = [
            ("hue", SECTORS, LabValue::Hue),
            ("twist", SECTORS, LabValue::Twist),
            ("chroma", SECTORS, LabValue::Chroma),
            ("matrix", 6, LabValue::Matrix),
            ("paperGain", 2, LabValue::PaperGain),
            ("paperOffset", 2, LabValue::PaperOffset),
            ("greyA", GREYS, LabValue::GreyA),
            ("greyB", GREYS, LabValue::GreyB),
        ];
        for (key, n, value) in arrays {
            array(key, n).into_iter().enumerate().for_each(|(i, v)| out.set(value(i), v));
        }
        out.set(LabValue::SaturationByL, scalar("saturationByL"));
        out.set(LabValue::Character, scalar("character"));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_read_back_and_a_malformed_key_rests_on_the_machine() {
        let mut lab = FrontierLab::default();
        lab.set(LabValue::Hue(8), -8.0);
        lab.set(LabValue::GreyB(2), 0.01);
        lab.set(LabValue::Character, -0.5);
        let text = serde_json::to_string(&lab).unwrap();
        assert!(text.contains("\"saturationByL\"") && text.contains("\"greyB\""), "{text}");
        assert_eq!(serde_json::from_str::<FrontierLab>(&text).unwrap(), lab);
        let torn: FrontierLab = serde_json::from_str(r#"{"hue":[1,2,3],"twist":"x","character":-0.25}"#).unwrap();
        assert_eq!(torn.hue, [0.0; SECTORS], "a wrongly sized array is dropped");
        assert_eq!((torn.twist, torn.character), ([0.0; SECTORS], -0.25));
        // Twin: a value past its range lands on its stop rather than passing.
        let far: FrontierLab = serde_json::from_str(r#"{"hue":[90,0,0,0,0,0,0,0,0,0,0,0],"character":2}"#).unwrap();
        assert_eq!((far.hue[0], far.character), (20.0, 0.0));
        assert!(serde_json::from_str::<FrontierLab>("[1]").is_err());
    }
}
