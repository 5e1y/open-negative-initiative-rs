//! A colour paper printing the negative in place of ONI's print: each paper's tables baked once from its spektrafilm
//! profile, and each frame's enlarger, set so the grey its windows print as a plain RAW's prints there too, neutral.

use crate::film::bake::{self, M3};
use crate::film::cie::{self, WAVELENGTHS};
use crate::film::dyes;
use oni_render::{DYE_LIGHT_FLOOR, DyeSeparation, PAPER_CURVE_NODES, Paper, Stage47, cpu};
use oni_settings::levels::LevelsChannel;
use oni_settings::tone::Print;
use oni_settings::{DyeSet, PaperStock, PipelineSettings};
use serde::Deserialize;
use std::sync::{LazyLock, OnceLock};

type V3 = [f32; 3];
type V4 = [f32; 4];

/// The enlarger's lamp, spektrafilm's TH-KG3: a 3400 K tungsten halogen behind a Schott KG3 heat filter.
pub const LAMP_KELVIN: f64 = 3400.0;

/// Kodak CC units: 100 is a density of 1 where the filter absorbs.
pub const CC_PER_DENSITY: f32 = 100.0;

/// A stop of filtration halves the light its filter absorbs, 30.1 CC: Temperature and Tint in the enlarger's units.
pub const CC_PER_STOP: f32 = CC_PER_DENSITY * std::f32::consts::LOG10_2;

/// Contrast's travel is a paper grade's: its whole track doubles or halves the paper's slope about the grey.
pub const CONTRAST_BASE: f32 = 4.0;

/// The neutral filtration's magenta and yellow at its start, its cyan at none as a colour enlarger's rests.
const START_CC: [f32; 2] = [50.0, 50.0];

/// The solve's probes, in log exposure and CC, its largest steps, its iterations and its tolerance in log light.
const PROBE: V3 = [0.01, 0.5, 0.5];
const STEP_BOUND: V3 = [1.0, 20.0, 20.0];
const ITERATIONS: usize = 40;
const TOLERANCE: f32 = 1e-6;

#[derive(Deserialize)]
struct File {
    data: Data,
}

#[derive(Deserialize)]
struct Data {
    wavelengths: Vec<f64>,
    log_sensitivity: Vec<[f64; 3]>,
    channel_density: Vec<[Option<f64>; 3]>,
    base_density: Vec<Option<f64>>,
    log_exposure: Vec<f64>,
    density_curves_model: Model,
}

/// The paper's curves as spektrafilm develops them: each layer a sum of normal distributions' integrals.
#[derive(Deserialize)]
struct Model {
    centers: [Vec<f64>; 3],
    amplitudes: [Vec<f64>; 3],
    sigmas: [Vec<f64>; 3],
}

impl Model {
    fn density(&self, c: usize, log_exposure: f64) -> f64 {
        let cdf = |z: f64| 0.5 * (1.0 + bake::erf(z / std::f64::consts::SQRT_2));
        (0..self.centers[c].len()).map(|i| self.amplitudes[c][i] * cdf((log_exposure - self.centers[c][i]) / self.sigmas[c][i])).sum()
    }
}

/// A paper's profile file, shipped unchanged.
pub fn source(paper: PaperStock) -> &'static [u8] {
    match paper {
        PaperStock::KodakPortraEndura => include_bytes!("../resources/film/kodak_portra_endura.json"),
        PaperStock::KodakSupraEndura => include_bytes!("../resources/film/kodak_supra_endura.json"),
        PaperStock::KodakUltraEndura => include_bytes!("../resources/film/kodak_ultra_endura.json"),
        PaperStock::KodakEnduraPremier => include_bytes!("../resources/film/kodak_endura_premier.json"),
        PaperStock::FujifilmCrystalArchiveTypeII => include_bytes!("../resources/film/fujifilm_crystal_archive_typeii.json"),
    }
}

/// A paper as the stage reads it: its layers' sensitivities, its curves, its print's dyes and how D50 shows them.
pub struct Sheet {
    pub sensitivity: Vec<[f64; 3]>,
    pub curves: [V3; PAPER_CURVE_NODES],
    pub curve_low: f32,
    pub curve_step: f32,
    pub print: Vec<V4>,
    pub view: Vec<V3>,
}

