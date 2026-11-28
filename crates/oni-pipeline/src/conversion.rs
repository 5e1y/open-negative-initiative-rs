//! Stages 4 to 7: the fused kernel's arguments as `Pipeline.apply` passes them, with the models they
//! read (the points reaching the windows, chroma decompression, the roll's factor).

use oni_render::{ColorMatrix, Negative, Shown, Stage47};
use oni_settings::axis::{density_axis, graduation, pipeline};
use oni_settings::levels::Levels;
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

pub use decompression::*;

fn vector(w: &Levels) -> [f32; 3] {
    [w.black, w.white, w.gamma()]
}

/// The ceiling the render reads: a positive's rests on the axis' base, a plain RAW's (owner's call).
pub fn rendered_ceiling(s: &PipelineSettings) -> f32 {
    if s.mode.reads_positive() { density_axis::base() } else { s.density_ceiling }
}

/// A digital photo's scene, in the light its decode balanced, as a film balanced for its own white sees it: a grey under
/// that white reaches the layers neutral, so a tungsten film turns a daylight scene blue; none on a film keeping the scene's own.
pub fn scene_white(s: &PipelineSettings) -> Option<ColorMatrix> {
    let white = s.film()?.stock.white()?;
    let anchor = oni_settings::white::anchor(s.decoded_from_mosaic);
    let m = oni_settings::white::relative(oni_settings::white::uv(white), anchor);
    let row = |r: [f32; 3]| [r[0], r[1], r[2], 0.0];
    Some(ColorMatrix { r_vector: row(m[0]), g_vector: row(m[1]), b_vector: row(m[2]), ..ColorMatrix::default() })
}

/// The fused kernel's arguments, in `Pipeline.apply`'s order: Lighting on the negative, the windows, then the pure
/// inversion of their shares, or the frame's paper on them, the print's ends its own; a legacy print's curve at its rest,
/// a black and white print's no channel's own tone.
pub fn stage47(s: &PipelineSettings) -> Stage47 {
    printed(s).0
}

/// The frame's paper and the enlarger's filtration, as stage 4-7 prints it; none where no paper prints.
pub fn printing(s: &PipelineSettings) -> Option<crate::paper::Printing> {
    printed(s).1
}

/// `k` printed on the frame's paper, with its printing, or shown as its pure inversion where it names none; on a legacy
/// print's own curve and for a measure upstream of the print, as it is.
fn on_paper(s: &PipelineSettings, k: Stage47) -> (Stage47, Option<crate::paper::Printing>) {
    match crate::paper::printing(s, &k) {
        Some(p) => (Stage47 { print_on: 0.0, hunt: 0.0, shown: Shown::Paper(p.paper), ..k }, Some(p)),
        None if s.shows_inversion() && !s.print.bypassed => (Stage47 { print_on: 0.0, hunt: 0.0, shown: Shown::Inversion, ..k }, None),
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
    let k = unprinted(&PipelineSettings { print: s.print.unlit(), ..s.clone() });
    Stage47 { negative: crate::negative::negative(s, &k), ..k }
}

/// The stops a legacy print's curve takes at rest on a negative: Lighting's grey is read under it.
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

/// Stage 4-7's arguments before any paper and Lighting: the reading, the windows and a legacy print's curve.
fn unprinted(s: &PipelineSettings) -> Stage47 {
    let print = &s.print;
    let mono = s.is_monochrome();
    let (decompression, film_share) = decompression_weights(s);
    let ceiling = rendered_ceiling(s);
    let pedestal = density_axis::pedestal(ceiling);
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
        hunt: if mono || !s.print.colour_compensation { 0.0 } else { 1.0 },
        decompression,
        balance_low: windows.map(|w| w.black),
        balance_span: windows.map(|w| signed_span(&w)),
        film_share,
        negative: None,
        shown: Shown::Curve,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_monochrome_frame_carries_no_hunt() {
        let on_curve = |mode| PipelineSettings { legacy_print: Some(oni_settings::LegacyPrint::default()), ..PipelineSettings::rest(mode, true) };
        assert_eq!(stage47(&on_curve(ConversionMode::Monochrome)).hunt, 0.0);
        // Twin: a colour negative on a legacy print's curve carries Hunt.
        let n = stage47(&on_curve(ConversionMode::Negative));
        assert_eq!(n.hunt, 1.0);
        assert_eq!((n.decompression, n.film_share), (0.5, 0.5));
    }

    #[test]
    fn a_positive_rests_on_the_base_ceiling_and_a_negative_on_its_own() {
        let mut s = PipelineSettings::rest(ConversionMode::Positive, true);
        s.density_ceiling = 2.4;
        assert_eq!(stage47(&s).pedestal, density_axis::pedestal(density_axis::base()));
        // Twin: a negative keeps its own ceiling.
        s.mode = ConversionMode::Negative;
        assert_eq!(stage47(&s).pedestal, density_axis::pedestal(2.4));
    }
}
