//! Each paper laid under Prepare's window (owner's call): the window's black, the bare film, exposes the paper's black
//! end of its useful range, 0.9 of its Dmax on its neutral, and the window's white its white end, its D-min and 0.04
//! (ISO 6846), unless its file states that range; no grey is placed. Each layer's own offset in log exposure, its
//! filtration, is the one that keeps a grey neutral along the whole range, each channel seen between its own ends.

use super::{Sheet, V3};
use oni_render::PAPER_CURVE_NODES;

/// The useful range's ends on the paper's neutral: its D-min plus 0.04, and 0.9 of its Dmax.
const WHITE_OVER_DMIN: f32 = 0.04;
const BLACK_OF_DMAX: f32 = 0.9;

/// The window shares a grey is weighed at, its ends included.
const SAMPLES: usize = 33;

/// The neutral solve's probe in log exposure, its largest step, its iterations and the least step it takes.
const PROBE: f32 = 1e-3;
const STEP_BOUND: f32 = 0.5;
const ITERATIONS: usize = 40;
const LEAST_STEP: f32 = 1e-6;

/// Where the window lies on a paper: each layer's log exposure over green's at one neutral exposure, the green layer's
/// log exposure under the bare film, and the log exposures from there to the window's white.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placing {
    pub offsets: V3,
    pub black: f32,
    pub range: f32,
}

/// Layer `c`'s dye amount at log exposure `log`, held at either end of its curve.
fn amount(sheet: &Sheet, c: usize, log: f32) -> f32 {
    let f = ((log - sheet.curve_low) / sheet.curve_step).clamp(0.0, (PAPER_CURVE_NODES - 1) as f32);
    let i = f.floor().min((PAPER_CURVE_NODES - 2) as f32);
    let at = i as usize;
    sheet.curves[at][c] + (f - i) * (sheet.curves[at + 1][c] - sheet.curves[at][c])
}

/// Each channel's density over the paper with its layers exposed at `log`.
fn density(sheet: &Sheet, log: V3) -> V3 {
    let amounts: V3 = std::array::from_fn(|c| amount(sheet, c, log[c]));
    std::array::from_fn(|c| sheet.floor[c] + (0..3).map(|k| sheet.crosstalk[c][k] * amounts[k]).sum::<f32>())
}

/// The layers' log exposures at green's `x`, each offset by its own.
fn exposed(x: f32, offsets: V3) -> V3 {
    offsets.map(|o| x + o)
}

/// The useful range on the paper's neutral, the three channels' mean density, with its layers at `offsets`: green's log
/// exposure at its white end and at its black end.
pub fn useful_range(sheet: &Sheet, offsets: V3) -> [f32; 2] {
    let mut neutral = [0.0f32; PAPER_CURVE_NODES];
    let mut most = f32::NEG_INFINITY;
    for (i, n) in neutral.iter_mut().enumerate() {
        let d = density(sheet, exposed(sheet.curve_low + sheet.curve_step * i as f32, offsets));
        // A neutral never falls with exposure, as its layers' amounts never do.
        most = most.max((d[0] + d[1] + d[2]) / 3.0);
        *n = most;
    }
    let at = |target: f32| {
        let i = neutral.iter().position(|d| *d >= target).unwrap_or(PAPER_CURVE_NODES - 1).max(1);
        let (low, high) = (neutral[i - 1], neutral[i]);
        let f = if high > low { ((target - low) / (high - low)).clamp(0.0, 1.0) } else { 1.0 };
        sheet.curve_low + sheet.curve_step * (i as f32 - 1.0 + f)
    };
    [at(neutral[0] + WHITE_OVER_DMIN), at(BLACK_OF_DMAX * neutral[PAPER_CURVE_NODES - 1])]
}

/// How far a grey along the window strays from neutral with the layers at `offsets`: at each share, the red and blue
/// channels' light between their own ends against green's, on a cube root as the eye weighs it.
fn strays(sheet: &Sheet, offsets: V3, useful: Option<[f32; 2]>) -> Vec<f32> {
    let [white, black] = useful.unwrap_or_else(|| useful_range(sheet, offsets));
    let light = |s: f32| oni_render::viewed(density(sheet, exposed(black - s * (black - white), offsets)));
    let (dark, bright) = (light(0.0), light(1.0));
    (0..SAMPLES)
        .flat_map(|i| {
            let v = light(i as f32 / (SAMPLES - 1) as f32);
            let seen: V3 = std::array::from_fn(|c| ((v[c] - dark[c]) / (bright[c] - dark[c])).cbrt());
            [seen[0] - seen[1], seen[2] - seen[1]]
        })
        .collect()
}

fn squared(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum()
}

/// The paper laid under the window: red's and blue's offsets over green's by Gauss-Newton's damped steps, then the useful
/// range they give, or `useful` where the file states it.
pub fn placed(sheet: &Sheet, useful: Option<[f32; 2]>) -> Placing {
    let at = |u: [f32; 2]| -> V3 { [u[0], 0.0, u[1]] };
    let mut u = [0.0f32; 2];
    for _ in 0..ITERATIONS {
        let r = strays(sheet, at(u), useful);
        let columns: [Vec<f32>; 2] = std::array::from_fn(|j| {
            let mut moved = u;
            moved[j] += PROBE;
            strays(sheet, at(moved), useful).iter().zip(&r).map(|(m, h)| (m - h) / PROBE).collect()
        });
        let dot = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
        let (a, b, d) = (dot(&columns[0], &columns[0]), dot(&columns[0], &columns[1]), dot(&columns[1], &columns[1]));
        let (g0, g1) = (dot(&columns[0], &r), dot(&columns[1], &r));
        let det = a * d - b * b;
        if !det.is_finite() || det.abs() < 1e-12 {
            break;
        }
        let step = [(d * g0 - b * g1) / det, (a * g1 - b * g0) / det];
        let within = step.iter().map(|v| STEP_BOUND / v.abs()).fold(1.0f32, f32::min);
        let step = step.map(|v| v * within);
        if step.iter().all(|v| v.abs() < LEAST_STEP) {
            break;
        }
        let moved = |t: f32| [u[0] - t * step[0], u[1] - t * step[1]];
        match super::printing::damped(squared(&r), |t| squared(&strays(sheet, at(moved(t)), useful))) {
            Some(t) => u = moved(t),
            None => break,
        }
    }
    let offsets = at(u);
    let [white, black] = useful.unwrap_or_else(|| useful_range(sheet, offsets));
    Placing { offsets, black, range: black - white }
}
