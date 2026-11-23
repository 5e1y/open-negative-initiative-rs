//! Each frame's enlarger on its paper at rest: the paper's own filtration, solved once on its layers where they print a
//! grey neutral on its white, and one time that shows the frame's reference grey at a plain RAW's grey through the
//! print's ends, as the window has laid the three channels' greys on one density. Lighting then moves the negative it
//! prints (`lighting`).

use super::*;
use crate::film::bake::{self, M3};
use oni_render::{PAPER_CURVE_NODES, Paper, Stage47, cpu};
use oni_settings::axis::pipeline::DMAX;
use oni_settings::levels::LevelsChannel;
use oni_settings::tone::Print;
use std::sync::LazyLock;

/// The neutral solve's probe and largest step in log exposure, its iterations, its tolerance in log light, and the
/// halvings a step may take before the solve stops where it stands.
const PROBE: f32 = 0.01;
const STEP_BOUND: f32 = 1.0;
const ITERATIONS: usize = 40;
const TOLERANCE: f32 = 1e-6;
const HALVINGS: i32 = 30;

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

/// The share of every window that prints a plain RAW's grey on ONI's print at rest in `s`'s mode.
pub fn grey_share(s: &PipelineSettings) -> f32 {
    let rest = Print { exposure: crate::conversion::resting_print_stops(s.mode), ..Print::default() };
    bisected(0.0, 2.0, *GREY_PRINT, |t| rest.output(t, LevelsChannel::Linked))
}

/// The scan densities, through the frame's gains, its windows print as a plain RAW's grey on ONI's print at rest:
/// where every window reads the one share that grey prints at.
pub fn reference_grey(s: &PipelineSettings) -> V3 {
    let share = grey_share(s);
    let k = windows_alone(s);
    let (bare, densest) = film_ends(s);
    std::array::from_fn(|c| {
        let (low, high) = (bare[c].min(densest[c]) - 1.0, bare[c].max(densest[c]) + 4.0);
        bisected(low, high, share, |d| cpu::stage47([10f32.powf(-d); 4], &k)[c])
    })
}

/// Each window's ends as scan densities: where its film is bare, and the densest it reads. A slide's windows read the
/// axis' top less the density, so its white is its bare film.
pub fn film_ends(s: &PipelineSettings) -> (V3, V3) {
    let w = s.levels.windows();
    if s.mode.reads_positive() {
        (w.map(|l| DMAX - l.white), w.map(|l| DMAX - l.black))
    } else {
        (w.map(|l| l.black), w.map(|l| l.white))
    }
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
        invert: s.mode.invert_flag(),
        ..Stage47::rest()
    }
}

/// The `x` between `low` and `high` where `f` reads `target`, `f` rising or falling along it.
pub(crate) fn bisected(mut low: f32, mut high: f32, target: f32, f: impl Fn(f32) -> f32) -> f32 {
    let rising = f(high) >= f(low);
    for _ in 0..60 {
        let mid = 0.5 * (low + high);
        if (f(mid) < target) == rising { low = mid } else { high = mid }
    }
    0.5 * (low + high)
}

/// The frame's three anchors as densities above each channel's bare film: the pick its print lays black (the bare film),
/// the densest read, the reference grey.
pub fn anchors(s: &PipelineSettings) -> [V3; 3] {
    let (bare, densest) = film_ends(s);
    let grey = reference_grey(s);
    [[0.0; 3], std::array::from_fn(|c| densest[c] - bare[c]), std::array::from_fn(|c| grey[c] - bare[c])]
}

/// The window's shares of a negative's pixel holding `above` over each channel's bare film, as stage 5 reads it in `k`.
pub fn shares_of(s: &PipelineSettings, k: &Stage47, above: V3) -> V3 {
    let bare = film_ends(s).0;
    let read = Stage47 { gain: [1.0; 3], pedestal: 0.0, negative: None, paper: None, ..*k };
    let light: V3 = std::array::from_fn(|c| 10f32.powf(-(bare[c] + above[c])));
    cpu::windowed([light[0], light[1], light[2], 1.0], &read)
}

