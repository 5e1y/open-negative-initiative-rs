//! The paper every print is made on, printing the window's shares: a paper is its curves, each channel reading its own
//! dye and the others' Status A crosstalk through a pure dye on its own Rec. 2020 channel; its tables baked once from
//! its profile or the Frontier's fit (`frontier`), laid under Prepare's window with its own filtration (`placing`),
//! Lighting on the negative it prints (`lighting`). A legacy print keeps the time it was edited under (`legacy`); a
//! film's look prints through its internegative (`internegative`).

use crate::baked::Baked;
use crate::film::bake::{self, M3};
use oni_render::PAPER_CURVE_NODES;
use oni_settings::stocks::Print;
use oni_settings::{PaperStock, PipelineSettings};
use serde::Deserialize;
use std::sync::Arc;

type V3 = [f32; 3];

pub mod frontier;
pub mod internegative;
mod legacy;
mod lighting;
pub mod placing;
mod printing;
pub use lighting::*;
pub use printing::*;

/// Kodak CC units: 100 is a density of 1 where the filter absorbs.
pub const CC_PER_DENSITY: f32 = 100.0;

/// Each layer reading its own channel's density: an enlarger's.
pub const OWN_LAYERS: [V3; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// Each channel reading its own dye alone: the Frontier's, whose curves are fitted as read.
pub const PURE_DYES: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

#[derive(Deserialize)]
struct File {
    data: Data,
}

/// A paper's profile as its file holds it, written from its maker's data sheet: its Status A curves, D-min included,
/// sampled on a uniform log exposure, what each channel reads of each dye at a unit amount, and the log exposures
/// Prepare's white and black land on where the file states them (ONI Standard's).
#[derive(Deserialize)]
struct Data {
    log_exposure: Vec<f64>,
    density_curves: Vec<[f64; 3]>,
    crosstalk: M3,
    #[serde(default)]
    useful_range: Option<[f64; 2]>,
}

/// A paper's profile as its file holds it; none for the Frontier.
pub fn source(paper: PaperStock) -> Option<Arc<str>> {
    match paper.entry().print {
        Print::Profile(json) => Some(json),
        Print::Frontier(_) => None,
    }
}

/// A paper's profile, refused off its curves' uniform nodes, holding a number not finite or dyes that no amounts unmix.
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
    if !le.iter().chain(d.density_curves.iter().flatten()).chain(d.crosstalk.iter().flatten()).all(|v| v.is_finite()) {
        return Err("a paper holding a number that is not finite".into());
    }
    if !bake::inverse(&d.crosstalk).iter().flatten().all(|v| v.is_finite()) {
        return Err("a paper's crosstalk that no amounts unmix".into());
    }
    if let Some([white, black]) = d.useful_range
        && !(white.is_finite() && black.is_finite() && le[0] <= white && white < black && black <= le[PAPER_CURVE_NODES - 1])
    {
        return Err("a paper's useful range outside its curves, or its black under its white".into());
    }
    Ok(d)
}

/// A paper as the stage reads it: each layer's dye amount over the D-min along its curve, the D-min and the dyes' Status
/// A crosstalk, what each layer reads of the three densities, and where Prepare's window lies on it (`placing`); a legacy
/// print's greys too, each layer's log exposure where it prints a grey neutral, on its white or with Print lift.
pub struct Sheet {
    pub curves: [V3; PAPER_CURVE_NODES],
    pub curve_low: f32,
    pub curve_step: f32,
    pub floor: V3,
    pub crosstalk: [V3; 3],
    pub mix: [V3; 3],
    pub placing: placing::Placing,
    pub grey: V3,
    pub lifted: V3,
}

impl Sheet {
    /// A paper's tables from its sheet's `densities`, D-min first: each node's amounts the ones its densities unmix to
    /// over the D-min through the `crosstalk`, so a grey prints on the sheet's curves; laid under the window once, its
    /// `useful` range where its file states one.
    pub fn new(densities: [V3; PAPER_CURVE_NODES], curve_low: f32, curve_step: f32, mix: [V3; 3], crosstalk: M3, useful: Option<[f32; 2]>) -> Sheet {
        let (floor, unmix) = (densities[0], bake::inverse(&crosstalk));
        let mut curves = densities.map(|d| bake::apply(&unmix, std::array::from_fn(|c| f64::from(d[c] - floor[c]))).map(|a| a as f32));
        // An amount never falls with exposure: a dip the traces' crosstalk unmixes to is held, 0.004 D at most.
        for i in 1..PAPER_CURVE_NODES {
            curves[i] = std::array::from_fn(|c| curves[i][c].max(curves[i - 1][c]));
        }
        let crosstalk = crosstalk.map(|row| row.map(|v| v as f32));
        let unplaced = placing::Placing { offsets: [0.0; 3], black: 0.0, range: 1.0 };
        let mut sheet = Sheet { curves, curve_low, curve_step, floor, crosstalk, mix, placing: unplaced, grey: [0.0; 3], lifted: [0.0; 3] };
        sheet.placing = placing::placed(&sheet, useful);
        (sheet.grey, sheet.lifted) = (legacy::own_grey(&sheet, false), legacy::own_grey(&sheet, true));
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
        d.crosstalk,
        d.useful_range.map(|r| r.map(|v| v as f32)),
    )
}

/// The paper's tables, baked at first use and again once its file changes.
pub fn sheet(paper: PaperStock) -> &'static Sheet {
    static SHEETS: Baked<(PaperStock, u64), Sheet> = Baked::new();
    let entry = paper.entry();
    SHEETS.get((entry.paper, entry.revision), || baked(&entry.print))
}

