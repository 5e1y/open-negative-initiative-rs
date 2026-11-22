//! Each frame's enlarger on its paper at rest: the time and filtration that print the reference grey at a plain RAW's
//! grey, neutral, on the bare print, its ends the paper's own; Lighting then moves it (`lighting`).

use super::*;
use crate::film::bake::{self, M3};
use oni_render::{DYE_LIGHT_FLOOR, DyeSeparation, PAPER_CURVE_NODES, Paper, Stage47, cpu};
use oni_settings::levels::LevelsChannel;
use oni_settings::tone::Print;
use oni_settings::PipelineSettings;
use std::sync::LazyLock;

/// The neutral filtration's magenta and yellow at its start, its cyan at none as a colour enlarger's rests.
const START_CC: [f32; 2] = [50.0, 50.0];

/// The solve's probes, in log exposure and CC, its largest steps, its iterations and its tolerance in log light.
const PROBE: V3 = [0.01, 0.5, 0.5];
const STEP_BOUND: V3 = [1.0, 20.0, 20.0];
const ITERATIONS: usize = 40;
const TOLERANCE: f32 = 1e-6;

/// A frame's paper and the enlarger's filtration it prints through, in CC: what the paper's block shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Printing {
    pub paper: Paper,
    pub filtration: V3,
}

/// What a plain RAW's render at rest prints a scene grey at: where the paper prints the frame's reference grey.
pub static GREY_PRINT: LazyLock<f32> = LazyLock::new(crate::film::plain_grey);

pub(super) fn filters(cc: V3) -> V3 {
    cc.map(|v| 10f32.powf(-v / CC_PER_DENSITY))
}

pub(super) fn luma(v: V3) -> f32 {
    oni_render::LUMA.iter().zip(v).map(|(w, x)| w * x).sum()
}

/// The scan densities, through the frame's gains, its windows print as a plain RAW's grey on ONI's print at rest:
/// where every window reads the one share that grey prints at.
pub fn reference_grey(s: &PipelineSettings) -> V3 {
    let rest = Print { exposure: Print::resting_exposure(s.mode), ..Print::default() };
    let shown = |t: f32| rest.output(t, LevelsChannel::Linked);
    let share = bisected(0.0, 2.0, *GREY_PRINT, shown);
    let k = windows_alone(s);
    std::array::from_fn(|c| {
        let w = s.levels.windows()[c];
        let (low, high) = (w.black.min(w.white) - 1.0, w.black.max(w.white) + 4.0);
        bisected(low, high, share, |d| cpu::stage47([10f32.powf(-d); 4], &k)[c])
    })
}

/// The frame's windows alone, read on density at unit gain: stage 5's output with no print or decompression.
fn windows_alone(s: &PipelineSettings) -> Stage47 {
    let w = s.levels.windows();
    let vector = |c: usize| [w[c].black, w[c].white, w[c].gamma()];
    Stage47 {
        gain: [1.0; 3],
        pedestal: 0.0,
        log_guard: 1e-30,
        red: vector(0),
        green: vector(1),
        blue: vector(2),
        low_points: w.map(|l| l.shadow_ordinate()),
        high_points: w.map(|l| l.highlight_ordinate()),
        ..Stage47::rest()
    }
}

/// The `x` between `low` and `high` where `f` reads `target`, `f` rising or falling along it.
fn bisected(mut low: f32, mut high: f32, target: f32, f: impl Fn(f32) -> f32) -> f32 {
    let rising = f(high) >= f(low);
    for _ in 0..60 {
        let mid = 0.5 * (low + high);
        if (f(mid) < target) == rising { low = mid } else { high = mid }
    }
    0.5 * (low + high)
}

/// The frame's three anchors as the paper reads them, dye amounts above the bare film through the separation where
/// one runs: each window's black (the dark pick, the film's base), its white (the densest it reads), and the reference grey.
pub fn anchors(s: &PipelineSettings, separation: Option<&DyeSeparation>) -> [V3; 3] {
    let w = s.levels.windows();
    let grey = reference_grey(s);
    let above: [V3; 3] = [[0.0; 3], std::array::from_fn(|c| w[c].white - w[c].black), std::array::from_fn(|c| grey[c] - w[c].black)];
    above.map(|a| separation.map_or(a, |d| d.separated(a)))
}

/// The `x` from `start` where `f` crosses zero, by Newton's steps in the enlarger's log exposure.
pub(super) fn root(start: f32, f: impl Fn(f32) -> f32) -> f32 {
    let mut x = start;
    for _ in 0..ITERATIONS {
        let here = f(x);
        if here.abs() < TOLERANCE {
            break;
        }
        let slope = (f(x + PROBE[0]) - here) / PROBE[0];
        if slope.is_nan() || slope.abs() <= 1e-6 {
            break;
        }
        x -= (here / slope).clamp(-STEP_BOUND[0], STEP_BOUND[0]);
    }
    x
}

