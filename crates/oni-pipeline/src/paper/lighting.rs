//! Lighting on a paper, in density as an enlarger and a printer take it: the points, zones and mids on the negative's
//! density, Exposure the enlarger's time, Temperature and Tint its filtration, Contrast the paper's grade, Brightness
//! the enlarger's light above a knee over the pick; the print's pick then laid on pure black, or where the rest prints
//! it, the paper's white held.

use super::printing::{bisected, filters, luma, root};
use super::*;
use oni_render::Paper;
use oni_settings::tone::Print;
use oni_settings::PipelineSettings;

pub use oni_settings::tone::CC_PER_STOP;

/// Contrast's paper grade: the slope times this to its power, so the track's ends are a grade softer or harder.
pub const GRADE_BASE: f32 = 4.0;

/// Brightness's knee: the light the print holds there, its pick on black and its white the paper's, L* 5; under it
/// the blacks and the first shadows, 8 to 17 % of a well-exposed frame's pixels on roll #8.
pub const BRIGHTNESS_KNEE_LIGHT: f32 = 0.0055;

/// How far past the grey, in its own amounts, the knee is sought: a print that never reaches it takes the end.
const KNEE_REACH: f32 = 8.0;

/// `p` with its ends, the pick printed untoned laid on `anchor`, the paper's white held, and Brightness's stops above
/// its knee: where the neutral from the pick to the `grey` prints `BRIGHTNESS_KNEE_LIGHT` over the pick.
pub fn ended(p: &Paper, pick: V3, grey: V3, anchor: V3, brightness: f32) -> Paper {
    let untoned = Paper { points_black: [0.0; 3], points_white: [0.0; 3], zones: [0.0; 2], mids: [0.0; 3], brightness: 0.0, ..*p };
    // The paper's least density: unexposed on a negative paper, fully exposed on a reversal one.
    let white = untoned.viewed(if p.reversal { p.curves[PAPER_CURVE_NODES - 1] } else { p.curves[0] });
    let white = if p.luma { [luma(white); 3] } else { white };
    let mean = |v: V3| (v[0] + v[1] + v[2]) / 3.0;
    let pick_log = mean(untoned.exposed(pick));
    let ended = Paper { pick: p.coloured(untoned.printed(pick)), white: p.coloured(white), anchor, brightness: 0.0, ..*p };
    let (k, w) = (luma(ended.pick), luma(ended.white));
    let neutral = |t: f32| std::array::from_fn(|c| pick[c] + t * (grey[c] - pick[c]));
    let light = |t: f32| (luma(ended.coloured(ended.printed(neutral(t)))) - k) / (w - k) * w;
    let t = bisected(0.0, KNEE_REACH, BRIGHTNESS_KNEE_LIGHT, light);
    let knee = ended.toward_light(pick_log, ended.exposed(ended.toned(neutral(t))));
    Paper { brightness, pick_log, knee, ..ended }
}

/// `bare` at its `rest` (log time, filtration) under the frame's Lighting and colour `density`, its `anchors` the
/// pick, the densest read and the reference grey: the pick prints where the rest prints it whatever Lighting moves.
pub(super) fn lit(bare: &Paper, rest: (f32, V3), anchors: &[V3; 3], density: f32, s: &PipelineSettings) -> Printing {
    let print = &s.print;
    let mono = bare.luma;
    let (time, rest_cc) = rest;
    let at_rest = Paper { scale: time.exp(), filters: filters(rest_cc), ..*bare };
    let [pick, _, grey] = *anchors;
    let anchor = if print.paper_blacks { at_rest.printed(pick) } else { [0.0; 3] };
    let points = print.channel_points(mono);
    let own = |v: V3| if mono { [0.0; 3] } else { v };
    let channel_contrast = own(print.channel_contrast);
    // A stop of Exposure is a stop brighter from the mode's rest, ungeared: less light on a negative paper, more on a
    // reversal one; a filter's stop likewise, its CC taken away on a reversal paper.
    let sense = if bare.reversal { -1.0 } else { 1.0 };
    let stops = sense * (print.exposure - Print::resting_exposure(s.mode));
    let toned = Paper {
        points_black: points.map(|p| p.0),
        points_white: points.map(|p| p.1),
        zones: [print.shadows, print.highlights],
        mids: own(print.channel_mid),
        pivot: at_rest.exposed(grey),
        slope: std::array::from_fn(|c| GRADE_BASE.powf(print.contrast + channel_contrast[c])),
        scale: at_rest.scale * (-stops).exp2(),
        density,
        ..at_rest
    };
    let moved = own(print.filters());
    // Each filter moved from the rest, the least at none, a rest under none included; the time then holds the grey's
    // luma as the print shows it, its pick on its anchor.
    let cc: V3 = std::array::from_fn(|c| rest_cc[c] + sense * CC_PER_STOP * moved[c]);
    let least = cc.iter().copied().fold(f32::INFINITY, f32::min);
    let cc = cc.map(|v| v - least);
    let shown = |p: &Paper| luma(ended(p, pick, grey, anchor, 0.0).shown(grey)).max(1e-12).ln();
    let (filtered, cc) = if cc == rest_cc {
        (toned, rest_cc)
    } else {
        let target = shown(&toned);
        let with = |e: f32| Paper { scale: toned.scale * e.exp(), filters: filters(cc), ..toned };
        let e = root(0.0, |e| shown(&with(e)) - target);
        (with(e), cc)
    };
    Printing { paper: ended(&filtered, pick, grey, anchor, print.brightness), filtration: cc }
}
