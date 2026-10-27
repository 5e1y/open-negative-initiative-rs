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

mod decompression;
mod points;
mod separation;

pub use decompression::*;
pub use points::*;
pub use separation::*;

fn vector(w: &Levels) -> [f32; 3] {
    [w.black, w.white, w.gamma()]
}

/// The ceiling the render reads: a positive's rests on the axis' base, a plain RAW's (owner's call).
pub fn rendered_ceiling(s: &PipelineSettings) -> f32 {
    if s.mode == ConversionMode::Positive { density_axis::base() } else { s.density_ceiling }
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
    let ceiling = rendered_ceiling(s);
    let pedestal = density_axis::pedestal(ceiling);
    Stage47 {
        gain: s.gains.stops.map(f32::exp2),
        red: vector(&lv.red),
        green: vector(&lv.green),
        blue: vector(&lv.blue),
        invert: s.mode.invert_flag(),
        density: if mono { 1.0 } else { color_density::factor(s.density) },
        pedestal,
        log_guard: density_axis::log_guard(ceiling),
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