/// The exposure and the magenta and yellow filtration, from `x`, where every channel's `miss` is zero.
fn neutral_on(miss: impl Fn(V3) -> V3, mut x: V3) -> V3 {
    for _ in 0..ITERATIONS {
        let r = miss(x);
        if r.iter().all(|v| v.abs() < TOLERANCE) {
            break;
        }
        let columns: [V3; 3] = std::array::from_fn(|j| {
            let mut moved = x;
            moved[j] += PROBE[j];
            let m = miss(moved);
            std::array::from_fn(|c| (m[c] - r[c]) / PROBE[j])
        });
        let jacobian: M3 = std::array::from_fn(|c| std::array::from_fn(|j| f64::from(columns[j][c])));
        let step = bake::apply(&bake::inverse(&jacobian), r.map(f64::from));
        if !step.iter().all(|v| v.is_finite()) {
            break;
        }
        x = std::array::from_fn(|j| x[j] - (step[j] as f32).clamp(-STEP_BOUND[j], STEP_BOUND[j]));
    }
    x
}

/// Each layer's density under a negative holding `amounts`, as a share of its curve's range.
pub fn layer_shares(p: &Paper, amounts: V3) -> V3 {
    let (low, high) = (p.curves[0], p.curves[PAPER_CURVE_NODES - 1]);
    let density = p.developed(p.graded(p.exposed(p.toned(amounts))));
    std::array::from_fn(|c| (density[c] - low[c]) / (high[c] - low[c]))
}

/// The enlarger at rest on the bare paper `p`, as its log time and its magenta and yellow (cyan at none): the reference
/// `grey` printed at a plain RAW's grey, neutral; black and white by the time alone, on its luma.
pub fn resting(p: &Paper, grey: V3, mono: bool) -> V3 {
    let at = |x: V3| Paper { scale: x[0].exp(), filters: filters([0.0, x[1], x[2]]), ..*p };
    let target = GREY_PRINT.ln();
    let miss = |x: V3| at(x).printed(grey).map(|v| v.max(1e-12).ln() - target);
    let start = -(p.exposed(grey).iter().sum::<f32>() / 3.0) * std::f32::consts::LN_10;
    if mono {
        return [root(start, |e| miss([e, 0.0, 0.0])[0]), 0.0, 0.0];
    }
    neutral_on(miss, [start, START_CC[0], START_CC[1]])
}

/// A dye's range on the negative, never empty: a window's span holds at least 1e-4 of density, as ONI's print reads it.
fn spanned(v: f32) -> f32 {
    if v.abs() < 1e-4 { 1e-4f32.copysign(v) } else { v }
}

/// The frame's paper, its enlarger at rest, then Lighting in density on the negative and the enlarger, the
/// grade on the paper, the colour density and the ends on the print. None on ONI's print.
pub fn printing(s: &PipelineSettings, k: &Stage47, separation: Option<&DyeSeparation>) -> Option<Printing> {
    let stock = s.paper().filter(|_| !s.print.bypassed)?;
    let mono = s.mode.is_monochrome();
    let table = enlarger((!mono).then_some(s.dye_set), stock);
    let sheet = sheet(stock);
    let black = s.levels.windows().map(|w| w.black);
    let (gain, pedestal, log_guard, read_black) = match separation {
        Some(_) => ([1.0; 3], 0.0, DYE_LIGHT_FLOOR, [0.0; 3]),
        None => (k.gain, k.pedestal, k.log_guard, black),
    };
    let anchors = anchors(s, separation);
    let bare = Paper {
        gain,
        pedestal,
        log_guard,
        black: read_black,
        span: anchors[1].map(spanned),
        points_black: [0.0; 3],
        points_white: [0.0; 3],
        zones: [0.0; 2],
        mids: [0.0; 3],
        negative: &table.negative,
        exposure: &table.exposure,
        dichroic: &table.dichroic,
        filters: [1.0; 3],
        scale: 1.0,
        pivot: [0.0; 3],
        slope: [1.0; 3],
        curves: &sheet.curves,
        curve_low: sheet.curve_low,
        curve_step: sheet.curve_step,
        print: &sheet.print,
        view: &sheet.view,
        density: 1.0,
        pick: [0.0; 3],
        white: [1.0; 3],
        anchor: [0.0; 3],
        brightness: 0.0,
        luma: mono,
    };
    let rest = resting(&bare, anchors[2], mono);
    Some(super::lighting::lit(&bare, rest, &anchors, k.density, s))
}
