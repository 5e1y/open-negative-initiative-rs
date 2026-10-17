//! Stages 4 to 7: the fused kernel's arguments as `Pipeline.apply` passes them, with the models they
//! read (the points reaching the windows, the dye separation, chroma decompression, the roll's factor).

use oni_render::Stage47;
use oni_settings::axis::{density_axis, graduation, pipeline};
use oni_settings::blocks::SeparationFamily;
use oni_settings::levels::{Levels, LevelsSet};
use oni_settings::tone::{Print, Tone, color_density};
use oni_settings::{ConversionMode, PipelineSettings};
use std::ops::RangeInclusive;

pub type M3 = [[f32; 3]; 3];

pub const IDENTITY: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// `Print.reaching`: each window's ends moved by the held points' share of its span, held inside
/// `bounds` when given; the print bypassed or at rest leaves the windows as they are.
pub fn reaching(print: &Print, levels: &LevelsSet, bounds: Option<RangeInclusive<f32>>) -> LevelsSet {
    if print.bypassed || (print.black_point == 0.0 && print.white_point == 0.0) {
        return *levels;
    }
    let (black, white) = print.held_points();
    let reach = |w: Levels| {
        let span = w.white - w.black;
        let mut out = Levels { black: w.black + black * span, white: w.white - white * span, ..w };
        if let Some(b) = &bounds {
            out.black = out.black.clamp(*b.start(), *b.end());
            out.white = out.white.clamp(*b.start(), *b.end());
        }
        out
    };
    LevelsSet { red: reach(levels.red), green: reach(levels.green), blue: reach(levels.blue) }
}

/// `Separation.Family.dyeRows`, output channel per row.
pub fn dye_rows(family: SeparationFamily) -> M3 {
    match family {
        SeparationFamily::KodakC41 => [[1.0000, -0.2379, -0.0092], [-0.0346, 1.0000, -0.0136], [-0.0327, -0.0790, 1.0000]],
        SeparationFamily::FujiC41 => [[1.0000, -0.3034, -0.0059], [-0.0688, 1.0000, -0.0138], [-0.0129, -0.0655, 1.0000]],
        SeparationFamily::GenericC41 => [[1.0000, -0.2706, -0.0076], [-0.0517, 1.0000, -0.0137], [-0.0228, -0.0723, 1.0000]],
        SeparationFamily::Ecn2 => [[1.0000, -0.2706, -0.0082], [-0.0995, 1.0000, -0.0252], [0.0260, -0.1023, 1.0000]],
        SeparationFamily::E6 => [[1.0000, -0.3584, 0.0067], [-0.3109, 1.0000, -0.0588], [-0.0269, -0.2385, 1.0000]],
    }
}

