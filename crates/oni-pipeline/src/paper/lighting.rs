//! The enlarger at its rest on a negative Lighting has moved (`crate::negative`): Brightness the enlarger's light faded in
//! over the deep shadows; the print's pick, the bare film as Lighting develops it, then laid on pure black and the paper's
//! white on pure white, or for Print lift the pick where the rest prints the bare film and the paper's white its own.

use super::printing::{bisected, filters, luma, root};
use super::*;
use oni_render::{Negative, Paper};
use oni_settings::PipelineSettings;

/// Where Brightness reaches Exposure's whole move: the light the print holds there, its pick on black and its white
/// the paper's, L* 25; under it the blacks and the shadows, held more the darker they print.
pub const BRIGHTNESS_REACH_LIGHT: f32 = 0.044155;

/// How far past the grey, in its own amounts, the reach is sought: a print that never gets there takes the end.
const SOUGHT: f32 = 8.0;

/// `p`'s print of `pick` and the paper's white, at its colour density, laid on pure black and pure white, or for Print
/// lift the pick on `lift` and the white its own.
pub fn laid(p: &Paper, pick: V3, lift: Option<V3>) -> Paper {
    let white = p.viewed(p.curves[0]);
    let white = p.coloured(if p.luma { [luma(white); 3] } else { white });
    let (anchor, white_anchor) = lift.map_or(([0.0; 3], [1.0; 3]), |a| (a, white));
    Paper { pick: p.coloured(p.printed(pick)), white, anchor, white_anchor, ..*p }
}

/// `p` with its ends, `laid` from `pick` and `lift`, and Brightness's stops faded in up to its reach: where the neutral
/// from the pick to the `grey` prints `BRIGHTNESS_REACH_LIGHT` over the pick, on black and on the paper's white.
pub fn ended(p: &Paper, pick: V3, grey: V3, lift: Option<V3>, brightness: f32) -> Paper {
    let plain = Paper { brightness: 0.0, ..*p };
    let mean = |v: V3| (v[0] + v[1] + v[2]) / 3.0;
    let pick_log = mean(plain.exposed(pick));
    let ended = laid(&plain, pick, lift);
    let (k, w) = (luma(ended.pick), luma(ended.white));
    let neutral = |t: f32| std::array::from_fn(|c| pick[c] + t * (grey[c] - pick[c]));
    let light = |t: f32| (luma(ended.coloured(ended.printed(neutral(t)))) - k) / (w - k) * w;
    let t = bisected(0.0, SOUGHT, BRIGHTNESS_REACH_LIGHT, light);
    let reach = ended.toward_light(pick_log, ended.exposed(neutral(t)));
    Paper { brightness, pick_log, reach, ..ended }
}

/// `bare` at its `rest` (log time, filtration) with colour `density`, its `anchors` the pick, the densest read and the
/// reference grey as the bare film holds them: the pick printed where the rest prints it whatever Lighting moves.
pub(super) fn lit(bare: &Paper, rest: (f32, V3), anchors: &[V3; 3], density: f32, s: &PipelineSettings, negative: Option<&Negative>) -> Printing {
    let (time, rest_cc) = rest;
    let at_rest = Paper { scale: time.exp(), filters: filters(rest_cc), density, ..*bare };
    let [pick, _, grey] = *anchors;
    let lift = super::lifted(s).then(|| at_rest.printed(pick));
    // An enlarger's filters read the least at none: a rest under none sheds that neutral density, the time then holding
    // the grey's luma as the print shows it.
    let least = rest_cc.iter().copied().fold(f32::INFINITY, f32::min);
    let cc = rest_cc.map(|v| v - least);
    let held = if cc == rest_cc {
        at_rest
    } else {
        let shown = |p: &Paper| luma(ended(p, pick, grey, lift, 0.0).shown(grey)).max(1e-12).ln();
        let target = shown(&at_rest);
        let with = |e: f32| Paper { scale: at_rest.scale * e.exp(), filters: filters(cc), ..at_rest };
        with(root(0.0, |e| shown(&with(e)) - target))
    };
    let developed = |a: V3| negative.map_or(a, |n| n.redeveloped(a));
    Printing { paper: ended(&held, developed(pick), developed(grey), lift, s.print.brightness), filtration: cc }
}
