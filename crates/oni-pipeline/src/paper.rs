//! A colour paper printing the film in place of ONI's print: each paper's tables baked once from its profile or the
//! Frontier's fit (`frontier`), each frame's enlarger at rest on it (`printing`) and Lighting on the enlarger and the
//! paper (`lighting`). A negative prints on a negative paper, a slide on a reversal paper.

use crate::film::bake;
use crate::film::cie::{self, WAVELENGTHS};
use crate::film::dyes;
use oni_render::PAPER_CURVE_NODES;
use oni_settings::{DyeSet, PaperStock, PipelineSettings};
use serde::Deserialize;
use std::sync::OnceLock;

type V3 = [f32; 3];
type V4 = [f32; 4];

pub mod frontier;
mod lighting;
mod printing;
pub use lighting::*;
pub use printing::*;

/// The enlarger's lamp, spektrafilm's TH-KG3: a 3400 K tungsten halogen behind a Schott KG3 heat filter.
pub const LAMP_KELVIN: f64 = 3400.0;

/// Kodak CC units: 100 is a density of 1 where the filter absorbs.
pub const CC_PER_DENSITY: f32 = 100.0;

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

/// A paper's profile file: spektrafilm's, shipped unchanged, or Radiance III's read off Kodak's data sheet
/// (`docs/research/positive-paper`); none for the Frontier.
pub fn source(paper: PaperStock) -> Option<&'static [u8]> {
    match paper {
        PaperStock::KodakPortraEndura => Some(include_bytes!("../resources/film/kodak_portra_endura.json")),
        PaperStock::KodakSupraEndura => Some(include_bytes!("../resources/film/kodak_supra_endura.json")),
        PaperStock::KodakUltraEndura => Some(include_bytes!("../resources/film/kodak_ultra_endura.json")),
        PaperStock::KodakEnduraPremier => Some(include_bytes!("../resources/film/kodak_endura_premier.json")),
        PaperStock::FujifilmCrystalArchiveTypeII => Some(include_bytes!("../resources/film/fujifilm_crystal_archive_typeii.json")),
        PaperStock::FujifilmFrontierSp3000 => None,
        PaperStock::KodakEktachromeRadianceIII => Some(include_bytes!("../resources/film/kodak_ektachrome_radiance_iii.json")),
    }
}

/// A paper as the stage reads it: its curves, its print's dyes and how they are shown.
pub struct Sheet {
    pub curves: [V3; PAPER_CURVE_NODES],
    pub curve_low: f32,
    pub curve_step: f32,
    pub print: Vec<V4>,
    pub view: Vec<V3>,
}

fn read(paper: PaperStock) -> Data {
    let file: File = serde_json::from_slice(source(paper).expect("a profiled paper")).expect("a shipped paper reads");
    let d = file.data;
    let on_grid = d.wavelengths.len() == WAVELENGTHS && d.wavelengths.iter().enumerate().all(|(i, w)| *w == bake::wavelength(i));
    assert!(on_grid && d.log_sensitivity.len() == WAVELENGTHS && d.channel_density.len() == WAVELENGTHS, "a paper on the 380 to 780 nm grid");
    assert!(d.log_exposure.len() == PAPER_CURVE_NODES, "a paper's curves on its nodes");
    let step = (d.log_exposure[PAPER_CURVE_NODES - 1] - d.log_exposure[0]) / (PAPER_CURVE_NODES - 1) as f64;
    assert!(d.log_exposure.iter().enumerate().all(|(i, e)| (e - d.log_exposure[0] - step * i as f64).abs() < 1e-9), "a uniform log exposure axis");
    d
}

fn baked(paper: PaperStock) -> Sheet {
    if paper == PaperStock::FujifilmFrontierSp3000 {
        return frontier::sheet();
    }
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

/// What the enlarger holds: a black and white negative's silver, a colour negative's dyes, or a slide's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Film {
    Silver,
    Negative(DyeSet),
    Slide,
}

impl Film {
    const COUNT: usize = DyeSet::ALL.len() + 2;

    /// What a frame lays in the enlarger: its silver, a slide's dyes or its negative's measured ones.
    pub fn of(s: &PipelineSettings) -> Film {
        match (s.mode.is_monochrome(), s.mode.reads_positive()) {
            (true, _) => Film::Silver,
            (false, true) => Film::Slide,
            (false, false) => Film::Negative(s.dye_set),
        }
    }

    fn index(self) -> usize {
        match self {
            Film::Silver => 0,
            Film::Slide => 1,
            Film::Negative(d) => 2 + DyeSet::ALL.iter().position(|s| *s == d).expect("every set is listed"),
        }
    }
}

/// A black and white negative's silver: neutral at every wavelength, its density the mean of its three readings.
fn silver() -> Vec<(usize, [f64; 4])> {
    (0..WAVELENGTHS).map(|l| (l, [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0, 0.0])).collect()
}

fn enlarging(film: Film, paper: PaperStock) -> Enlarger {
    if paper == PaperStock::FujifilmFrontierSp3000 {
        return frontier::enlarger(film == Film::Silver);
    }
    let bands = match film {
        Film::Silver => silver(),
        Film::Negative(set) => dyes::measured(set),
        Film::Slide => dyes::slide(),
    };
    let lamp = lamp();
    let s: Vec<[f64; 3]> = read(paper).log_sensitivity.iter().map(|s| s.map(|v| 10f64.powf(v))).collect();
    Enlarger {
        negative: bands.iter().map(|(_, d)| d.map(|v| v as f32)).collect(),
        exposure: bands.iter().map(|(l, _)| s[*l].map(|v| (v * lamp[*l]) as f32)).collect(),
        dichroic: bands.iter().map(|(l, _)| dichroic(*l).map(|v| v as f32)).collect(),
    }
}

/// The enlarger for `film` on `paper`, baked at first use; the Frontier's scanner reads the dye amounts themselves.
pub fn enlarger(film: Film, paper: PaperStock) -> &'static Enlarger {
    static TABLES: [OnceLock<Enlarger>; Film::COUNT * PaperStock::ALL.len()] = [const { OnceLock::new() }; Film::COUNT * PaperStock::ALL.len()];
    let at = PaperStock::ALL.iter().position(|p| *p == paper).expect("every paper is listed");
    TABLES[at * Film::COUNT + film.index()].get_or_init(|| enlarging(film, paper))
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
