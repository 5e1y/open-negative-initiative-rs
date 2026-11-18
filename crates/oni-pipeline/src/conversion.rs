//! Stages 4 to 7: the fused kernel's arguments as `Pipeline.apply` passes them, with the models they
//! read (the points reaching the windows, the dye separation, chroma decompression, the roll's factor).

use oni_render::{ColorMatrix, Stage47};
use oni_settings::axis::{density_axis, graduation, pipeline};
use oni_settings::blocks::SeparationFamily;
use oni_settings::levels::{Levels, LevelsSet};
use oni_settings::tone::{Print, Tone, color_density};
use oni_settings::{ConversionMode, PipelineSettings};
use std::ops::RangeInclusive;

pub type M3 = [[f32; 3]; 3];

pub const IDENTITY: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// The light a positive read as its own negative is inverted about: a slide's densest film, about
/// 1e-3 of a camera scan's light, lands near a negative's clear base (owner's experiment).
pub const INVERSION: f32 = 5e-4;
/// The darkest light inverted, past which the sensor's black would read as endless density.
pub const INVERSION_FLOOR: f32 = INVERSION / 64.0;

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
    if s.mode.reads_positive() { density_axis::base() } else { s.density_ceiling }
}

/// The windows the kernel reads before the points reach them: a digital photo has no film to place
/// them on, so its stored ones wait, unrendered, for the film mode they were placed in.
pub fn placed_levels(s: &PipelineSettings) -> LevelsSet {
    if s.mode.is_film() { s.levels } else { LevelsSet::neutral(s.mode, density_axis::base()) }
}

/// A digital photo's balance on its chosen white, relative to the white its decode stands on; none
/// at that white, on a film or with no white chosen, so those renders hold no stage for it.
pub fn white(s: &PipelineSettings) -> Option<ColorMatrix> {
    let chosen = s.white.filter(|_| !s.mode.is_film())?;
    let anchor = oni_settings::white::anchor(s.decoded_from_mosaic);
    // Compared as the sliders read it, so a white reset to the anchor's own lays no stage.
    if chosen == oni_settings::white::of_uv(anchor) {
        return None;
    }
    let m = oni_settings::white::relative(oni_settings::white::uv(chosen), anchor);
    let row = |r: [f32; 3]| [r[0], r[1], r[2], 0.0];
    Some(ColorMatrix { r_vector: row(m[0]), g_vector: row(m[1]), b_vector: row(m[2]), ..ColorMatrix::default() })
}

/// The fused kernel's arguments, in `Pipeline.apply`'s order. A digital photo renders no gain, window,
/// dyes or print filter: its colour is its white's; a black and white print, no channel's own tone.
pub fn stage47(s: &PipelineSettings) -> Stage47 {
    let mono = s.mode.is_monochrome();
    let film = s.mode.is_film();
    let levels = placed_levels(s);
    let lv = reaching(&s.print, &levels, mono.then(|| graduation::window(s.mode)), mono);
    let sep = separation_rows(s.separation_family, separation_dose(s), s.mode);
    let base = s.print.hunt_base.unwrap_or(Tone::NEUTRAL);
    let (decompression, film_share) = decompression_weights(s);
    let windows = lv.windows();
    let unreached = levels.windows();
    let ceiling = rendered_ceiling(s);
    let pedestal = density_axis::pedestal(ceiling);
    Stage47 {
        gain: if film { s.gains.stops.map(f32::exp2) } else { [1.0; 3] },
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
        own_stops: if film { s.print.filters() } else { [0.0; 3] },
        own_contrast: if mono { [0.0; 3] } else { s.print.channel_contrast },
        own_mid: if mono { [0.0; 3] } else { s.print.channel_mid },
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
