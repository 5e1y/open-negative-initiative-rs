//! `FilterBalance`: the print's light read by Shades of Gray, each channel's sixth-power mean of the
//! rendered frame, brought onto green's by Temperature and Tint; Balance strength then doses it.

use oni_settings::tone::{Print, Tone};
use oni_settings::PipelineSettings;

/// Shades of Gray's exponent, the Minkowski mean that estimates a scene's light best over a large
/// calibrated set (Finlayson and Trezzi).
pub const MINKOWSKI: f32 = 6.0;

/// The side the rendered frame is read at: the means are of areas, not of grain.
pub const READ_SIDE: f64 = 480.0;

/// The Newton step's probe on each axis, and its most iterations.
pub const STEP: f32 = 0.02;
pub const ITERATIONS: usize = 8;

/// `Print.filters(temperature:tint:)`: the three exposures the two axes write.
pub fn axes_to_filters(temperature: f32, tint: f32) -> [f32; 3] {
    let (a, b) = (Print::TEMPERATURE_AXIS, Print::TINT_AXIS);
    [temperature * a[0] + tint * b[0], temperature * a[1] + tint * b[1], temperature * a[2] + tint * b[2]]
}

/// `Print.temperature` and `Print.tint` read off three exposures.
pub fn filters_to_axes(f: [f32; 3]) -> (f32, f32) {
    (f[0] - f[2], (f[0] + f[2]) / 2.0 - f[1])
}

/// A pixel the print can show; a dust past white rides through the filters and holds the means apart.
pub fn printable(p: [f32; 3]) -> bool {
    p.iter().all(|v| *v < 1.0) && finite(p)
}

/// Every pixel kept, the twin a check reads to see the dust it should leave out.
pub fn finite(p: [f32; 3]) -> bool {
    !p.iter().any(|v| v.is_nan()) && p[0].min(p[1]).min(p[2]).is_finite()
}

/// The settings a press reads: Lighting at rest but for its filters, so a press does not move with the light dialled.
pub fn unlit(s: &PipelineSettings) -> PipelineSettings {
    let mut out = s.clone();
    out.print = Print { red: s.print.red, green: s.print.green, blue: s.print.blue, ..Print::default() };
    out
}

/// The three filters written as the press writes them, each a bare exposure.
pub fn filtered(s: &mut PipelineSettings, f: [f32; 3]) {
    s.print.red = Tone::exposure(f[0]);
    s.print.green = Tone::exposure(f[1]);
    s.print.blue = Tone::exposure(f[2]);
}

/// Each channel's Minkowski mean over the kept pixels, in f64 as the sum runs; `None` on no pixel.
pub fn minkowski(pixels: &[[f32; 3]]) -> Option<[f32; 3]> {
    if pixels.is_empty() {
        return None;
    }
    let p = f64::from(MINKOWSKI);
    let mut sums = [0.0f64; 3];
    for pixel in pixels {
        for c in 0..3 {
            sums[c] += f64::from(pixel[c].max(0.0)).powf(p);
        }
    }
    let count = pixels.len() as f64;
    Some(sums.map(|s| (s / count).powf(1.0 / p) as f32))
}

/// The full correction: Temperature and Tint bringing red's and blue's sixth-power means onto green's,
/// Newton on the kernel's own output through `read`; none in monochrome.
pub fn filters(monochrome: bool, mut read: impl FnMut([f32; 3]) -> Option<[f32; 3]>) -> Option<[f32; 3]> {
    if monochrome {
        return Some([0.0; 3]);
    }
    let mut error = |e: [f32; 2]| -> Option<[f32; 2]> {
        let means = read(axes_to_filters(e[0], e[1]))?;
        let green = means[1].max(1e-6);
        Some([(means[0].max(1e-6) / green).ln(), (means[2].max(1e-6) / green).ln()])
    };
    let low = [Print::TEMPERATURE_RANGE.0, Print::TINT_RANGE.0];
    let high = [Print::TEMPERATURE_RANGE.1, Print::TINT_RANGE.1];
    let mut e = [0.0f32; 2];
    for _ in 0..ITERATIONS {
        let here = error(e)?;
        if !(here[0].abs() > 1e-4 || here[1].abs() > 1e-4) {
            break;
        }
        let t = error([e[0] + STEP, e[1]])?;
        let u = error([e[0], e[1] + STEP])?;
        let dt = [(t[0] - here[0]) / STEP, (t[1] - here[1]) / STEP];
        let du = [(u[0] - here[0]) / STEP, (u[1] - here[1]) / STEP];
        let det = dt[0] * du[1] - du[0] * dt[1];
        if !(det.abs() > 1e-8) {
            break;
        }
        let delta = [(du[0] * here[1] - here[0] * du[1]) / det, (dt[1] * here[0] - dt[0] * here[1]) / det];
        // Held at the track's stop, never refused: a cast past it still moves as far as it can.
        e = [(e[0] + delta[0]).max(low[0]).min(high[0]), (e[1] + delta[1]).max(low[1]).min(high[1])];
    }
    Some(axes_to_filters(e[0], e[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newton_lands_a_linear_cast_on_grey_and_monochrome_takes_nothing() {
        // A frame whose means move by exp of each filter: the press must undo the cast exactly.
        let cast = [0.3f32, 0.0, -0.2];
        let read = |f: [f32; 3]| Some([0, 1, 2].map(|c| 0.4 * (cast[c] + f[c]).exp()));
        let found = filters(false, read).unwrap();
        let means = read(found).unwrap();
        assert!((means[0] / means[1] - 1.0).abs() < 1e-4 && (means[2] / means[1] - 1.0).abs() < 1e-4, "{means:?}");
        assert!((found[0] + found[1] + found[2]).abs() < 1e-6, "no common part: {found:?}");
        assert_eq!(filters(true, read), Some([0.0; 3]));
        // Twin: left at zero the same frame stays parted.
        let bare = read([0.0; 3]).unwrap();
        assert!((bare[0] / bare[1] - 1.0).abs() > 0.1);
    }

    #[test]
    fn the_axes_round_trip_through_the_filters() {
        let f = axes_to_filters(0.42, -0.17);
        let (t, u) = filters_to_axes(f);
        assert!((t - 0.42).abs() < 1e-6 && (u + 0.17).abs() < 1e-6);
        assert!(!printable([1.0, 0.2, 0.2]) && printable([0.99, 0.2, 0.2]) && finite([5.0, 0.0, 0.0]));
    }
}
