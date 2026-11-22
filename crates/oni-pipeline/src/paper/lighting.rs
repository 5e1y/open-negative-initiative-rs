//! Lighting on a paper, in density as an enlarger and a printer take it: the points, zones and mids on the negative's
//! density, Exposure the enlarger's time, Temperature and Tint its filtration, Contrast the paper's grade; Brightness and
//! True blacks on the print, between the paper's own Dmax and white.

use super::printing::{filters, luma, root};
use super::*;
use oni_render::{PAPER_CURVE_NODES, Paper};
use oni_settings::tone::Print;
use oni_settings::PipelineSettings;

pub use oni_settings::tone::CC_PER_STOP;

/// Contrast's paper grade: the slope times this to its power, so the track's ends are a grade softer or harder.
pub const GRADE_BASE: f32 = 4.0;

/// `bare` at its `rest` (log time, magenta, yellow) under the frame's Lighting, its `anchors` the base, the densest read and
/// the reference grey: the paper's ends move with every slider, as on an enlarger.
pub(super) fn lit(bare: &Paper, rest: V3, anchors: &[V3; 3], s: &PipelineSettings) -> Printing {
    let print = &s.print;
    let mono = bare.luma;
    let rest_cc = [0.0, rest[1], rest[2]];
    let at_rest = Paper { scale: rest[0].exp(), filters: filters(rest_cc), ..*bare };
    let grey = anchors[2];
    let points = print.channel_points(mono);
    let own = |v: V3| if mono { [0.0; 3] } else { v };
    let channel_contrast = own(print.channel_contrast);
    // A stop of Exposure is a stop less light on the paper, from the mode's rest, ungeared.
    let stops = print.exposure - Print::resting_exposure(s.mode);
    let toned = Paper {
        points_black: points.map(|p| p.0),
        points_white: points.map(|p| p.1),
        zones: [print.shadows, print.highlights],
        mids: own(print.channel_mid),
        pivot: at_rest.exposed(grey),
        slope: std::array::from_fn(|c| GRADE_BASE.powf(print.contrast + channel_contrast[c])),
        scale: at_rest.scale * (-stops).exp2(),
        ..at_rest
    };
    let moved = own(print.filters());
    // Each filter moved from the rest, the least at none, a rest under none included; the time then holds the grey's luma.
    let cc: V3 = std::array::from_fn(|c| rest_cc[c] + CC_PER_STOP * moved[c]);
    let least = cc.iter().copied().fold(f32::INFINITY, f32::min);
    let cc = cc.map(|v| v - least);
    let (filtered, cc) = if cc == rest_cc {
        (toned, rest_cc)
    } else {
        let target = luma(toned.printed(grey)).max(1e-12).ln();
        let with = |e: f32| Paper { scale: toned.scale * e.exp(), filters: filters(cc), ..toned };
        let e = root(0.0, |e| luma(with(e).printed(grey)).max(1e-12).ln() - target);
        (with(e), cc)
    };
    let shown = |density: V3| {
        let light = filtered.viewed(density);
        if mono { [luma(light); 3] } else { light }
    };
    let paper = Paper {
        dmax: shown(filtered.curves[PAPER_CURVE_NODES - 1]),
        white: shown(filtered.curves[0]),
        brightness: Print::EXPOSURE_GEAR * print.brightness,
        true_blacks: print.true_blacks,
        ..filtered
    };
    Printing { paper, filtration: cc }
}
