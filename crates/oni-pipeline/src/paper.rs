//! A colour paper printing the window's shares in place of ONI's print: each paper's tables and its own filtration baked
//! once from its profile or the Frontier's fit (`frontier`), each frame's enlarger time on it (`printing`) and Lighting on
//! the enlarger and the paper (`lighting`). A slide prints through its internegative (`internegative`); with Print on
//! paper off, any frame on a neutral paper carrying no paper's data (`neutral`).

use crate::film::bake;
use crate::film::cie::{self, WAVELENGTHS};
use crate::film::dyes;
use oni_render::PAPER_CURVE_NODES;
use oni_settings::{PaperStock, PipelineSettings};
use serde::Deserialize;
use std::sync::OnceLock;

type V3 = [f32; 3];
type V4 = [f32; 4];

pub mod frontier;
pub mod internegative;
mod lighting;
pub mod neutral;
mod printing;
pub use lighting::*;
pub use printing::*;

/// Kodak CC units: 100 is a density of 1 where the filter absorbs.
pub const CC_PER_DENSITY: f32 = 100.0;

/// Each layer reading its own channel's density: an enlarger's.
pub const OWN_LAYERS: [V3; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

#[derive(Deserialize)]
struct File {
    data: Data,
}

/// A paper's profile as its file holds it, written from its maker's data sheet, its curves sampled.
#[derive(Deserialize)]
struct Data {
    wavelengths: Vec<f64>,
    channel_density: Vec<[Option<f64>; 3]>,
    base_density: Vec<Option<f64>>,
    log_exposure: Vec<f64>,
    density_curves: Vec<[f64; 3]>,
}

/// A paper's profile file; none for the Frontier.
pub fn source(paper: PaperStock) -> Option<&'static [u8]> {
    dyes::source(paper.profile()?)
}

/// A paper's profile, refused off the grid, off its curves' uniform nodes or holding a number not finite.
fn parse(json: &[u8]) -> Result<Data, String> {
    let d = serde_json::from_slice::<File>(json).map_err(|e| e.to_string())?.data;
    let on_grid = d.wavelengths.len() == WAVELENGTHS && d.wavelengths.iter().enumerate().all(|(i, w)| *w == bake::wavelength(i));
    let sized = [d.channel_density.len(), d.base_density.len()].iter().all(|n| *n == WAVELENGTHS);
    if !on_grid || !sized {
        return Err("a paper off the 380 to 780 nm grid".into());
    }
    let le = &d.log_exposure;
    let step = le.last().zip(le.first()).map_or(0.0, |(h, l)| (h - l) / (PAPER_CURVE_NODES - 1) as f64);
    if le.len() != PAPER_CURVE_NODES || !le.iter().enumerate().all(|(i, e)| (e - le[0] - step * i as f64).abs() < 1e-6) {
        return Err("a paper's curves off their uniform nodes".into());
    }
    if d.density_curves.len() != PAPER_CURVE_NODES {
        return Err("a paper's curves off their nodes".into());
    }
    let numbers = le.iter().chain(d.density_curves.iter().flatten());
    if !numbers.chain(d.channel_density.iter().flatten().chain(&d.base_density).flatten()).all(|v| v.is_finite()) {
        return Err("a paper holding a number that is not finite".into());
    }
    Ok(d)
}

/// A paper as the stage reads it: its curves, its print's dyes and how they are shown, what each layer reads of the
/// three densities, and each layer's log exposure where it prints a grey neutral on its white, whence its filtration:
/// its white laid on pure white, or shown as its own with Print lift (`lifted`).
pub struct Sheet {
    pub curves: [V3; PAPER_CURVE_NODES],
    pub curve_low: f32,
    pub curve_step: f32,
    pub print: Vec<V4>,
    pub view: Vec<V3>,
    pub mix: [V3; 3],
    pub grey: V3,
    pub lifted: V3,
}

impl Sheet {
    /// A paper's tables, its greys solved on them once.
    pub fn new(curves: [V3; PAPER_CURVE_NODES], curve_low: f32, curve_step: f32, print: Vec<V4>, view: Vec<V3>, mix: [V3; 3]) -> Sheet {
        let mut sheet = Sheet { curves, curve_low, curve_step, print, view, mix, grey: [0.0; 3], lifted: [0.0; 3] };
        (sheet.grey, sheet.lifted) = (printing::own_grey(&sheet, false), printing::own_grey(&sheet, true));
        sheet
    }
}

/// Whether `json` reads as a paper's profile, and why not.
pub fn validate(json: &[u8]) -> Result<(), String> {
    parse(json).map(|_| ())
}

fn read(paper: PaperStock) -> Data {
    parse(source(paper).expect("a profiled paper")).expect("a shipped paper reads")
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
    Sheet::new(
        std::array::from_fn(|i| d.density_curves[i].map(|v| v as f32)),
        le[0] as f32,
        ((le[PAPER_CURVE_NODES - 1] - le[0]) / (PAPER_CURVE_NODES - 1) as f64) as f32,
        print,
        view,
        OWN_LAYERS,
    )
}

/// The paper's tables, baked at first use.
pub fn sheet(paper: PaperStock) -> &'static Sheet {
    static SHEETS: [OnceLock<Sheet>; PaperStock::ALL.len()] = [const { OnceLock::new() }; PaperStock::ALL.len()];
    let at = PaperStock::ALL.iter().position(|p| *p == paper).expect("every paper is listed");
    SHEETS[at].get_or_init(|| baked(paper))
}

/// The sheet `s` prints on: its paper's, or the neutral one with Print on paper off.
pub fn sheet_of(s: &PipelineSettings, paper: PaperStock) -> &'static Sheet {
    if s.paperless { neutral::sheet() } else { sheet(paper) }
}
