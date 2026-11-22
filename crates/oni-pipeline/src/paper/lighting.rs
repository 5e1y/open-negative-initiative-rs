//! Lighting on a paper, in density as an enlarger and a printer take it: the points, zones and mids on the negative's
//! density, Exposure the enlarger's time, Temperature and Tint its filtration, Contrast the paper's grade; the print's
//! pick then laid where the rest prints it, or on pure black, the paper's white held, Brightness between them.

use super::printing::{filters, luma, root};
use super::*;
use oni_render::Paper;
use oni_settings::tone::Print;
use oni_settings::PipelineSettings;

pub use oni_settings::tone::CC_PER_STOP;

/// Contrast's paper grade: the slope times this to its power, so the track's ends are a grade softer or harder.
pub const GRADE_BASE: f32 = 4.0;

/// `p` with its print's ends: the pick printed without Lighting's points, zones and mids, and the paper's own white,
/// both at its colour density; the pick laid on `anchor`, Brightness `brightness` stops between them.
pub fn ended(p: &Paper, pick: V3, anchor: V3, brightness: f32) -> Paper {
    let untoned = Paper { points_black: [0.0; 3], points_white: [0.0; 3], zones: [0.0; 2], mids: [0.0; 3], ..*p };
    // The paper's least density: unexposed on a negative paper, fully exposed on a reversal one.
    let white = untoned.viewed(if p.reversal { p.curves[PAPER_CURVE_NODES - 1] } else { p.curves[0] });
    let white = if p.luma { [luma(white); 3] } else { white };
    Paper { pick: p.coloured(untoned.printed(pick)), white: p.coloured(white), anchor, brightness, ..*p }
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
    let shown = |p: &Paper| luma(ended(p, pick, anchor, 0.0).shown(grey)).max(1e-12).ln();
    let (filtered, cc) = if cc == rest_cc {
        (toned, rest_cc)
    } else {
        let target = shown(&toned);
        let with = |e: f32| Paper { scale: toned.scale * e.exp(), filters: filters(cc), ..toned };
        let e = root(0.0, |e| shown(&with(e)) - target);
        (with(e), cc)
    };
    Printing { paper: ended(&filtered, pick, anchor, Print::EXPOSURE_GEAR * print.brightness), filtration: cc }
}
