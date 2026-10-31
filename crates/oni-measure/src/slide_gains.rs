//! A slide's gains alone: its cells, or a whole roll's pooled, those that cannot speak of the light
//! left out, red's and blue's medians brought onto green's, its windows left at rest (owner's call).

use crate::auto_levels::{REFERENCE, stops_for_shift};
use oni_settings::ConversionMode;
use oni_settings::blocks::Gains;

/// The counted window's ends a cell's channel may not touch: under the floor it is the sensor's
/// black, over the ceiling a clipped light, and neither holds the light's colour.
pub const FLOOR: f32 = 0.02;
pub const CEILING: f32 = 0.98;
/// The share of the usable cells kept, those nearest the cells' typical colour: a saturated subject
/// departs from it, the light's cast does not.
pub const NEAREST: f32 = 0.5;

fn median(mut values: Vec<f32>) -> f32 {
    values.sort_by(f32::total_cmp);
    values[values.len() / 2]
}

/// A cell's colour without its brightness: each channel's departure from the cell's own mean.
fn chroma(c: &[f32; 3]) -> [f32; 3] {
    let mean = (c[0] + c[1] + c[2]) / 3.0;
    c.map(|v| v - mean)
}

/// The cells a balance reads: none at the floor or the ceiling, then the share nearest the typical
/// colour; empty when no cell is usable.
pub fn voting(cells: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let usable: Vec<[f32; 3]> = cells.iter().copied().filter(|c| c.iter().all(|v| (FLOOR..=CEILING).contains(v))).collect();
    if usable.is_empty() {
        return usable;
    }
    let typical = [0, 1, 2].map(|k| median(usable.iter().map(|c| chroma(c)[k]).collect()));
    let off = |c: &[f32; 3]| chroma(c).iter().zip(typical).map(|(a, b)| (a - b) * (a - b)).sum::<f32>();
    let mut ranked = usable;
    ranked.sort_by(|a, b| off(a).total_cmp(&off(b)));
    let kept = ((ranked.len() as f32 * NEAREST).ceil() as usize).max(1);
    ranked.truncate(kept);
    ranked
}

/// The gains bringing the voting cells' red and blue medians onto green's, from rest; `None` when
/// no cell votes. Cells are read at rest gains, on the counted 0…1 window.
pub fn gains(cells: &[[f32; 3]], mode: ConversionMode) -> Option<[f32; 3]> {
    let kept = voting(cells);
    if kept.is_empty() {
        return None;
    }
    let medians = [0, 1, 2].map(|k| median(kept.iter().map(|c| c[k]).collect()));
    Some([0, 1, 2].map(|k| {
        let travel = if k == REFERENCE { 0.0 } else { medians[REFERENCE] - medians[k] };
        (Gains::BASE + stops_for_shift(travel, mode)).clamp(Gains::RANGE.0, Gains::RANGE.1)
    }))
}
