//! Stages 4 to 7: the fused kernel's arguments as `Pipeline.apply` passes them, with the models they
//! read (the points reaching the windows, the dye separation, chroma decompression, the roll's factor).

use oni_render::{ColorMatrix, Negative, Stage47};
use oni_settings::axis::{density_axis, graduation, pipeline};
use oni_settings::levels::{Levels, LevelsSet};
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, PipelineSettings};

pub type M3 = [[f32; 3]; 3];

pub const IDENTITY: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// The light a positive read as its own negative is inverted about: a slide's densest film, about
/// 1e-3 of a camera scan's light, lands near a negative's clear base (owner's experiment).
pub const INVERSION: f32 = 5e-4;
/// The darkest light inverted, past which the sensor's black would read as endless density.
pub const INVERSION_FLOOR: f32 = INVERSION / 64.0;

mod decompression;
mod separation;

pub use decompression::*;
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

/// The fused kernel's arguments, in `Pipeline.apply`'s order: the dyes, Lighting on the negative, the windows and the
/// print at its rest of every mode; a black and white print, no channel's own tone; the frame's paper in place of the
/// windows and the print, the print's ends its own; a slide on a paper, its internegative's.
pub fn stage47(s: &PipelineSettings) -> Stage47 {
    if let Some(copy) = crate::paper::internegative::copy(s) {
        return stage47(&copy.settings);
    }
    printed(s).0
}

/// The frame's paper and the enlarger's filtration, as stage 4-7 prints it; none on ONI's own print.
pub fn printing(s: &PipelineSettings) -> Option<crate::paper::Printing> {
    if let Some(copy) = crate::paper::internegative::copy(s) {
        return printing(&copy.settings);
    }
    printed(s).1
}

/// `k` printed on the frame's paper, with its printing; on ONI's print, as it is.
fn on_paper(s: &PipelineSettings, k: Stage47) -> (Stage47, Option<crate::paper::Printing>) {
    match crate::paper::printing(s, &k) {
        Some(p) => (Stage47 { print_on: 0.0, hunt: 0.0, paper: Some(p.paper), ..k }, Some(p)),
        None => (k, None),
    }
}

/// Stage 4-7's arguments on the frame's print, the saturation curve reading the lightness that print gives a grey,
/// which no colour tool moves: read on the print without the curve.
fn printed(s: &PipelineSettings) -> (Stage47, Option<crate::paper::Printing>) {
    let k = lit(s);
    let Some(n) = k.negative.filter(|n| n.curve.is_some()) else { return on_paper(s, k) };
    let (plain, _) = on_paper(s, Stage47 { negative: Some(Negative { curve: None, ..n }), ..k });
    let lightness = crate::negative::lightness(&n, &plain);
    on_paper(s, Stage47 { negative: Some(Negative { lightness, ..n }), ..k })
}

/// Stage 4-7's arguments before any paper, Lighting and the colour tools on the negative.
fn lit(s: &PipelineSettings) -> Stage47 {
    let on_negative = !crate::negative::rests(s);
    let k = unprinted(&PipelineSettings { print: s.print.unlit(), ..s.clone() }, on_negative && crate::negative::reads_amounts(s));
    Stage47 { negative: crate::negative::negative(s, &k), ..k }
}

/// The stops ONI's print curve takes at rest on a negative: the curve its windows and reference grey are read under.
pub const NEGATIVE_PRINT_STOPS: f32 = 1.5;

/// The stops ONI's print curve takes at its rest in `mode`: a negative's `NEGATIVE_PRINT_STOPS`, a slide's none.
pub fn resting_print_stops(mode: ConversionMode) -> f32 {
    if mode.reads_positive() { 0.0 } else { NEGATIVE_PRINT_STOPS }
}

/// The stops ONI's print curve takes on `s`, Lighting lying on the negative: its rest, or a legacy print's machine
/// entry, the negative's stops left out as its look was fitted.
fn print_stops(s: &PipelineSettings) -> f32 {
    let rest = resting_print_stops(s.mode);
    if s.legacy().is_some() { rest - NEGATIVE_PRINT_STOPS } else { rest }
}

/// Stage 4-7's arguments before any paper and Lighting: the reading, its dyes, the windows and ONI's print, the dyes'
/// amounts read as they are on a paper or where Lighting reads them before their tone (`amounts`).
fn unprinted(s: &PipelineSettings, amounts: bool) -> Stage47 {
    let print = &s.print;
    let mono = s.is_monochrome();
    let (decompression, film_share) = decompression_weights(s);
    let ceiling = rendered_ceiling(s);
    let pedestal = density_axis::pedestal(ceiling);
    let on_paper = s.paper().is_some() && !print.bypassed;
    let dyes = if on_paper || amounts { stock_amounts(s) } else { stock_separation(s) };
    let windows = s.levels.windows();
    Stage47 {
        gain: s.gains.stops.map(f32::exp2),
        red: vector(&s.levels.red),
        green: vector(&s.levels.green),
        blue: vector(&s.levels.blue),
        invert: s.mode.invert_flag(),
        pedestal,
        log_guard: density_axis::log_guard(ceiling),
        window: pipeline::DMAX,
        low_points: windows.map(|w| w.shadow_ordinate()),
        high_points: windows.map(|w| w.highlight_ordinate()),
        print_on: if print.bypassed { 0.0 } else { 1.0 },
        print_stops: print_stops(s),
        print_contrast: print.contrast,
        print_zones: [print.shadows, print.highlights],
        own_stops: print.filters(),
        own_contrast: if mono { [0.0; 3] } else { print.channel_contrast },
        own_mid: if mono { [0.0; 3] } else { print.channel_mid },
        hunt: if mono || !s.print.colour_compensation { 0.0 } else { 1.0 },
        decompression,
        balance_low: windows.map(|w| w.black),
        balance_span: windows.map(|w| signed_span(&w)),
        film_share,
        dyes,
        negative: None,
        paper: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_monochrome_frame_carries_no_hunt_or_unmix() {
        let s = PipelineSettings::rest(ConversionMode::Monochrome, true);
        let k = stage47(&s);
        assert_eq!((k.hunt, k.dyes), (0.0, None));
        // Twin: a colour negative at rest carries Hunt and its measured dyes, wholly unmixed.
        let n = stage47(&PipelineSettings::rest(ConversionMode::Negative, true));
        assert_eq!(n.hunt, 1.0);
        assert_eq!(n.dyes.map(|d| d.dose), Some(1.0));
        assert_eq!((n.decompression, n.film_share), (0.5, 0.5));
    }
}
