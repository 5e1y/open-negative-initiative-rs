//! Each frame printed on the paper it names, its look as baked, its pick on black and its window's white on white;
//! Lighting then moves the negative it prints (`lighting`). The reference grey the Lighting sliders turn about is read
//! here too.

use super::*;
use crate::film::bake;
use oni_render::{Paper, Shown, Stage47, cpu};
use oni_settings::PipelineSettings;
use oni_settings::axis::pipeline::DMAX;
use oni_settings::levels::LevelsChannel;
use oni_settings::tone::Print;
use std::sync::LazyLock;

/// A frame's paper and the enlarger's filtration its look was baked through, in CC, none on a machine's print: what the
/// paper's block shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Printing {
    pub paper: Paper,
    pub filtration: Option<V3>,
}

pub(super) fn luma(v: V3) -> f32 {
    oni_render::LUMA.iter().zip(v).map(|(w, x)| w * x).sum()
}

/// What a plain RAW's render at rest prints a scene grey at: where a legacy print lays the frame's reference grey.
pub static GREY_PRINT: LazyLock<f32> = LazyLock::new(crate::film::plain_grey);

/// The share of every window Lighting's moves turn about: where the pure inversion shows a scene grey, its sRGB code; on
/// a legacy print, where its own curve at rest shows a plain RAW's grey, as the Scanner face turned it.
pub fn grey_share(s: &PipelineSettings) -> f32 {
    if s.legacy().is_none() {
        return 1.055 * crate::film::GREY.powf(1.0 / 2.4) - 0.055;
    }
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

/// Whether the print keeps the paper's own ends: Print lift.
pub fn lifted(s: &PipelineSettings) -> bool {
    s.print.print_lift
}

/// A dye's range on the negative, never empty: a window's span holds at least 1e-4 of density, as ONI's print reads it.
pub(crate) fn spanned(v: f32) -> f32 {
    if v.abs() < 1e-4 { 1e-4f32.copysign(v) } else { v }
}

/// The frame's paper at the window's shares: its look as baked, then Brightness and the ends on the print of the negative
/// as Lighting develops it. None where the frame names no paper, and while the print is bypassed.
pub fn printing(s: &PipelineSettings, k: &Stage47) -> Option<Printing> {
    let paper = s.paper().filter(|_| !s.print.bypassed)?;
    let sheet = sheet(paper)?;
    let (curves, crosstalk) = compressed(sheet, s.compression_strength());
    let at_rest = Paper {
        curves,
        curve_low: sheet.curve_low,
        curve_step: sheet.curve_step,
        floor: sheet.floor,
        crosstalk,
        primaries: sheet.primaries,
        latitude: sheet.latitude,
        brightness: 0.0,
        pick_share: 0.0,
        reach: 0.0,
        pick: [0.0; 3],
        white: [1.0; 3],
        anchor: [0.0; 3],
        white_anchor: [1.0; 3],
        luma: s.is_monochrome(),
    };
    // The window's white at rest is the light laid on white; Brightness's reach is sought up to it.
    let white = Paper { luma: false, ..at_rest }.printed([1.0; 3]);
    let (bare, densest) = film_ends(s);
    Some(super::lighting::lit(&at_rest, std::array::from_fn(|c| densest[c] - bare[c]), 1.0, white, s, k, sheet.filtration))
}

/// The look's curves and crosstalk at colour compression `k`: each channel reading its own dye alone at 0, the crosstalk
/// as baked at 1, the curves unmixed so a grey keeps its densities.
pub fn compressed(sheet: &Sheet, k: f32) -> ([V3; PAPER_CURVE_NODES], [V3; 3]) {
    // At 1 the look is its file's, whatever its crosstalk, an uninvertible one included.
    if k == 1.0 {
        return (sheet.curves, sheet.crosstalk);
    }
    let (x, k) = (sheet.crosstalk.map(|r| r.map(f64::from)), f64::from(k));
    let own = |r: usize, c: usize| if r == c { x[r][c] } else { 0.0 };
    let mix: bake::M3 = std::array::from_fn(|r| std::array::from_fn(|c| own(r, c) + k * (x[r][c] - own(r, c))));
    let side: bake::M3 = std::array::from_fn(|r| std::array::from_fn(|c| (1.0 - k) * (x[r][c] - own(r, c))));
    let n = bake::mul(&bake::inverse(&mix), &side);
    let curves = sheet.curves.map(|a| {
        let a = a.map(f64::from);
        let more = bake::apply(&n, a);
        std::array::from_fn(|c| (a[c] + more[c]) as f32)
    });
    (curves, mix.map(|r| r.map(|v| v as f32)))
}
