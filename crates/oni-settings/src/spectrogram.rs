//! The colour mixer: per-band hue, saturation and luminance offsets on eight fixed centres.

use crate::tolerant;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Band {
    Red,
    Orange,
    Yellow,
    Green,
    Aqua,
    Blue,
    Purple,
    Magenta,
}

impl Band {
    pub const ALL: [Band; 8] =
        [Self::Red, Self::Orange, Self::Yellow, Self::Green, Self::Aqua, Self::Blue, Self::Purple, Self::Magenta];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn centre(self) -> f32 {
        [0.0, 30.0, 60.0, 120.0, 180.0, 240.0, 270.0, 300.0][self.index()]
    }

    /// The room on each side, short of the neighbouring centre by `HUE_MARGIN`.
    pub fn hue_range(self) -> (f32, f32) {
        let i = self.index();
        let up = Spectrogram::separation(self.centre(), Self::ALL[(i + 1) % 8].centre());
        let down = Spectrogram::separation(Self::ALL[(i + 7) % 8].centre(), self.centre());
        (-(down - Spectrogram::HUE_MARGIN), up - Spectrogram::HUE_MARGIN)
    }

    /// Round the wheel as drawn: the centres climb anticlockwise, so a clockwise step is the previous.
    pub fn clockwise(self) -> Band {
        Self::ALL[(self.index() + 7) % 8]
    }

    pub fn anticlockwise(self) -> Band {
        Self::ALL[(self.index() + 1) % 8]
    }

    pub fn label(self) -> &'static str {
        ["Red", "Orange", "Yellow", "Green", "Aqua", "Blue", "Purple", "Magenta"][self.index()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct Spectrogram {
    pub hue: [f32; 8],
    pub saturation: [f32; 8],
    pub luma: [f32; 8],
}

impl Spectrogram {
    pub const HUE_MARGIN: f32 = 3.0;
    pub const SATURATION_RANGE: (f32, f32) = (-1.0, 1.0);
    pub const LUMA_RANGE: (f32, f32) = (-1.0, 1.0);
    pub const BAND_SPREAD: f32 = 1.8;
    /// The keyboard step, a share of each axis's travel; a preference, in no sidecar.
    pub const DEFAULT_STEP: f32 = 0.02;

    pub fn separation(a: f32, b: f32) -> f32 {
        let turn = (b - a) % 360.0;
        if turn < 0.0 { turn + 360.0 } else { turn }
    }

    /// One band set through the clamps the wheel draws.
    pub fn set(&mut self, band: Band, hue: f32, saturation: f32, luma: f32) {
        let (low, high) = band.hue_range();
        let i = band.index();
        self.hue[i] = hue.max(low).min(high);
        self.saturation[i] = saturation.clamp(Self::SATURATION_RANGE.0, Self::SATURATION_RANGE.1);
        self.luma[i] = luma.clamp(Self::LUMA_RANGE.0, Self::LUMA_RANGE.1);
    }

    pub fn is_neutral(&self) -> bool {
        *self == Spectrogram::default()
    }

    /// One band's hue, saturation and luma offsets.
    pub fn band(&self, band: Band) -> (f32, f32, f32) {
        let i = band.index();
        (self.hue[i], self.saturation[i], self.luma[i])
    }
}

/// Read back through the clamps: a document from a wider travel never carries a point off the wheel.
impl<'de> Deserialize<'de> for Spectrogram {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let map = tolerant::object(Value::deserialize(d)?, "Spectrogram")?;
        let hue = tolerant::opt::<[f32; 8], D::Error>(&map, "hue")?.unwrap_or_default();
        let saturation = tolerant::opt::<[f32; 8], D::Error>(&map, "saturation")?.unwrap_or_default();
        let luma = tolerant::opt::<[f32; 8], D::Error>(&map, "luma")?.unwrap_or_default();
        let mut out = Spectrogram::default();
        for band in Band::ALL {
            let i = band.index();
            out.set(band, hue[i], saturation[i], luma[i]);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hue_stored_past_the_band_reads_back_on_its_stop() {
        let legacy = r#"{"hue":[30,0,0,0,0,0,0,0],"saturation":[0,0,0,0,0,0,0,0]}"#;
        let read: Spectrogram = serde_json::from_str(legacy).unwrap();
        assert_eq!(read.hue[0], Band::Red.hue_range().1);
        assert_eq!(Band::Red.hue_range(), (-57.0, 27.0));
        // Twin: a hue inside the band is read as written.
        let inside: Spectrogram = serde_json::from_str(r#"{"hue":[12,0,0,0,0,0,0,0]}"#).unwrap();
        assert_eq!(inside.hue[0], 12.0);
        assert!(serde_json::from_str::<Spectrogram>(r#"{"hue":[1,2,3]}"#).is_err());
    }
}
