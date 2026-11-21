//! Stages 4 to 7: the fused kernel's arguments as `Pipeline.apply` passes them, with the models they
//! read (the points reaching the windows, the dye separation, chroma decompression, the roll's factor).

use oni_render::{ColorMatrix, Stage47};
use oni_settings::axis::{density_axis, graduation, pipeline};
use oni_settings::levels::{Levels, LevelsSet};
use oni_settings::tone::{Print, Tone, color_density};
use oni_settings::PipelineSettings;
#[cfg(test)]
use oni_settings::ConversionMode;
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

/// A digital photo's scene lit at its film's own white, relative to the white its decode stands on, as the
/// film sees a scene and its print balances what it leaves; none on a film mode.
pub fn scene_white(s: &PipelineSettings) -> Option<ColorMatrix> {
    let film = s.film()?;
    let anchor = oni_settings::white::anchor(s.decoded_from_mosaic);
    let m = oni_settings::white::relative(oni_settings::white::uv(film.stock.white()), anchor);
    let row = |r: [f32; 3]| [r[0], r[1], r[2], 0.0];
    Some(ColorMatrix { r_vector: row(m[0]), g_vector: row(m[1]), b_vector: row(m[2]), ..ColorMatrix::default() })
}

/// The fused kernel's arguments, in `Pipeline.apply`'s order: the windows, dyes and print of every mode; a
/// black and white print, no channel's own tone; the frame's paper, windowed, in place of the scan's windows.
pub fn stage47(s: &PipelineSettings) -> Stage47 {
    let k = unprinted(s);
    match crate::paper::printing(s, &k, k.dyes.as_ref()) {
        Some(p) => printed_on(k, s, p.paper),
        None => k,
    }
}

/// The frame's paper and the enlarger's filtration, as stage 4-7 prints it; none on ONI's own print.
pub fn printing(s: &PipelineSettings) -> Option<crate::paper::Printing> {
    let k = unprinted(s);
    crate::paper::printing(s, &k, k.dyes.as_ref())
}

/// Stage 4-7's arguments before any paper: the reading, its dyes, the windows and ONI's print.
fn unprinted(s: &PipelineSettings) -> Stage47 {
    let print = &s.print.on_curve(Print::resting_exposure(s.mode));
    let mono = s.mode.is_monochrome();
    let base = print.hunt_base.unwrap_or(Tone::NEUTRAL);
    let (decompression, film_share) = decompression_weights(s);
    let ceiling = rendered_ceiling(s);
    let pedestal = density_axis::pedestal(ceiling);
    let on_paper = s.paper().is_some() && !print.bypassed;
    let dyes = if on_paper { stock_amounts(s) } else { stock_separation(s) };
    let lv = reaching(print, &s.levels, mono.then(|| graduation::window(s.mode)), mono);
    let unreached = s.levels.windows();
    let windows = lv.windows();
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
        print_on: if print.bypassed { 0.0 } else { 1.0 },
        print_stops: print.exposure,
        print_contrast: print.contrast,
        print_zones: [print.shadows, print.highlights],
        own_stops: print.filters(),
        own_contrast: if mono { [0.0; 3] } else { print.channel_contrast },
        own_mid: if mono { [0.0; 3] } else { print.channel_mid },
        hunt: if mono { 0.0 } else { 1.0 },
        hunt_contrast: 1.0,
        base_stops: base.exposure,
        base_contrast: base.contrast,
        base_zones: [base.shadows, base.highlights],
        decompression,
        balance_low: unreached.map(|w| w.black),
        balance_span: unreached.map(|w| signed_span(&w)),
        film_share,
        follow: if mono { 1.0 } else { Print::FOLLOW },
        dyes,
        paper: None,
    }
}

/// Stage 4-7 printing on `paper`, windowed on its anchors, in place of the scan's windows: its print read on the display
/// gamma, Lighting's points moving its ends, the colour they stretch held as on ONI's print, then ONI's print curve from
/// its mode's rest, which is none, its filters, Hunt blind to Contrast, and the colour density.
fn printed_on(k: Stage47, s: &PipelineSettings, paper: oni_render::Paper) -> Stage47 {
    let points = s.print.channel_points(s.mode.is_monochrome());
    let window = |c: usize| [points[c].0, 1.0 - points[c].1, 1.0];
    let (decompression, film_share) = paper_hold(s);
    Stage47 {
        red: window(0),
        green: window(1),
        blue: window(2),
        low_points: [0.25; 3],
        high_points: [0.75; 3],
        print_stops: k.print_stops - Print::resting_exposure(s.mode),
        hunt_contrast: 0.0,
        follow: 1.0,
        decompression,
        balance_low: [0.0; 3],
        balance_span: [1.0; 3],
        film_share,
        paper: Some(paper),
        ..k
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_monochrome_frame_carries_no_hunt_density_or_unmix() {
        let s = PipelineSettings::rest(ConversionMode::Monochrome, true);
        let k = stage47(&s);
        assert_eq!((k.hunt, k.density, k.follow, k.dyes), (0.0, 1.0, 1.0, None));
        // Twin: a colour negative at rest carries Hunt and its measured dyes, wholly unmixed.
        let n = stage47(&PipelineSettings::rest(ConversionMode::Negative, true));
        assert_eq!(n.hunt, 1.0);
        assert_eq!(n.dyes.map(|d| d.dose), Some(1.0));
        assert_eq!((n.decompression, n.film_share), (0.5, 0.5));
    }
}
