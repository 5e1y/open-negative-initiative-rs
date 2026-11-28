//! The frames, Lighting's sliders and the readings the checks of a paper's ends share.

#![allow(dead_code)]

use crate::common::V4;
use crate::frames::*;
use oni_pipeline::{conversion, paper};
use oni_render::{PAPER_CURVE_NODES, Paper, Shown, Stage47, cpu};
use oni_settings::tone::Print;
use oni_settings::{PaperStock, PipelineSettings};

pub fn frames() -> [(&'static str, PipelineSettings); 5] {
    let frontier = PipelineSettings { print_paper: Some(PaperStock::of("fujifilm-frontier-sp-3000")), ..prepared_negative() };
    let endura = PipelineSettings { print_paper: Some(PaperStock::of("kodak-endura-premier")), ..prepared_negative() };
    [("negative", prepared_negative()), ("digital", digital()), ("black and white", mono()), ("Frontier negative", frontier), ("Endura Premier negative", endura)]
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
    conversion::stage47(s).paper().expect("a paper")
}

/// The print on `k` of the pixel holding `above` over each channel's bare film, through Lighting and the window; the
/// negative's range on each channel, the frame's densest read above its bare film.
pub fn lit_shown(s: &PipelineSettings, k: &Stage47, above: [f32; 3]) -> [f32; 3] {
    let bare = paper::film_ends(s).0;
    rgb(cpu::stage47(scan_at(s, std::array::from_fn(|c| bare[c] + above[c])), k))
}
pub fn span_of(s: &PipelineSettings) -> [f32; 3] {
    paper::anchors(s)[1]
}

/// The window's shares of the frame's pick, densest read and reference grey as Lighting in `k` develops them.
pub fn anchor_shares(s: &PipelineSettings, k: &Stage47) -> [[f32; 3]; 3] {
    paper::anchors(s).map(|a| paper::shares_of(s, k, k.negative.map_or(a, |n| n.redeveloped(a))))
}

/// The frame's anchors printed on `k`: its pick, its densest read and its reference grey.
pub fn printed_on(s: &PipelineSettings, k: &Stage47) -> [[f32; 3]; 3] {
    anchor_scans(s).map(|q| rgb(cpu::stage47(q, k)))
}

/// The same print with no ends: the paper's light at its colour density, as before the pick was anchored.
pub fn unanchored(k: Stage47) -> Stage47 {
    Stage47 { shown: k.paper().map_or(k.shown, |p| Shown::Paper(Paper { anchor: p.pick, brightness: 0.0, ..p })), ..k }
}

/// The paper's own Dmax as its print shows it.
pub fn dmax(p: &Paper) -> [f32; 3] {
    let v = p.viewed(p.dyed(p.curves[PAPER_CURVE_NODES - 1]));
    if p.luma { [0.2627 * v[0] + 0.678 * v[1] + 0.0593 * v[2]; 3] } else { v }
}

pub fn rgb(v: V4) -> [f32; 3] {
    [v[0], v[1], v[2]]
}

pub fn worst(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f32::max)
}
