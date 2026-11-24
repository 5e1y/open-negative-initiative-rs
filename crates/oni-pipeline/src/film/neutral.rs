//! The neutral film, as its file holds it: the range its profile states, its kernel, and its curve on the log exposure
//! Lighting reads a negative back through.

use super::{CEILING, GREY, saturation};
use oni_render::{NeutralNegative, cpu};
use oni_settings::VirtualStock;
use oni_settings::axis::{density_axis, pipeline};
use serde::Deserialize;

#[derive(Deserialize)]
struct File {
    data: Data,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Data {
    range: f32,
}

/// The log exposure about a grey the curve spans: from a hundred-thousandth of the white to four stops past it.
const CURVE_SPAN: (f64, f64) = (-4.0, 2.1);

/// The density from the bare film to the sensor's white a neutral film's `profile` states, or why it does not read:
/// over none and within the density axis, which keeps its scan over the pedestal taken off it.
pub fn range(profile: &str) -> Result<f32, String> {
    let range = serde_json::from_str::<File>(profile).map_err(|e| e.to_string())?.data.range;
    if !(range > 0.0 && range <= pipeline::DMAX) {
        return Err(format!("a neutral film's range lies over 0 and up to {} of density", pipeline::DMAX));
    }
    Ok(range)
}

/// The kernel of a film of `range` at rest exposure: the sensor's saturation on the code's 1, read on a virtual
/// negative's ceiling.
pub fn kernel_of(range: f32) -> NeutralNegative {
    NeutralNegative { per_white: 1.0 / saturation(), range, veil: density_axis::pedestal(CEILING) }
}

/// `stock`'s kernel at rest exposure where its file makes it a neutral film; none on a data sheet's.
pub fn kernel(stock: VirtualStock) -> Option<NeutralNegative> {
    let entry = stock.entry();
    entry.neutral.then(|| kernel_of(range(&entry.profile).expect("a profile reads once its file is taken")))
}

/// `k` at `stops` of film exposure.
pub fn exposed(k: NeutralNegative, stops: f32) -> NeutralNegative {
    NeutralNegative { per_white: k.per_white * stops.exp2(), ..k }
}

/// A film of `range`'s density on `nodes` log exposures about a grey, the three layers alike, as a data sheet's curves
/// are held.
pub fn curve(range: f32, nodes: usize) -> (Vec<f64>, Vec<[f64; 3]>) {
    let k = kernel_of(range);
    let le: Vec<f64> = (0..nodes).map(|i| CURVE_SPAN.0 + (CURVE_SPAN.1 - CURVE_SPAN.0) * i as f64 / (nodes - 1) as f64).collect();
    let density = le.iter().map(|e| [f64::from(k.range * cpu::screen_code(GREY * 10f32.powf(*e as f32) * k.per_white)); 3]).collect();
    (le, density)
}
