//! The frames the paper's checks print: a digital photo, a prepared negative, black and white, and their anchors' scans.
//! Each prints on Royal Digital, a data sheet's paper and its dyes' crosstalk, on the paper's own black (Print lift), so
//! a check reads the paper's print itself; pure black is `paper_ends`'. The same frames on ONI Standard, or on no paper.

#![allow(dead_code)]

use crate::common::V4;
use oni_pipeline::{conversion, paper};
use oni_render::cpu;
use oni_settings::{ConversionMode, DyeSet, PaperStock, PipelineSettings, VirtualStock};

/// The data-sheet paper the frames print on.
pub const SHEET_PAPER: PaperStock = PaperStock::of("kodak-royal-digital");

/// ONI Standard, the neutral paper.
pub const STANDARD: PaperStock = PaperStock::of("oni-standard");

/// `s` on ONI Standard.
pub fn onis(s: PipelineSettings) -> PipelineSettings {
    PipelineSettings { print_paper: Some(STANDARD), ..s }
}

/// `s` naming no paper: its pure inversion, or a legacy print's own curve.
pub fn unpapered(s: PipelineSettings) -> PipelineSettings {
    PipelineSettings { print_paper: None, ..s }
}

fn on_print_lift(mut s: PipelineSettings) -> PipelineSettings {
    s.print.print_lift = true;
    s
}

pub fn digital() -> PipelineSettings {
    on_print_lift(PipelineSettings { print_paper: Some(SHEET_PAPER), ..PipelineSettings::rest(ConversionMode::Digital, true).on_stock(VirtualStock::of("kodak-portra-800")) })
}

pub fn prepared_negative() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.gains.stops = [-0.8, 0.0, 1.2];
    let windows = [(0.55, 1.6, 0.45), (0.54, 1.75, 0.5), (0.52, 1.9, 0.58)];
    for (w, (black, white, mid)) in [&mut s.levels.red, &mut s.levels.green, &mut s.levels.blue].into_iter().zip(windows) {
        (w.black, w.white, w.mid) = (black, white, mid);
    }
    s.dye_set = DyeSet::of("kodak-portra-400");
    s.print_paper = Some(SHEET_PAPER);
    on_print_lift(s)
}

/// The scan pixel holding densities `d`, through the frame's gains and pedestal.
pub fn scan_at(s: &PipelineSettings, d: [f32; 3]) -> V4 {
    let k = conversion::stage47(s);
    let light: [f32; 3] = std::array::from_fn(|c| 10f32.powf(-d[c]) / k.gain[c] - k.pedestal);
    [light[0], light[1], light[2], 1.0]
}

/// The scan pixel holding the frame's reference grey.
pub fn grey_scan(s: &PipelineSettings) -> V4 {
    scan_at(s, paper::reference_grey(s))
}

/// The scan pixels of the frame's three anchors: its windows' black and white, and its reference grey.
pub fn anchor_scans(s: &PipelineSettings) -> [V4; 3] {
    let w = s.levels.windows();
    [scan_at(s, w.map(|l| l.black)), scan_at(s, w.map(|l| l.white)), grey_scan(s)]
}

pub fn mono() -> PipelineSettings {
    on_print_lift(PipelineSettings { print_paper: Some(SHEET_PAPER), ..PipelineSettings::rest(ConversionMode::Monochrome, true) })
}

/// `s` moved by `f`, its anchors printed.
pub fn lit(s: &PipelineSettings, f: impl Fn(&mut PipelineSettings)) -> [V4; 3] {
    let mut t = s.clone();
    f(&mut t);
    let k = conversion::stage47(&t);
    anchor_scans(s).map(|q| cpu::stage47(q, &k))
}
