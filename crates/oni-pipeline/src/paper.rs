//! A colour paper printing the window's shares in place of ONI's print: a paper is its curves, each layer's density read
//! through a pure dye on its own Rec. 2020 channel; its tables and its own filtration baked once from its profile or the
//! Frontier's fit (`frontier`), each frame's enlarger time on it (`printing`) and Lighting on the enlarger and the paper
//! (`lighting`). A slide prints through its internegative (`internegative`); with Print on paper off, any frame on a
//! neutral paper, ONI's print as its curve (`neutral`).

use crate::baked::Baked;
use oni_render::PAPER_CURVE_NODES;
use oni_settings::stocks::Print;
use oni_settings::{PaperStock, PipelineSettings};
use serde::Deserialize;
use std::sync::Arc;

type V3 = [f32; 3];

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

/// A paper's profile as its file holds it, written from its maker's data sheet: its Status A curves, D-min included,
/// sampled on a uniform log exposure.
#[derive(Deserialize)]
struct Data {
    log_exposure: Vec<f64>,
    density_curves: Vec<[f64; 3]>,
}

/// A paper's profile as its file holds it; none for the Frontier.
pub fn source(paper: PaperStock) -> Option<Arc<str>> {
    match paper.entry().print {
        Print::Profile(json) => Some(json),
        Print::Frontier(_) => None,
    }
}

/// A paper's profile, refused off its curves' uniform nodes or holding a number not finite.
fn parse(json: &[u8]) -> Result<Data, String> {
    let d = serde_json::from_slice::<File>(json).map_err(|e| e.to_string())?.data;
    let le = &d.log_exposure;
    let step = le.last().zip(le.first()).map_or(0.0, |(h, l)| (h - l) / (PAPER_CURVE_NODES - 1) as f64);
    if le.len() != PAPER_CURVE_NODES || !le.iter().enumerate().all(|(i, e)| (e - le[0] - step * i as f64).abs() < 1e-6) {
        return Err("a paper's curves off their uniform nodes".into());
    }
    if d.density_curves.len() != PAPER_CURVE_NODES {
        return Err("a paper's curves off their nodes".into());
    }
    if !le.iter().chain(d.density_curves.iter().flatten()).all(|v| v.is_finite()) {
        return Err("a paper holding a number that is not finite".into());
    }
    Ok(d)
}

/// A paper as the stage reads it: its curves, what each layer reads of the three densities, and each layer's log exposure
/// where it prints a grey neutral on its white, whence its filtration: its white laid on pure white, or shown as its own
/// with Print lift (`lifted`).
pub struct Sheet {
    pub curves: [V3; PAPER_CURVE_NODES],
    pub curve_low: f32,
    pub curve_step: f32,
    pub mix: [V3; 3],
    pub grey: V3,
    pub lifted: V3,
}

impl Sheet {
    /// A paper's tables, its greys solved on them once.
    pub fn new(curves: [V3; PAPER_CURVE_NODES], curve_low: f32, curve_step: f32, mix: [V3; 3]) -> Sheet {
        let mut sheet = Sheet { curves, curve_low, curve_step, mix, grey: [0.0; 3], lifted: [0.0; 3] };
        (sheet.grey, sheet.lifted) = (printing::own_grey(&sheet, false), printing::own_grey(&sheet, true));
        sheet
    }
}

/// Whether `json` reads as a paper's profile, and why not.
pub fn validate(json: &[u8]) -> Result<(), String> {
    parse(json).map(|_| ())
}

fn read(json: &str) -> Data {
    parse(json.as_bytes()).expect("a paper reads once its file is taken")
}

fn baked(print: &Print) -> Sheet {
    let json = match print {
        Print::Frontier(fit) => return frontier::sheet(&frontier::read(fit).expect("a fit reads once its file is taken")),
        Print::Profile(json) => json,
    };
    let d = read(json);
    let le = &d.log_exposure;
    Sheet::new(
        std::array::from_fn(|i| d.density_curves[i].map(|v| v as f32)),
        le[0] as f32,
        ((le[PAPER_CURVE_NODES - 1] - le[0]) / (PAPER_CURVE_NODES - 1) as f64) as f32,
        OWN_LAYERS,
    )
}

/// The paper's tables, baked at first use and again once its file changes.
pub fn sheet(paper: PaperStock) -> &'static Sheet {
    static SHEETS: Baked<(PaperStock, u64), Sheet> = Baked::new();
    let entry = paper.entry();
    SHEETS.get((entry.paper, entry.revision), || baked(&entry.print))
}

/// The sheet `s` prints on: its paper's, or the neutral one with Print on paper off.
pub fn sheet_of(s: &PipelineSettings, paper: PaperStock) -> &'static Sheet {
    if s.paperless { neutral::sheet() } else { sheet(paper) }
}
