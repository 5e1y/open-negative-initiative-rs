//! The enlarger at its rest on a negative Lighting has moved (`crate::negative`): Brightness the enlarger's light faded in
//! over the deep shadows; the print's pick, the bare film as Lighting develops it, then laid on pure black and the
//! window's white on pure white, or for Print lift the pick where the rest prints the bare film and that white its own.

use super::printing::{bisected, luma, shares_of};
use super::*;
use oni_render::{Paper, Stage47};

/// Where Brightness reaches Exposure's whole move: the light the print holds there, its pick on black and its white
/// the paper's, L* 25; under it the blacks and the shadows, held more the darker they print.
pub const BRIGHTNESS_REACH_LIGHT: f32 = 0.044155;

/// `p`'s print of `pick` and the light `white` laid on pure black and pure white, or for Print lift the pick on `lift`
/// and that white its own.
pub fn laid(p: &Paper, pick: V3, white: V3, lift: Option<V3>) -> Paper {
    let white = if p.luma { [luma(white); 3] } else { white };
    let (anchor, white_anchor) = lift.map_or(([0.0; 3], [1.0; 3]), |a| (a, white));
    Paper { pick: p.printed(pick), white, anchor, white_anchor, ..*p }
}

/// `p` with its ends, `laid` from `pick`, `white` and `lift`, and Brightness's stops faded in up to its reach: where the
/// neutral from the pick toward `toward`, sought up to `sought` times as far, prints `BRIGHTNESS_REACH_LIGHT` over the
/// pick, on black and on that white.
pub fn ended(p: &Paper, pick: V3, toward: V3, sought: f32, white: V3, lift: Option<V3>, brightness: f32) -> Paper {
    let plain = Paper { brightness: 0.0, ..*p };
    let mean = |v: V3| (v[0] + v[1] + v[2]) / 3.0;
    let pick_log = mean(plain.exposed(plain.density(pick)));
    let ended = laid(&plain, pick, white, lift);
    let (k, w) = (luma(ended.pick), luma(ended.white));
    let neutral = |t: f32| std::array::from_fn(|c| pick[c] + t * (toward[c] - pick[c]));
    let light = |t: f32| (luma(ended.printed(neutral(t))) - k) / (w - k) * w;
    let t = bisected(0.0, sought, BRIGHTNESS_REACH_LIGHT, light);
    let reach = ended.toward_light(pick_log, ended.exposed(ended.density(neutral(t))));
    Paper { brightness, pick_log, reach, ..ended }
}

/// `at_rest`, the enlarger's light at its time through the paper's `filtration`, its ends and Brightness on the negative
/// as Lighting develops it: the pick, the bare film, printed where the rest prints it whatever Lighting moves; Brightness
/// sought toward `toward` over the bare film, `white` laid on white.
pub(super) fn lit(at_rest: &Paper, toward: V3, sought: f32, white: V3, s: &PipelineSettings, k: &Stage47, filtration: V3) -> Printing {
    let lift = super::lifted(s).then(|| at_rest.printed(shares_of(s, k, [0.0; 3])));
    // The pick and the tone sought as Lighting develops the film, read through the window as every pixel is.
    let developed = |a: V3| shares_of(s, k, k.negative.as_ref().map_or(a, |n| n.redeveloped(a)));
    Printing { paper: ended(at_rest, developed([0.0; 3]), developed(toward), sought, white, lift, s.print.brightness), filtration }
}