/// How a share of the frame's window lies back on density, from its `anchors`: the windows' mean span, and the power
/// that lays the mean window's grey on the mean grey's own density, so the print reads the negative's density at one
/// grade on the mean.
pub fn laid_back(s: &PipelineSettings, anchors: &[V3; 3]) -> (f32, f32) {
    let mean = |v: V3| v.iter().map(|d| d.abs()).sum::<f32>() / 3.0;
    let (span, at_grey) = (spanned(mean(anchors[1])), mean(anchors[2]));
    let bend = (at_grey / span).ln() / grey_share(s).ln();
    (span, if bend.is_finite() && bend > 0.0 { bend } else { 1.0 })
}

/// The `x` from `start` where `f` crosses zero, by Newton's steps in the enlarger's log exposure.
fn root(start: f32, f: impl Fn(f32) -> f32) -> f32 {
    let mut x = start;
    for _ in 0..ITERATIONS {
        let here = f(x);
        if here.abs() < TOLERANCE {
            break;
        }
        let slope = (f(x + PROBE) - here) / PROBE;
        if slope.is_nan() || slope.abs() <= 1e-6 {
            break;
        }
        let step = (here / slope).clamp(-STEP_BOUND, STEP_BOUND);
        match damped(here.abs(), |t| f(x - t * step).abs()) {
            Some(t) => x -= t * step,
            None => break,
        }
    }
    x
}

/// The share of a Newton step that lowers the miss from `now`, halving it from the whole: a steep paper's step past
/// its root lands where its curve lies flat, from which no step returns.
fn damped(now: f32, miss_at: impl Fn(f32) -> f32) -> Option<f32> {
    (0..HALVINGS).map(|i| 0.5f32.powi(i)).find(|t| miss_at(*t) < now)
}

/// The three `x`, from `x`, where every channel's `miss` is zero.
fn neutral_on(miss: impl Fn(V3) -> V3, mut x: V3) -> V3 {
    for _ in 0..ITERATIONS {
        let r = miss(x);
        if r.iter().all(|v| v.abs() < TOLERANCE) {
            break;
        }
        let columns: [V3; 3] = std::array::from_fn(|j| {
            let mut moved = x;
            moved[j] += PROBE;
            let m = miss(moved);
            std::array::from_fn(|c| (m[c] - r[c]) / PROBE)
        });
        let jacobian: M3 = std::array::from_fn(|c| std::array::from_fn(|j| f64::from(columns[j][c])));
        let step = bake::apply(&bake::inverse(&jacobian), r.map(f64::from));
        if !step.iter().all(|v| v.is_finite()) {
            break;
        }
        // Shortened whole within its bound, so it keeps the direction that lowers the miss.
        let within = (0..3).map(|j| STEP_BOUND / (step[j] as f32).abs()).fold(1.0f32, f32::min);
        let step: V3 = std::array::from_fn(|j| step[j] as f32 * within);
        let moved = |t: f32| -> V3 { std::array::from_fn(|j| x[j] - t * step[j]) };
        let squared = |v: V3| v.iter().map(|c| c * c).sum::<f32>();
        match damped(squared(r), |t| squared(miss(moved(t)))) {
            Some(t) => x = moved(t),
            None => break,
        }
    }
    x
}

/// Each layer's log exposure where the paper prints a grey neutral on its own white, a plain RAW's grey's share of its
/// white, or with `lift` a plain RAW's grey's light: the point its filtration and the frame's time bring the reference
/// grey to, as the print's ends then show it.
pub fn own_grey(sheet: &Sheet, lift: bool) -> V3 {
    let on = |c: usize, log: f32| {
        let f = ((log - sheet.curve_low) / sheet.curve_step).clamp(0.0, (PAPER_CURVE_NODES - 1) as f32);
        let i = f.floor().min((PAPER_CURVE_NODES - 2) as f32);
        let at = i as usize;
        sheet.curves[at][c] + (f - i) * (sheet.curves[at + 1][c] - sheet.curves[at][c])
    };
    let viewed = oni_render::viewed;
    let white = viewed(sheet.curves[0]);
    let target = if lift { (*GREY_PRINT / luma(white)).ln() } else { GREY_PRINT.ln() };
    let miss = |log: V3| -> V3 {
        let v = viewed(std::array::from_fn(|c| on(c, log[c])));
        std::array::from_fn(|c| (v[c] / white[c]).max(1e-12).ln() - target)
    };
    // From one exposure on the three, where the print's luma stands at the grey's share of its white.
    let high = sheet.curve_low + sheet.curve_step * (PAPER_CURVE_NODES - 1) as f32;
    let start = bisected(sheet.curve_low, high, target, |e| (luma(viewed([on(0, e), on(1, e), on(2, e)])) / luma(white)).max(1e-12).ln());
    neutral_on(miss, [start; 3])
}

