//! Stages 4 to 7: the fused kernel's arguments as `Pipeline.apply` passes them, with the models they
//! read (the points reaching the windows, the dye separation, chroma decompression, the roll's factor).

use oni_render::{ColorMatrix, Stage47};
use oni_settings::axis::{density_axis, graduation, pipeline};
use oni_settings::blocks::SeparationFamily;
use oni_settings::levels::{Levels, LevelsSet};
use oni_settings::modes::Face;
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
    if s.filmed().is_some() {
        crate::film::CEILING
    } else if s.mode.reads_positive() {
        density_axis::base()
    } else {
        s.density_ceiling
    }
}

/// The windows the kernel reads before the points reach them: a digital photo has no film to place
/// them on, so its stored ones wait, unrendered, for the film mode they were placed in.
/// A filmed one's virtual negative is read on its film's own windows.
pub fn placed_levels(s: &PipelineSettings) -> LevelsSet {
    match s.filmed() {
        Some(f) => f.levels(),
        None if s.mode.is_film() => s.levels,
        None => LevelsSet::neutral(s.mode, density_axis::base()),
    }
}

/// A digital photo's balance on its scene's white, relative to the white its decode stands on; none
/// at that white, on a film or with no white chosen, so those renders hold no stage for it.
pub fn white(s: &PipelineSettings) -> Option<ColorMatrix> {
    let chosen = s.scene_white().filter(|_| !s.mode.is_film())?;
    let anchor = oni_settings::white::anchor(s.decoded_from_mosaic);
    // Compared as the sliders read it, so a white reset to the anchor's own lays no stage.
    if chosen == oni_settings::white::of_uv(anchor) {
        return None;
    }
    let m = oni_settings::white::relative(oni_settings::white::uv(chosen), anchor);
    let row = |r: [f32; 3]| [r[0], r[1], r[2], 0.0];
    Some(ColorMatrix { r_vector: row(m[0]), g_vector: row(m[1]), b_vector: row(m[2]), ..ColorMatrix::default() })
}

/// The fused kernel's arguments, in `Pipeline.apply`'s order: a digital photo has no window, dyes or filter,
/// rests Lighting for its levels and curves; a black and white print, no channel's own tone.
pub fn stage47(s: &PipelineSettings) -> Stage47 {
    if s.mode.is_film() { fused(s, &s.print, s.density) } else { fused(s, &s.unlit_print(), color_density::BASE) }
}

/// Lighting on a digital photo's levels and curves: the print, each window lane its points' reach on the
/// picture's 0 to 1. None on a film, a bypassed print, or with Lighting, channel tone and Saturation at rest.
pub fn lighting(s: &PipelineSettings) -> Option<Stage47> {
    if s.mode.is_film() || s.print.bypassed {
        return None;
    }
    let mut lit = fused(s, &s.print, s.density);
    // A filmed photo's filters are laid with its negative, in stage 4-7: Lighting never lays them twice.
    if s.filmed().is_some() {
        lit.own_stops = [0.0; 3];
    }
    if lit == stage47(s) {
        return None;
    }
    let unit = LevelsSet { red: Levels::default(), green: Levels::default(), blue: Levels::default() };
    let reach = reaching(&s.print, &unit, None, false);
    Some(Stage47 { red: vector(&reach.red), green: vector(&reach.green), blue: vector(&reach.blue), ..lit })
}

/// The gains the kernel renders, in stops: a film's own, or the machine's keys on a digital photo's Scanner
/// face, on its virtual negative's own; a digital photo's stored gains wait, unrendered, for the film mode
/// they were placed in.
pub fn rendered_gains(s: &PipelineSettings) -> [f32; 3] {
    let film = s.filmed().map_or([0.0; 3], |f| f.stops);
    if s.mode.is_film() {
        s.gains.stops
    } else if s.face == Face::Scanner {
        let keys = crate::scanner::gain_stops(s);
        std::array::from_fn(|c| film[c] + keys[c])
    } else {
        film
    }
}

/// The kernel's arguments with `print` and the colour density's `density` in the frame's own place.
fn fused(s: &PipelineSettings, print: &Print, density: f32) -> Stage47 {
    let mono = s.mode.is_monochrome();
    let film = s.mode.is_film();
    let levels = placed_levels(s);
    let lv = reaching(print, &levels, mono.then(|| graduation::window(s.mode)), mono);
    let sep = separation_rows(s.separation_family, separation_dose(s), s.mode);
    let base = print.hunt_base.unwrap_or(Tone::NEUTRAL);
    let (decompression, film_share) = decompression_weights(s);
    let windows = lv.windows();
    let unreached = levels.windows();
    let ceiling = rendered_ceiling(s);
    let pedestal = density_axis::pedestal(ceiling);
    Stage47 {
        gain: rendered_gains(s).map(f32::exp2),
        red: vector(&lv.red),
        green: vector(&lv.green),
        blue: vector(&lv.blue),
        invert: s.read_as().invert_flag(),
        density: if mono { 1.0 } else { color_density::factor(density) },
        pedestal,
        log_guard: density_axis::log_guard(ceiling),
        window: pipeline::DMAX,
        low_points: windows.map(|w| w.shadow_ordinate()),
        high_points: windows.map(|w| w.highlight_ordinate()),
        sep_r: sep[0],
        sep_g: sep[1],
        sep_b: sep[2],
        print_on: if print.bypassed { 0.0 } else { 1.0 },
        print_stops: print.exposure,
        print_contrast: print.contrast,
        print_zones: [print.shadows, print.highlights],
        own_stops: if film || s.filmed().is_some() { print.filters() } else { [0.0; 3] },
        own_contrast: if mono { [0.0; 3] } else { print.channel_contrast },
        own_mid: if mono { [0.0; 3] } else { print.channel_mid },
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
