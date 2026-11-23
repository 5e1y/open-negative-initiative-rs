//! The enlarger at its rest on a negative Lighting has moved (`crate::negative`): Brightness the enlarger's light faded in
//! over the deep shadows; the print's pick, the bare film as Lighting develops it, then laid on pure black and the paper's
//! white on pure white, or for Print lift the pick where the rest prints the bare film and the paper's white its own.

use super::printing::{bisected, luma, shares_of};
use super::*;
use oni_render::{Paper, Stage47};

/// Where Brightness reaches Exposure's whole move: the light the print holds there, its pick on black and its white
/// the paper's, L* 25; under it the blacks and the shadows, held more the darker they print.
pub const BRIGHTNESS_REACH_LIGHT: f32 = 0.044155;

/// How far past the grey, in its own shares, the reach is sought: a print that never gets there takes the end.
const SOUGHT: f32 = 8.0;

/// `p`'s print of `pick` and the paper's white, laid on pure black and pure white, or for Print lift the pick on `lift`
/// and the white its own.
pub fn laid(p: &Paper, pick: V3, lift: Option<V3>) -> Paper {
    let white = p.viewed(p.curves[0]);
    let white = if p.luma { [luma(white); 3] } else { white };
    let (anchor, white_anchor) = lift.map_or(([0.0; 3], [1.0; 3]), |a| (a, white));
    Paper { pick: p.printed(pick), white, anchor, white_anchor, ..*p }
}

/// `p` with its ends, `laid` from `pick` and `lift`, and Brightness's stops faded in up to its reach: where the neutral
/// from the pick to the `grey` prints `BRIGHTNESS_REACH_LIGHT` over the pick, on black and on the paper's white.
pub fn ended(p: &Paper, pick: V3, grey: V3, lift: Option<V3>, brightness: f32) -> Paper {
    let plain = Paper { brightness: 0.0, ..*p };
    let mean = |v: V3| (v[0] + v[1] + v[2]) / 3.0;
    let pick_log = mean(plain.exposed(plain.density(pick)));
    let ended = laid(&plain, pick, lift);
    let (k, w) = (luma(ended.pick), luma(ended.white));
    let neutral = |t: f32| std::array::from_fn(|c| pick[c] + t * (grey[c] - pick[c]));
    let light = |t: f32| (luma(ended.printed(neutral(t))) - k) / (w - k) * w;
    let t = bisected(0.0, SOUGHT, BRIGHTNESS_REACH_LIGHT, light);
    let reach = ended.toward_light(pick_log, ended.exposed(ended.density(neutral(t))));
    Paper { brightness, pick_log, reach, ..ended }
}

/// `bare` at its rest `time` (its log), through the paper's `filtration`, its `anchors` the pick, the densest read and
/// the reference grey as densities above the bare film: the pick printed where the rest prints it whatever Lighting moves.
pub(super) fn lit(bare: &Paper, time: f32, anchors: &[V3; 3], s: &PipelineSettings, k: &Stage47, filtration: V3) -> Printing {
    let at_rest = Paper { scale: time.exp(), ..*bare };
    let [pick, _, grey] = *anchors;
    let lift = super::lifted(s).then(|| at_rest.printed(shares_of(s, k, pick)));
    // The pick and the grey as Lighting develops the film, read through the window as every pixel is.
    let developed = |a: V3| shares_of(s, k, k.negative.as_ref().map_or(a, |n| n.redeveloped(a)));
    Printing { paper: ended(&at_rest, developed(pick), developed(grey), lift, s.print.brightness), filtration }
}
