//! Each frame's enlarger on the paper it names: the window laid on the paper's useful range, its bare film on the paper's
//! black and its white on the paper's white, through the paper's own filtration (`placing`), no grey placed; Lighting then
//! moves the negative it prints (`lighting`). The reference grey the Lighting sliders turn about is read here too.

use super::*;
use oni_render::{Paper, Shown, Stage47, cpu};
use oni_settings::axis::pipeline::DMAX;
use oni_settings::levels::LevelsChannel;
use oni_settings::tone::Print;
use std::sync::LazyLock;

/// The halvings a solve's step may take before the solve stops where it stands.
const HALVINGS: i32 = 30;

/// A frame's paper and the enlarger's filtration it prints through, in CC: what the paper's block shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Printing {
    pub paper: Paper,
    pub filtration: V3,
}

/// What a plain RAW's render at rest prints a scene grey at: where a legacy print lays the frame's reference grey.
pub static GREY_PRINT: LazyLock<f32> = LazyLock::new(crate::film::plain_grey);

pub(super) fn filters(cc: V3) -> V3 {
    cc.map(|v| 10f32.powf(-v / CC_PER_DENSITY))
}

pub(super) fn luma(v: V3) -> f32 {
    oni_render::LUMA.iter().zip(v).map(|(w, x)| w * x).sum()
}

/// The share of every window Lighting's moves turn about: where the legacy print's curve at rest in `s`'s mode shows a
/// plain RAW's grey.
pub fn grey_share(s: &PipelineSettings) -> f32 {
    let rest = Print { exposure: crate::conversion::resting_print_stops(s.mode), ..Print::default() };
    bisected(0.0, 2.0, *GREY_PRINT, |t| rest.output(t, LevelsChannel::Linked))
}

/// The frame's reference grey, the scan densities through its gains where every window reads the one share Lighting's
/// moves turn about.
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
    let read = Stage47 { gain: [1.0; 3], pedestal: 0.0, negative: None, shown: Shown::Curve, ..*k };
    let light: V3 = std::array::from_fn(|c| 10f32.powf(-(bare[c] + above[c])));
    cpu::windowed([light[0], light[1], light[2], 1.0], &read)
}

/// The share of a solve's step that lowers the miss from `now`, halving it from the whole: a steep paper's step past
/// its root lands where its curve lies flat, from which no step returns.
pub(super) fn damped(now: f32, miss_at: impl Fn(f32) -> f32) -> Option<f32> {
    (0..HALVINGS).map(|i| 0.5f32.powi(i)).find(|t| miss_at(*t) < now)
}

/// The filtration in CC, its least at none, that brings one neutral density to each layer's log exposure `at`.
pub fn filtration(at: V3) -> V3 {
    let most = at.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    at.map(|e| (most - e) * CC_PER_DENSITY)
}

/// Whether the print keeps the paper's own ends: Print lift.
pub fn lifted(s: &PipelineSettings) -> bool {
    s.print.print_lift
}

/// A dye's range on the negative, never empty: a window's span holds at least 1e-4 of density, as ONI's print reads it.
pub(crate) fn spanned(v: f32) -> f32 {
    if v.abs() < 1e-4 { 1e-4f32.copysign(v) } else { v }
}

/// The frame's paper at the window's shares: the window's bare film on the paper's black and its white on the paper's
/// white through its own filtration, then Brightness and the ends on the print of the negative as Lighting develops
/// it. None where the frame names no paper, and while the print is bypassed.
pub fn printing(s: &PipelineSettings, k: &Stage47) -> Option<Printing> {
    let stock = s.paper().filter(|_| !s.print.bypassed)?;
    let sheet = sheet(stock)?;
    let placing = &sheet.placing;
    let cc = filtration(placing.offsets);
    let most = placing.offsets.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let at_rest = Paper {
        span: placing.range,
        bend: 1.0,
        mix: sheet.mix,
        filters: filters(cc),
        scale: 10f32.powf(placing.black + most),
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
    // The window's white at rest is the light laid on white; Brightness's reach is sought up to it.
    let white = Paper { luma: false, ..at_rest }.printed([1.0; 3]);
    let (bare, densest) = film_ends(s);
    Some(super::lighting::lit(&at_rest, std::array::from_fn(|c| densest[c] - bare[c]), 1.0, white, s, k, cc))
}