fn read(paper: PaperStock) -> Data {
    let file: File = serde_json::from_slice(source(paper)).expect("a shipped paper reads");
    let d = file.data;
    let on_grid = d.wavelengths.len() == WAVELENGTHS && d.wavelengths.iter().enumerate().all(|(i, w)| *w == bake::wavelength(i));
    assert!(on_grid && d.log_sensitivity.len() == WAVELENGTHS && d.channel_density.len() == WAVELENGTHS, "a paper on the 380 to 780 nm grid");
    assert!(d.log_exposure.len() == PAPER_CURVE_NODES, "a paper's curves on its nodes");
    let step = (d.log_exposure[PAPER_CURVE_NODES - 1] - d.log_exposure[0]) / (PAPER_CURVE_NODES - 1) as f64;
    assert!(d.log_exposure.iter().enumerate().all(|(i, e)| (e - d.log_exposure[0] - step * i as f64).abs() < 1e-9), "a uniform log exposure axis");
    d
}

fn baked(paper: PaperStock) -> Sheet {
    let d = read(paper);
    let to_rgb = bake::view_matrix(&cie::D50);
    let norm: f64 = (0..WAVELENGTHS).map(|l| cie::D50[l] * cie::Y[l]).sum();
    let (print, view) = (0..WAVELENGTHS)
        .filter_map(|l| {
            let [c, m, y] = d.channel_density[l];
            let dye = [c?, m?, y?, d.base_density[l]?].map(|v| v as f32);
            let xyz = [cie::X[l], cie::Y[l], cie::Z[l]].map(|v| v * cie::D50[l] / norm);
            Some((dye, bake::apply(&to_rgb, xyz).map(|v| v as f32)))
        })
        .unzip();
    let le = &d.log_exposure;
    Sheet {
        sensitivity: d.log_sensitivity.iter().map(|s| s.map(|v| 10f64.powf(v))).collect(),
        curves: std::array::from_fn(|i| std::array::from_fn(|c| d.density_curves_model.density(c, le[i]) as f32)),
        curve_low: le[0] as f32,
        curve_step: ((le[PAPER_CURVE_NODES - 1] - le[0]) / (PAPER_CURVE_NODES - 1) as f64) as f32,
        print,
        view,
    }
}

/// The paper's tables, baked at first use.
pub fn sheet(paper: PaperStock) -> &'static Sheet {
    static SHEETS: [OnceLock<Sheet>; PaperStock::ALL.len()] = [const { OnceLock::new() }; PaperStock::ALL.len()];
    let at = PaperStock::ALL.iter().position(|p| *p == paper).expect("every paper is listed");
    SHEETS[at].get_or_init(|| baked(paper))
}

/// scipy's Akima interpolation of `(x, y)` at `at`, inside the samples.
pub fn akima(x: &[f64], y: &[f64], at: f64) -> f64 {
    let n = x.len();
    let m: Vec<f64> = (0..n - 1).map(|i| (y[i + 1] - y[i]) / (x[i + 1] - x[i])).collect();
    let (mm, mp) = (2.0 * m[0] - m[1], 2.0 * m[n - 2] - m[n - 3]);
    let ext: Vec<f64> = [2.0 * mm - m[0], mm].into_iter().chain(m.iter().copied()).chain([mp, 2.0 * mp - m[n - 2]]).collect();
    let dm: Vec<f64> = ext.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
    let largest = (0..n).map(|i| dm[i + 2] + dm[i]).fold(0.0, f64::max);
    let slope = |i: usize| {
        let (f1, f2) = (dm[i + 2], dm[i]);
        if f1 + f2 > 1e-9 * largest { (f1 * ext[i + 1] + f2 * ext[i + 2]) / (f1 + f2) } else { 0.5 * (ext[i + 3] + ext[i]) }
    };
    let k = x.partition_point(|v| *v <= at).clamp(1, n - 1) - 1;
    let (h, s) = (x[k + 1] - x[k], at - x[k]);
    let (t0, t1) = (slope(k), slope(k + 1));
    y[k] + t0 * s + (3.0 * m[k] - 2.0 * t0 - t1) / h * s * s + (t0 + t1 - 2.0 * m[k]) / (h * h) * s * s * s
}

