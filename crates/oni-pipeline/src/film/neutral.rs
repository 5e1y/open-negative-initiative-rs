//! The neutral film, as its file holds it: the range its profile states and its tone curve, its kernel, and its curve on
//! the log exposure Lighting reads a negative back through.

use super::{CEILING, GREY, saturation};
use oni_render::{NEUTRAL_NODES, NeutralNegative, cpu};
use oni_settings::VirtualStock;
use oni_settings::axis::{density_axis, pipeline};
use serde::Deserialize;

#[derive(Deserialize)]
struct File {
    data: Data,
}

/// The range, then the tone curve: density above the bare film on log exposure about a grey.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Data {
    range: f32,
    log_exposure: Vec<f64>,
    density: Vec<f64>,
}

/// The log exposure about a grey the curve Lighting reads spans: from a hundred-thousandth of the white to four stops past it.
const CURVE_SPAN: (f64, f64) = (-4.0, 2.1);

/// The kernel at rest exposure a neutral film's `profile` states, or why it does not read: a range over none and within the
/// density axis, which keeps its scan over the pedestal taken off it, and a curve rising from nought past the white.
pub fn read(profile: &str) -> Result<NeutralNegative, String> {
    let d = serde_json::from_str::<File>(profile).map_err(|e| e.to_string())?.data;
    if !(d.range > 0.0 && d.range <= pipeline::DMAX) {
        return Err(format!("a neutral film's range lies over 0 and up to {} of density", pipeline::DMAX));
    }
    let white = f64::from(saturation() / GREY).log10();
    let (le, density) = (&d.log_exposure, &d.density);
    let rising = le.windows(2).all(|w| w[1] > w[0]) && density.windows(2).all(|w| w[1] >= w[0]);
    if le.len() < 2 || le.len() != density.len() || !rising || density[0] != 0.0 || le[le.len() - 1] <= white {
        return Err("a neutral film's curve rises from nought along its log exposures, past the white".into());
    }
    let at = |e: f64| {
        let i = le.partition_point(|v| *v <= e).clamp(1, le.len() - 1);
        density[i - 1] + (density[i] - density[i - 1]) * ((e - le[i - 1]) / (le[i] - le[i - 1])).clamp(0.0, 1.0)
    };
    let (low, step) = (le[0] - white, (le[le.len() - 1] - le[0]) / (NEUTRAL_NODES - 1) as f64);
    let one = at(white);
    let curve = std::array::from_fn(|i| (at(le[0] + i as f64 * step) / one) as f32);
    Ok(NeutralNegative { per_white: 1.0 / saturation(), range: d.range, veil: density_axis::pedestal(CEILING), low: low as f32, step: step as f32, curve })
}

/// `stock`'s kernel at rest exposure where its file makes it a neutral film; none on a data sheet's.
pub fn kernel(stock: VirtualStock) -> Option<NeutralNegative> {
    let entry = stock.entry();
    entry.neutral.then(|| read(&entry.profile).expect("a profile reads once its file is taken"))
}

/// `k` at `stops` of film exposure.
pub fn exposed(k: NeutralNegative, stops: f32) -> NeutralNegative {
    NeutralNegative { per_white: k.per_white * stops.exp2(), ..k }
}

/// The film of `k`'s density on `nodes` log exposures about a grey, the three layers alike, as a data sheet's curves are
/// held.
pub fn curve(k: &NeutralNegative, nodes: usize) -> (Vec<f64>, Vec<[f64; 3]>) {
    let le: Vec<f64> = (0..nodes).map(|i| CURVE_SPAN.0 + (CURVE_SPAN.1 - CURVE_SPAN.0) * i as f64 / (nodes - 1) as f64).collect();
    let density = le.iter().map(|e| [f64::from(cpu::neutral_dyes([GREY * 10f32.powf(*e as f32); 4], k)[1]); 3]).collect();
    (le, density)
}
