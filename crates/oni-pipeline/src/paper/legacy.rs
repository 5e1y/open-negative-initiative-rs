//! A legacy print on a paper keeps the enlarger it was edited under: the paper's own filtration, solved once on its layers
//! where they print a grey neutral on its white, and one time that shows the frame's reference grey at a plain RAW's grey
//! through the print's ends, the window's shares laid back on the negative's density about that grey.

use super::printing::{GREY_PRINT, anchors, bisected, damped, filters, filtration, grey_share, lifted, luma, shares_of, spanned};
use super::*;
use crate::film::bake::{self, M3};
use oni_render::{PAPER_CURVE_NODES, Paper, Stage47};

/// The neutral solve's probe and largest step in log exposure, its iterations and its tolerance in log light.
const PROBE: f32 = 0.01;
const STEP_BOUND: f32 = 1.0;
const ITERATIONS: usize = 40;
const TOLERANCE: f32 = 1e-6;

/// How far past the grey, in its own shares, Brightness's reach is sought: a print that never gets there takes the end.
const SOUGHT: f32 = 8.0;

/// How a share of the frame's window lies back on density, from its `anchors`: the windows' mean span, and the power
/// that lays the mean window's grey on the mean grey's own density, so the print reads the negative's density at one
/// grade on the mean.
fn laid_back(s: &PipelineSettings, anchors: &[V3; 3]) -> (f32, f32) {
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
pub(super) fn own_grey(sheet: &Sheet, lift: bool) -> V3 {
    let on = |c: usize, log: f32| {
        let f = ((log - sheet.curve_low) / sheet.curve_step).clamp(0.0, (PAPER_CURVE_NODES - 1) as f32);
        let i = f.floor().min((PAPER_CURVE_NODES - 2) as f32);
        let at = i as usize;
        sheet.curves[at][c] + (f - i) * (sheet.curves[at + 1][c] - sheet.curves[at][c])
    };
    let viewed = |amounts: V3| -> V3 {
        let row = |c: usize| (0..3).map(|k| sheet.crosstalk[c][k] * amounts[k]).sum::<f32>();
        oni_render::viewed(std::array::from_fn(|c| sheet.floor[c] + row(c)))
    };
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

/// The paper's own white, its D-min, the light a legacy print lays on white.
fn paper_white(p: &Paper) -> V3 {
    p.viewed(p.dyed(p.curves[0]))
}

/// A legacy print on its paper at the window's shares: the paper's own filtration, its time at rest showing the reference
/// grey at a plain RAW's grey, then Brightness and the ends, the paper's white on white, on the negative as Lighting
/// develops it.
pub(super) fn printing(s: &PipelineSettings, k: &Stage47, sheet: &'static Sheet) -> Printing {
    let above = anchors(s);
    let (span, bend) = laid_back(s, &above);
    let lift = lifted(s);
    let grey = if lift { sheet.lifted } else { sheet.grey };
    let cc = filtration(grey);
    let bare = Paper {
        span,
        bend,
        mix: sheet.mix,
        filters: filters(cc),
        scale: 1.0,
        curves: &sheet.curves,
        curve_low: sheet.curve_low,
        curve_step: sheet.curve_step,
        floor: sheet.floor,
        crosstalk: sheet.crosstalk,
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
        luma(if lift { q.printed(at_grey) } else { super::laid(&q, pick, paper_white(&q), None).ended(q.printed(at_grey)) })
    };
    let time = root(start, |e| shown(e).max(1e-12).ln() - GREY_PRINT.ln());
    let at_rest = Paper { scale: time.exp(), ..bare };
    super::lighting::lit(&at_rest, above[2], SOUGHT, paper_white(&at_rest), s, k, cc)
}
