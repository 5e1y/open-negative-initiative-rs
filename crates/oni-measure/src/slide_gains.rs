//! A slide's gains from a white the hand picked: its colour brought to grey and its level kept, its
//! windows left at rest; no pick, no gains (owner's call).

use crate::auto_levels::{REFERENCE, stops_for_shift};
use oni_settings::ConversionMode;
use oni_settings::blocks::Gains;

/// The counted window's ends a picked channel may not touch: under the floor it is the sensor's
/// black, over the ceiling a clipped light, and neither holds the light's colour.
pub const FLOOR: f32 = 0.02;
pub const CEILING: f32 = 0.98;

/// Whether a reading on the counted window can speak of the light: no channel black nor clipped.
pub fn usable(fractions: &[f32; 3]) -> bool {
    fractions.iter().all(|v| (FLOOR..=CEILING).contains(v))
}

/// The gains bringing a picked white to grey, from rest, their mean held so only its colour moves:
/// the bare light is brighter than the film's white. `None` when it is black or clipped; the white is
/// read at rest gains, on the counted 0…1 window.
pub fn white(fractions: [f32; 3], mode: ConversionMode) -> Option<[f32; 3]> {
    if !usable(&fractions) {
        return None;
    }
    let onto_green = [0, 1, 2].map(|k| if k == REFERENCE { 0.0 } else { stops_for_shift(fractions[REFERENCE] - fractions[k], mode) });
    let mean = onto_green.iter().sum::<f32>() / 3.0;
    Some(onto_green.map(|s| (Gains::BASE + s - mean).clamp(Gains::RANGE.0, Gains::RANGE.1)))
}