/// The filtration in CC, its least at none, that brings one neutral density to each layer's log exposure `grey`.
pub fn filtration(grey: V3) -> V3 {
    let most = grey.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    grey.map(|e| (most - e) * CC_PER_DENSITY)
}

/// Each layer's density under the window's `shares`, as a share of its curve's range.
pub fn layer_shares(p: &Paper, shares: V3) -> V3 {
    let (low, high) = (p.curves[0], p.curves[PAPER_CURVE_NODES - 1]);
    let density = p.developed(p.exposed(p.density(shares)));
    std::array::from_fn(|c| (density[c] - low[c]) / (high[c] - low[c]))
}

/// Whether the print keeps the paper's own ends, Print lift on a paper: the neutral paper prints ONI's print's, pure.
pub fn lifted(s: &PipelineSettings) -> bool {
    s.print.print_lift && !s.paperless
}

/// A dye's range on the negative, never empty: a window's span holds at least 1e-4 of density, as ONI's print reads it.
pub(crate) fn spanned(v: f32) -> f32 {
    if v.abs() < 1e-4 { 1e-4f32.copysign(v) } else { v }
}

/// The frame's paper, or the neutral one with Print on paper off, at the window's shares: its own filtration, its time
/// at rest showing the reference grey at a plain RAW's grey, then Brightness and the ends on the print of the negative as
/// Lighting develops it. None on ONI's print.
pub fn printing(s: &PipelineSettings, k: &Stage47) -> Option<Printing> {
    let stock = s.paper().filter(|_| !s.print.bypassed)?;
    let sheet = sheet_of(s, stock);
    let above = anchors(s);
    let (span, bend) = laid_back(s, &above);
    // The neutral paper's axis spans the frame's mean range, as a window spans its own: one range on the three layers.
    let axis = if s.paperless { span } else { 1.0 };
    let lift = lifted(s);
    let grey = if lift { sheet.lifted } else { sheet.grey }.map(|e| e * axis);
    let cc = filtration(grey);
    let bare = Paper {
        span,
        bend,
        mix: sheet.mix,
        filters: filters(cc),
        scale: 1.0,
        curves: &sheet.curves,
        curve_low: sheet.curve_low * axis,
        curve_step: sheet.curve_step * axis,
        pick: [0.0; 3],
        white: [1.0; 3],
        anchor: [0.0; 3],
        white_anchor: [1.0; 3],
        brightness: 0.0,
        pick_log: 0.0,
        reach: 0.0,
        luma: s.is_monochrome(),
    };
    // From the reference grey's density laid on the paper's own grey, the time that shows it at a plain RAW's grey, the
    // ends as Print lift lays them: on the paper's own white the grey's share of it is the light a plain RAW's grey holds.
    let [pick, _, at_grey] = above.map(|a| shares_of(s, k, a));
    let most = grey.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let start = (most + bare.density(at_grey).iter().sum::<f32>() / 3.0) * std::f32::consts::LN_10;
    let shown = |e: f32| {
        let q = Paper { scale: e.exp(), ..bare };
        luma(if lift { q.printed(at_grey) } else { super::laid(&q, pick, None).ended(q.printed(at_grey)) })
    };
    let time = root(start, |e| shown(e).max(1e-12).ln() - GREY_PRINT.ln());
    Some(super::lighting::lit(&bare, time, &above, s, k, cc))
}
