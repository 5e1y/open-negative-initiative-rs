//! Each frame's enlarger on its paper: set so the grey its windows print as a plain RAW's prints there too, neutral,
//! frozen there, and the frame's three anchors printed through it to lay ONI's window on the print.

use super::*;
use crate::film::bake::{self, M3};
use oni_render::{DYE_LIGHT_FLOOR, DyeSeparation, Paper, Stage47, cpu};
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

fn filters(cc: V3) -> V3 {
    cc.map(|v| 10f32.powf(-v / CC_PER_DENSITY))
}

fn luma(v: V3) -> f32 {
    oni_render::LUMA.iter().zip(v).map(|(w, x)| w * x).sum()
}

fn ln_luma(v: V3) -> f32 {
    luma(v).max(1e-12).ln()
}

/// The scan densities, through the frame's gains, its windows print as a plain RAW's grey on ONI's print at rest:
/// where every window reads the one share that grey prints at.
pub fn reference_grey(s: &PipelineSettings) -> V3 {
    let rest = Print { exposure: Print::resting_exposure(s.mode), ..Print::default() };
    let shown = |t: f32| rest.output(t, LevelsChannel::Linked, true);
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
/// one runs: each window's black (the dark pick), its white, and the reference grey.
pub fn anchors(s: &PipelineSettings, separation: Option<&DyeSeparation>) -> [V3; 3] {
    let w = s.levels.windows();
    let grey = reference_grey(s);
    let above: [V3; 3] = [[0.0; 3], std::array::from_fn(|c| w[c].white - w[c].black), std::array::from_fn(|c| grey[c] - w[c].black)];
    above.map(|a| separation.map_or(a, |d| d.separated(a)))
}

/// `p` laid through the window its anchors print on it: each channel's black on 0, its white on 1, its gamma putting
/// the grey on a plain RAW's; a black and white print reads one window on its luma.
pub fn anchored(p: Paper, anchors: &[V3; 3], mono: bool) -> Paper {
    let read = |a: &V3| {
        let light = p.printed(*a);
        if mono { [luma(light); 3] } else { light }
    };
    let [black, white, grey] = [&anchors[0], &anchors[1], &anchors[2]].map(read);
    let gamma = std::array::from_fn(|c| GREY_PRINT.ln() / ((grey[c] - black[c]) / (white[c] - black[c])).ln());
    Paper { window_black: black, window_white: white, window_gamma: gamma, window_luma: mono, ..p }
}

/// The enlarger's light, from `scale` in its log, that holds the luma `read` prints the grey at on a plain RAW's.
fn luma_held(paper: &Paper, scale: f32, read: impl Fn(&Paper) -> V3) -> f32 {
    let target = ln_luma([*GREY_PRINT; 3]);
    let at = |x: f32| ln_luma(read(&Paper { scale: x.exp(), ..*paper }));
    let mut scale = scale;
    for _ in 0..ITERATIONS {
        let here = at(scale);
        if (here - target).abs() < TOLERANCE {
            break;
        }
        let slope = (at(scale + PROBE[0]) - here) / PROBE[0];
        if slope.is_nan() || slope.abs() <= 1e-6 {
            break;
        }
        scale -= ((here - target) / slope).clamp(-STEP_BOUND[0], STEP_BOUND[0]);
    }
    scale
}

/// The frame's paper, its enlarger set where the reference grey prints a plain RAW's grey, neutral, frozen there under
/// its anchors' window; Lighting's filters move its filtration, the windowed grey's luma held. None on ONI's print.
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
    let paper = Paper {
        gain,
        pedestal,
        log_guard,
        black: read_black,
        negative: &table.negative,
        exposure: &table.exposure,
        dichroic: &table.dichroic,
        filters: [1.0; 3],
        scale: 1.0,
        curves: &sheet.curves,
        curve_low: sheet.curve_low,
        curve_step: sheet.curve_step,
        print: &sheet.print,
        view: &sheet.view,
        window_black: [0.0; 3],
        window_white: [1.0; 3],
        window_gamma: [1.0; 3],
        window_luma: false,
    };
    let anchors = anchors(s, separation);
    let grey = anchors[2];
    let target = GREY_PRINT.ln();
    // The neutral filtration and exposure, cyan at none: Newton on each channel's log light.
    let at = |x: V3| Paper { scale: x[0].exp(), filters: filters([0.0, x[1], x[2]]), ..paper };
    let miss = |x: V3| at(x).printed(grey).map(|v| v.max(1e-12).ln() - target);
    let raw = paper.exposed(grey);
    let mut x = [-(raw.iter().sum::<f32>() / 3.0) * std::f32::consts::LN_10, START_CC[0], START_CC[1]];
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
    // Lighting's filters move the neutral one, the least filter at none.
    let neutral = [0.0, x[1], x[2]];
    let filtration = |moved: V3| {
        let cc: V3 = std::array::from_fn(|c| neutral[c] + CC_PER_STOP * moved[c]);
        let least = cc.iter().copied().fold(f32::INFINITY, f32::min);
        cc.map(|v| v - least)
    };
    // The paper at its rest, its filters at Lighting's rest: the window its anchors print there stays whatever moves.
    let rest_cc = filtration([0.0; 3]);
    let unwindowed = Paper { filters: filters(rest_cc), ..paper };
    let rest_scale = luma_held(&unwindowed, x[0], |p| p.printed(grey));
    let rest = anchored(Paper { scale: rest_scale.exp(), ..unwindowed }, &anchors, mono);
    let cc = filtration(s.print.filters());
    let moved = Paper { filters: filters(cc), ..rest };
    let scale = luma_held(&moved, rest_scale, |p| p.windowed(p.printed(grey)));
    Some(Printing { paper: Paper { scale: scale.exp(), ..moved }, filtration: cc })
}
