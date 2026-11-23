//! The colour mixer's eight hue bands: each band's doses of hue, saturation and luma, and how much of a hue each owns.
//! The negative lays them on the scene (`negative`).

use super::*;

/// The eight bands' doses, each already divided by its band's authority as the mixer packs them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spectrogram {
    pub hue_low: V4,
    pub hue_high: V4,
    pub sat_low: V4,
    pub sat_high: V4,
    pub luma_low: V4,
    pub luma_high: V4,
}

impl Spectrogram {
    pub fn rows(&self) -> Vec<V4> {
        vec![self.hue_low, self.hue_high, self.sat_low, self.sat_high, self.luma_low, self.luma_high]
    }

    /// The hue turn in degrees, the saturation's boost and the luma's lift a hue in degrees takes, the eight bands'
    /// weights normalised.
    pub fn doses(&self, hue: f32) -> V3 {
        let mut w_low: V4 = std::array::from_fn(|i| band_weight(hue, i));
        let mut w_high: V4 = std::array::from_fn(|i| band_weight(hue, i + 4));
        let total = w_low.iter().sum::<f32>() + w_high.iter().sum::<f32>();
        if total > 0.0 {
            w_low = w_low.map(|w| w / total);
            w_high = w_high.map(|w| w / total);
        }
        [
            dot4(w_low, self.hue_low) + dot4(w_high, self.hue_high),
            dot4(w_low, self.sat_low) + dot4(w_high, self.sat_high),
            dot4(w_low, self.luma_low) + dot4(w_high, self.luma_high),
        ]
    }
}

/// Fixed, so editing a band never changes which colours it catches.
const BAND_CENTRE: [f32; 8] = [0.0, 30.0, 60.0, 120.0, 180.0, 240.0, 270.0, 300.0];

/// How far past its neighbour a band owns a hue: stopping at its centre turns a hue edge into a lightness one.
const BAND_SPREAD: f32 = 1.8;

fn band_weight(hue: f32, i: usize) -> f32 {
    let centre = BAND_CENTRE[i];
    let mut delta = hue - centre;
    if delta > 180.0 {
        delta -= 360.0;
    }
    if delta < -180.0 {
        delta += 360.0;
    }
    let neighbour = if delta >= 0.0 { (i + 1) & 7 } else { (i + 7) & 7 };
    let mut span = (BAND_CENTRE[neighbour] - centre).abs();
    if span > 180.0 {
        span = 360.0 - span;
    }
    smoothstep(0.0, 1.0, (1.0 - delta.abs() / (span * BAND_SPREAD)).max(0.0))
}