/// The Schott KG3's transmittance on the grid, read from its published curve, its repeated wavelengths kept once.
fn kg3() -> [f64; WAVELENGTHS] {
    let text = include_str!("../resources/film/schott_kg3.csv");
    let mut points: Vec<(f64, f64)> = text
        .lines()
        .filter_map(|l| {
            let mut v = l.split(',').map(|s| s.trim().parse::<f64>());
            Some((v.next()?.ok()?, v.next()?.ok()?))
        })
        .collect();
    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    points.dedup_by(|b, a| a.0 == b.0);
    let (x, y): (Vec<f64>, Vec<f64>) = points.into_iter().unzip();
    std::array::from_fn(|l| akima(&x, &y, bake::wavelength(l)))
}

/// The lamp's light on the grid, its mean 1 as spektrafilm scales an illuminant.
pub fn lamp() -> [f64; WAVELENGTHS] {
    // Planck's second radiation constant as colour-science rounds it, which spektrafilm's lamp is drawn with.
    let c2 = 0.014_388;
    let filter = kg3();
    let raw: [f64; WAVELENGTHS] = std::array::from_fn(|l| {
        let w = bake::wavelength(l) * 1e-9;
        filter[l] / (w.powi(5) * ((c2 / (w * LAMP_KELVIN)).exp() - 1.0))
    });
    let mean = raw.iter().sum::<f64>() / WAVELENGTHS as f64;
    raw.map(|v| v / mean)
}

/// spektrafilm's dichroic filters, cyan, magenta and yellow: erf edges at 607, 500 and 610, and 516 nm.
pub fn dichroic(l: usize) -> [f64; 3] {
    let w = bake::wavelength(l);
    let magenta = if w <= 550.0 { -bake::erf((w - 500.0) / 8.0) } else { bake::erf((w - 610.0) / 8.0) };
    [-bake::erf((w - 607.0) / 8.0), magenta, bake::erf((w - 516.0) / 12.0)].map(|v| v / 2.0 + 0.5)
}

/// The enlarger's light through one negative onto one paper: the negative's bands, the lamp read by each layer, the filters.
pub struct Enlarger {
    pub negative: Vec<V4>,
    pub exposure: Vec<V3>,
    pub dichroic: Vec<V3>,
}

/// A black and white negative's silver: neutral at every wavelength, its density the mean of its three readings.
fn silver() -> Vec<(usize, [f64; 4])> {
    (0..WAVELENGTHS).map(|l| (l, [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0, 0.0])).collect()
}

fn enlarging(set: Option<DyeSet>, paper: PaperStock) -> Enlarger {
    let bands = set.map_or_else(silver, dyes::measured);
    let lamp = lamp();
    let s = &sheet(paper).sensitivity;
    Enlarger {
        negative: bands.iter().map(|(_, d)| d.map(|v| v as f32)).collect(),
        exposure: bands.iter().map(|(l, _)| s[*l].map(|v| (v * lamp[*l]) as f32)).collect(),
        dichroic: bands.iter().map(|(l, _)| dichroic(*l).map(|v| v as f32)).collect(),
    }
}

/// The enlarger for a negative's dyes, or a black and white's silver, on `paper`, baked at first use.
pub fn enlarger(set: Option<DyeSet>, paper: PaperStock) -> &'static Enlarger {
    const NEGATIVES: usize = DyeSet::ALL.len() + 1;
    static TABLES: [OnceLock<Enlarger>; NEGATIVES * PaperStock::ALL.len()] = [const { OnceLock::new() }; NEGATIVES * PaperStock::ALL.len()];
    let negative = set.map_or(0, |d| 1 + DyeSet::ALL.iter().position(|s| *s == d).expect("every set is listed"));
    let at = PaperStock::ALL.iter().position(|p| *p == paper).expect("every paper is listed");
    TABLES[at * NEGATIVES + negative].get_or_init(|| enlarging(set, paper))
}

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