fn mul(m: &M3, v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `Separation.matrix`: the identity at zero, the family's rows normalised to sum to one at one.
pub fn separation_matrix(family: SeparationFamily, strength: f32) -> M3 {
    let s = strength.clamp(oni_settings::blocks::separation::RANGE.0, oni_settings::blocks::separation::RANGE.1);
    let full = dye_rows(family).map(|r| {
        let sum = r[0] + r[1] + r[2];
        r.map(|x| x / sum)
    });
    std::array::from_fn(|i| std::array::from_fn(|j| IDENTITY[i][j] + s * (full[i][j] - IDENTITY[i][j])))
}

/// `Separation.chromaKept`: `m` divided on the plane of colours by its gain there, the square root of
/// its determinant on that plane, so a dose turns hues and keeps their mean saturation.
pub fn chroma_kept(m: M3) -> M3 {
    let u = [1.0 / 2f32.sqrt(), -1.0 / 2f32.sqrt(), 0.0];
    let v = [1.0 / 6f32.sqrt(), 1.0 / 6f32.sqrt(), -2.0 / 6f32.sqrt()];
    let (a, b, c, d) = (dot(u, mul(&m, u)), dot(u, mul(&m, v)), dot(v, mul(&m, u)), dot(v, mul(&m, v)));
    let gain = (a * d - b * c).abs().sqrt();
    if m == IDENTITY || gain <= 1e-6 {
        return m;
    }
    m.map(|r| r.map(|x| x * (1.0 / gain) + (1.0 / 3.0) * (1.0 - 1.0 / gain)))
}

/// `Separation.rows`: the named family, else the mode's resting one; monochrome has nothing to unmix.
pub fn separation_rows(family: Option<SeparationFamily>, strength: f32, mode: ConversionMode) -> M3 {
    match family.or(SeparationFamily::resting(mode)) {
        Some(f) if !mode.is_monochrome() => chroma_kept(separation_matrix(f, strength)),
        _ => IDENTITY,
    }
}

/// `Separation.dose`: narrowband passes separated the dyes in the capture.
pub fn separation_dose(s: &PipelineSettings) -> f32 {
    if s.rgb_passes.is_none() { s.separation } else { 0.0 }
}

/// `RollReference.colourFactor`: the spans the points reach over the roll's, on their geometric mean.
pub fn colour_factor(s: &PipelineSettings) -> f32 {
    let Some(reference) = &s.roll_reference else { return 1.0 };
    let own = s.levels.windows().map(|w| (w.white - w.black).abs());
    let roll: [f32; 3] = std::array::from_fn(|c| {
        (graduation::value(reference.white[c], s.mode) - graduation::value(reference.black[c], s.mode)).abs()
    });
    let (black, white) = s.print.held_points();
    let reached = if s.print.bypassed { 1.0 } else { (1.0 - black - white) / (1.0 - Print::RESTING_BLACK_POINT) };
    let product: f32 = (0..3).map(|c| own[c].max(1e-4) / roll[c].max(1e-4) * reached.max(1e-4)).product();
    product.powf(1.0 / 3.0).clamp(oni_settings::frame::RollReference::FACTOR_RANGE.0, oni_settings::frame::RollReference::FACTOR_RANGE.1)
}

/// `ChromaDecompression.weights`: the windows' colour and the film's own the kernel carries.
pub fn decompression_weights(s: &PipelineSettings) -> (f32, f32) {
    let factor = colour_factor(s);
    if s.roll_reference.is_some() {
        return (factor, 0.0);
    }
    let dose = if s.mode == ConversionMode::Negative { s.chroma_decompression.clamp(0.0, 1.0) } else { 1.0 };
    (dose * factor, 1.0 - dose)
}

/// `ChromaDecompression.signedSpan`.
pub fn signed_span(w: &Levels) -> f32 {
    let span = w.white - w.black;
    if span.abs() < 1e-4 { if span < 0.0 { -1e-4 } else { 1e-4 } } else { span }
}

fn vector(w: &Levels) -> [f32; 3] {
    [w.black, w.white, w.gamma()]
}

/// The fused kernel's arguments, in `Pipeline.apply`'s order.
pub fn stage47(s: &PipelineSettings) -> Stage47 {
    let mono = s.mode.is_monochrome();
    let lv = reaching(&s.print, &s.levels, mono.then(|| graduation::window(s.mode)));
    let sep = separation_rows(s.separation_family, separation_dose(s), s.mode);
    let base = s.print.hunt_base.unwrap_or(Tone::NEUTRAL);
    let (decompression, film_share) = decompression_weights(s);
    let windows = lv.windows();
    let unreached = s.levels.windows();
    let pedestal = density_axis::pedestal(s.density_ceiling);
    Stage47 {
        gain: s.gains.stops.map(f32::exp2),
        red: vector(&lv.red),
        green: vector(&lv.green),
        blue: vector(&lv.blue),
        invert: s.mode.invert_flag(),
        density: if mono { 1.0 } else { color_density::factor(s.density) },
        pedestal,
        log_guard: density_axis::log_guard(s.density_ceiling),
        window: pipeline::DMAX,
        low_points: windows.map(|w| w.shadow_ordinate()),
        high_points: windows.map(|w| w.highlight_ordinate()),
        sep_r: sep[0],
        sep_g: sep[1],
        sep_b: sep[2],
        print_on: if s.print.bypassed { 0.0 } else { 1.0 },
        print_stops: s.print.exposure,
        print_contrast: s.print.contrast,
        print_zones: [s.print.shadows, s.print.highlights],
        own_stops: s.print.filters(),
        hunt: if mono { 0.0 } else { 1.0 },
        base_stops: base.exposure,
        base_contrast: base.contrast,
        base_zones: [base.shadows, base.highlights],
        decompression,
        balance_low: unreached.map(|w| w.black),
        balance_span: unreached.map(|w| signed_span(&w)),
        film_share,
        follow: if mono { 1.0 } else { Print::FOLLOW },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_separation_keeps_greys_and_the_mean_saturation() {
        let rows = separation_rows(Some(SeparationFamily::KodakC41), 0.6, ConversionMode::Negative);
        for r in rows {
            assert!((r[0] + r[1] + r[2] - 1.0).abs() < 1e-6, "a grey is a fixed point: {r:?}");
        }
        assert_eq!(separation_rows(Some(SeparationFamily::KodakC41), 0.0, ConversionMode::Negative), IDENTITY);
        assert_eq!(separation_rows(None, 0.6, ConversionMode::Monochrome), IDENTITY);
        // Twin: the bare dose, without `chromaKept`, multiplies the chroma and differs.
        let bare = separation_matrix(SeparationFamily::KodakC41, 0.6);
        assert!((0..3).any(|i| (0..3).any(|j| (bare[i][j] - rows[i][j]).abs() > 1e-3)));
    }

    #[test]
    fn the_points_reach_into_each_window_and_a_bypassed_print_leaves_them() {
        let levels = LevelsSet::default();
        let print = Print { black_point: 0.1, white_point: 0.2, ..Print::default() };
        let reached = reaching(&print, &levels, None);
        assert!((reached.green.black - 0.3).abs() < 1e-6 && (reached.green.white - 2.4).abs() < 1e-6);
        let held = reaching(&print, &levels, Some(0.5..=2.0));
        assert_eq!((held.red.black, held.red.white), (0.5, 2.0));
        // Twin: the bypassed print reaches nothing.
        assert_eq!(reaching(&Print { bypassed: true, ..print }, &levels, None), levels);
    }

    #[test]
    fn a_monochrome_frame_carries_no_hunt_density_or_unmix() {
        let s = PipelineSettings::rest(ConversionMode::Monochrome, true);
        let k = stage47(&s);
        assert_eq!((k.hunt, k.density, k.follow, k.sep_r), (0.0, 1.0, 1.0, [1.0, 0.0, 0.0]));
        // Twin: a colour negative at rest carries Hunt and the resting C-41 unmix.
        let n = stage47(&PipelineSettings::rest(ConversionMode::Negative, true));
        assert_eq!(n.hunt, 1.0);
        assert_ne!(n.sep_r, [1.0, 0.0, 0.0]);
        assert_eq!((n.decompression, n.film_share), (0.5, 0.5));
    }
}
