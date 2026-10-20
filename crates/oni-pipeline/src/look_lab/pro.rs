//! The neutral set's target: ONI Pro at rest on a balanced synthetic roll, each negative read both as
//! the Frontier's cube input and through Pro's own CPU references to display code.

use crate::conversion;
use crate::finishing;
use crate::scanner::{self, CHECK_GREY, MACHINE};
use oni_render::{ScannerEncode, cpu};
use oni_settings::PipelineSettings;
use oni_settings::modes::Face;

use super::measure::SPREAD;

/// The grid's half reach in density around the roll's grey, and its step.
pub const REACH: f64 = 0.75;
pub const STEP: f64 = 0.05;

/// The roll `scanner::check_roll` balances, on the Frontier and on Pro.
pub fn rolls() -> (PipelineSettings, PipelineSettings) {
    let frontier = scanner::check_roll();
    let pro = PipelineSettings { face: Face::Pro, ..frontier.clone() };
    (frontier, pro)
}

/// The Frontier's cube input for a scanned pixel, `None` where the reach or the domain clamps it.
pub fn cube_input(pixel: [f32; 4], frontier: &PipelineSettings) -> Option<[f64; 3]> {
    let k = scanner::fitted_density(frontier);
    let out = cpu::scanner_density(pixel, &k);
    let x: [f64; 3] = std::array::from_fn(|c| f64::from(k.low) + f64::from(k.span) * f64::from(out[c]));
    let (_, high) = MACHINE.reached();
    let inside = (0..3).all(|c| out[c] > 1e-6 && x[c] < f64::from(high[c]));
    inside.then_some(x)
}

/// Pro at rest on a scanned pixel, to display sRGB code: the conversion, the points' saturation, the clip.
pub fn pro_code(pixel: [f32; 4], pro: &PipelineSettings) -> [f64; 3] {
    let mut out = cpu::stage47(pixel, &conversion::stage47(pro));
    if let Some(k) = finishing::saturation(&finishing::points_curve(&pro.print)) {
        out = cpu::saturation_curve(out, &k);
    }
    let code = cpu::scanner_encode(cpu::output_clip(out), &ScannerEncode { root: 1.0 });
    [code[0], code[1], code[2]].map(f64::from)
}

/// Every negative of the grid around the roll's grey that the Frontier reads unclamped, with its
/// channels within `SPREAD` of each other at the cube input: (cube input, Pro's code).
pub fn grid() -> Vec<([f64; 3], [f64; 3])> {
    let (frontier, pro) = rolls();
    let lift = CHECK_GREY.map(|g| f64::from(g - CHECK_GREY[1]));
    let grey = f64::from(CHECK_GREY[1]);
    let n = (2.0 * REACH / STEP).round() as usize + 1;
    let mut out = Vec::new();
    for i in 0..n * n * n {
        let u = [i % n, (i / n) % n, i / (n * n)].map(|k| -REACH + STEP * k as f64);
        let pixel: [f32; 4] = std::array::from_fn(|c| if c < 3 { 10f64.powf(-(grey + u[c] + lift[c])) as f32 } else { 1.0 });
        let Some(x) = cube_input(pixel, &frontier) else { continue };
        let spread = x.iter().cloned().fold(f64::MIN, f64::max) - x.iter().cloned().fold(f64::MAX, f64::min);
        if spread < SPREAD {
            out.push((x, pro_code(pixel, &pro)));
        }
    }
    out
}