fn ln_luma(v: V3) -> f32 {
    oni_render::LUMA.iter().zip(v).map(|(w, x)| w * x).sum::<f32>().max(1e-12).ln()
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

/// The frame's paper, its reading through stage 4's gains and the separation's dye amounts where one runs, its
/// enlarger set so the reference grey prints at a plain RAW's grey, neutral, then Lighting's filters, the luma held,
/// its exposure and its contrast; none where the frame prints on ONI's own print.
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
    let mut paper = Paper {
        gain,
        pedestal,
        log_guard,
        black: read_black,
        negative: &table.negative,
        exposure: &table.exposure,
        dichroic: &table.dichroic,
        filters: [1.0; 3],
        scale: 1.0,
        pivot: [0.0; 3],
        contrast: 1.0,
        curves: &sheet.curves,
        curve_low: sheet.curve_low,
        curve_step: sheet.curve_step,
        print: &sheet.print,
        view: &sheet.view,
    };
    let grey = reference_grey(s);
    let above: V3 = std::array::from_fn(|c| grey[c] - black[c]);
    let amounts = separation.map_or(above, |d| d.separated(above));
    let printed = |p: &Paper| p.viewed(p.developed(p.exposed(amounts)));
    let target = GREY_PRINT.ln();
    // The neutral filtration and exposure, cyan at none: Newton on each channel's log light.
    let at = |x: V3| Paper { scale: x[0].exp(), filters: filters([0.0, x[1], x[2]]), ..paper };
    let miss = |x: V3| printed(&at(x)).map(|v| v.max(1e-12).ln() - target);
    let raw = paper.exposed(amounts);
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
    // Lighting's filters move the neutral one, the least filter at none, the reference grey's luma held.
    let moved = s.print.filters();
    let neutral = [0.0, x[1], x[2]];
    let cc: V3 = std::array::from_fn(|c| neutral[c] + CC_PER_STOP * moved[c]);
    let least = cc.iter().copied().fold(f32::INFINITY, f32::min);
    let cc = cc.map(|v| v - least);
    let luma_target = ln_luma([*GREY_PRINT; 3]);
    let mut scale = x[0];
    for _ in 0..ITERATIONS {
        let here = ln_luma(printed(&Paper { scale: scale.exp(), filters: filters(cc), ..paper }));
        if (here - luma_target).abs() < TOLERANCE {
            break;
        }
        let probe = ln_luma(printed(&Paper { scale: (scale + PROBE[0]).exp(), filters: filters(cc), ..paper }));
        let slope = (probe - here) / PROBE[0];
        if slope.is_nan() || slope.abs() <= 1e-6 {
            break;
        }
        scale -= ((here - luma_target) / slope).clamp(-STEP_BOUND[0], STEP_BOUND[0]);
    }
    paper.filters = filters(cc);
    paper.scale = scale.exp();
    paper.pivot = paper.exposed(amounts);
    paper.contrast = CONTRAST_BASE.powf(s.print.contrast);
    // Exposure brightens the print as it does ONI's: a stop up is a stop less light on the paper.
    paper.scale *= (Print::resting_exposure(s.mode) - s.print.exposure).exp2();
    Some(Printing { paper, filtration: cc })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn akima_runs_through_its_samples_and_follows_a_cubic() {
        let x: Vec<f64> = (0..12).map(|i| f64::from(i) * 1.5).collect();
        let y: Vec<f64> = x.iter().map(|v| v * v * 0.1 - v).collect();
        assert!((akima(&x, &y, 4.5) - y[3]).abs() < 1e-12, "through a sample");
        let between = akima(&x, &y, 5.25);
        assert!((between - (5.25 * 5.25 * 0.1 - 5.25)).abs() < 2e-2, "{between}");
        // Twin: a straight line read between the same samples misses the parabola.
        assert!(((y[3] + y[4]) / 2.0 - (5.25 * 5.25 * 0.1 - 5.25)).abs() > 0.05);
    }

    #[test]
    fn the_dichroics_pass_their_bands_and_stop_their_own() {
        let at = |nm: f64| ((nm - cie::FIRST_NM) / cie::STEP_NM) as usize;
        let blue = dichroic(at(450.0));
        assert!(blue[0] > 0.99 && blue[1] > 0.99 && blue[2] < 0.01, "a yellow filter stops blue: {blue:?}");
        let green = dichroic(at(550.0));
        assert!(green[0] > 0.99 && green[1] < 0.01 && green[2] > 0.99, "{green:?}");
        let red = dichroic(at(650.0));
        assert!(red[0] < 0.01 && red[1] > 0.99 && red[2] > 0.99, "{red:?}");
        // Twin: no filter stops the green the magenta filter does.
        assert!(green[0] > 0.5 && green[2] > 0.5);
    }
}
