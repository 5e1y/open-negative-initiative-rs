//! The frames, Lighting's sliders and the readings the checks of a paper's ends share.

#![allow(dead_code)]

use crate::common::V4;
use crate::frames::*;
use oni_pipeline::{conversion, paper};
use oni_render::{PAPER_CURVE_NODES, Paper, Stage47, cpu};
use oni_settings::tone::Print;
use oni_settings::{PaperStock, PipelineSettings};

pub fn frames() -> [(&'static str, PipelineSettings); 4] {
    let frontier = PipelineSettings { print_paper: Some(PaperStock::of("fujifilm-frontier-sp-3000")), ..prepared_negative() };
    [("negative", prepared_negative()), ("digital", digital()), ("black and white", mono()), ("Frontier negative", frontier)]
}

pub type Move = (&'static str, fn(&mut PipelineSettings));

/// Every Lighting slider but the points, each at a visible travel either way.
pub const SLIDERS: [Move; 22] = [
    ("Exposure -1", |s| s.print.exposure -= 1.0),
    ("Exposure +1", |s| s.print.exposure += 1.0),
    ("Contrast -0.3", |s| s.print.contrast = -0.3),
    ("Contrast +0.3", |s| s.print.contrast = 0.3),
    ("Temperature -1", |s| s.print.set_filters(Print::TEMPERATURE_AXIS.map(|a| -a))),
    ("Temperature +1", |s| s.print.set_filters(Print::TEMPERATURE_AXIS)),
    ("Tint -1", |s| s.print.set_filters(Print::TINT_AXIS.map(|a| -a))),
    ("Tint +1", |s| s.print.set_filters(Print::TINT_AXIS)),
    ("red filter +0.3", |s| s.print.set_filters([0.3, 0.0, 0.0])),
    ("Shadows -0.2", |s| s.print.shadows = -0.2),
    ("Shadows +0.2", |s| s.print.shadows = 0.2),
    ("Highlights -0.2", |s| s.print.highlights = -0.2),
    ("Highlights +0.2", |s| s.print.highlights = 0.2),
    ("channel contrast -0.2", |s| s.print.channel_contrast = [-0.2, 0.0, 0.2]),
    ("channel contrast +0.2", |s| s.print.channel_contrast = [0.2, 0.0, -0.2]),
    ("channel mids", |s| s.print.channel_mid = [0.2, -0.2, 0.0]),
    ("Saturation -0.5", |s| s.density = -0.5),
    ("Saturation +0.5", |s| s.density = 0.5),
    ("Brightness -1", |s| s.print.brightness = -1.0),
    ("Brightness +1", |s| s.print.brightness = 1.0),
    ("Exposure +1, Contrast +0.3, Temperature +1", |s| {
        (s.print.exposure, s.print.contrast) = (s.print.exposure + 1.0, 0.3);
        s.print.set_filters(Print::TEMPERATURE_AXIS);
    }),
    ("Exposure -1, Saturation +0.5, Brightness +1", |s| (s.print.exposure, s.density, s.print.brightness) = (s.print.exposure - 1.0, 0.5, 1.0)),
];

pub fn moved(s: &PipelineSettings, f: impl Fn(&mut PipelineSettings)) -> PipelineSettings {
    let mut t = s.clone();
    f(&mut t);
    t
}

pub fn paper_of(s: &PipelineSettings) -> Paper {
    conversion::stage47(s).paper.expect("a paper")
}

/// The paper's print of `amounts` through Lighting on the negative, its tones included; the negative's range on each
/// dye, the frame's densest read above its bare film.
pub fn lit_shown(k: &Stage47, amounts: [f32; 3]) -> [f32; 3] {
    k.paper.expect("a paper").shown(k.negative.map_or(amounts, |n| n.toned(n.redeveloped(amounts))))
}
pub fn span_of(s: &PipelineSettings) -> [f32; 3] {
    paper::anchors(s, conversion::stage47(s).dyes.as_ref())[1]
}

/// The frame's anchors printed on `k`: its pick, its densest read and its reference grey.
pub fn printed_on(s: &PipelineSettings, k: &Stage47) -> [[f32; 3]; 3] {
    anchor_scans(s).map(|q| rgb(cpu::stage47(q, k)))
}

/// The same print with no ends: the paper's light at its colour density, as before the pick was anchored.
pub fn unanchored(k: Stage47) -> Stage47 {
    Stage47 { paper: k.paper.map(|p| Paper { anchor: p.pick, brightness: 0.0, ..p }), ..k }
}

/// The paper's own Dmax as its print shows it.
pub fn dmax(p: &Paper) -> [f32; 3] {
    let v = p.viewed(p.curves[PAPER_CURVE_NODES - 1]);
    if p.luma { [0.2627 * v[0] + 0.678 * v[1] + 0.0593 * v[2]; 3] } else { v }
}

pub fn rgb(v: V4) -> [f32; 3] {
    [v[0], v[1], v[2]]
}

pub fn worst(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f32::max)
}
