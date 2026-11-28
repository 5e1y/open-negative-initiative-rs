//! The paper a frame may be printed on, a look at the chain's end: each layer's dye amount off its curve at its channel's
//! window share, the dyes' crosstalk, the light, then its ends. Every number is baked in the paper's file by
//! `scripts/paper-looks.py`, read once per file here (`sheet`), printed at the frame (`printing`), Lighting on the
//! negative it prints (`lighting`).

use crate::baked::Baked;
use oni_render::PAPER_CURVE_NODES;
use oni_settings::PaperStock;
use serde::Deserialize;

type V3 = [f32; 3];

mod lighting;
mod printing;
pub use lighting::*;
pub use printing::*;

/// A paper's look as its file holds it: the window's share its curves span, each layer's dye amount at each node, the
/// dyes' crosstalk and D-min, the light's primaries, the log exposure the window spans, and the enlarger's filtration
/// the bake laid where an enlarger printed it.
#[derive(Deserialize)]
struct Look {
    axis: [f32; 2],
    curves: Vec<V3>,
    crosstalk: [V3; 3],
    floor: V3,
    primaries: [V3; 3],
    latitude: f32,
    #[serde(default)]
    filtration: Option<V3>,
}

/// A paper as the stage reads it.
pub struct Sheet {
    pub curves: [V3; PAPER_CURVE_NODES],
    pub curve_low: f32,
    pub curve_step: f32,
    pub floor: V3,
    pub crosstalk: [V3; 3],
    pub primaries: [V3; 3],
    pub latitude: f32,
    /// The enlarger's filtration in CC, what the paper's block shows; none on a machine's print.
    pub filtration: Option<V3>,
}

/// A paper's look, refused off its nodes, holding a number not finite, or spanning no share or no exposure.
fn parse(json: &[u8]) -> Result<Sheet, String> {
    let l = serde_json::from_slice::<Look>(json).map_err(|e| e.to_string())?;
    if l.curves.len() != PAPER_CURVE_NODES {
        return Err(format!("a paper's look holds {} nodes, not {PAPER_CURVE_NODES}", l.curves.len()));
    }
    let numbers = l.axis.iter().chain(l.curves.iter().flatten()).chain(l.crosstalk.iter().flatten()).chain(&l.floor).chain(l.primaries.iter().flatten());
    if !numbers.chain([&l.latitude]).all(|v| v.is_finite()) || l.filtration.is_some_and(|f| !f.iter().all(|v| v.is_finite())) {
        return Err("a paper's look holding a number that is not finite".into());
    }
    if !(l.axis[0] < l.axis[1] && l.latitude > 0.0) {
        return Err("a paper's look spanning no share or no exposure".into());
    }
    Ok(Sheet {
        curves: std::array::from_fn(|i| l.curves[i]),
        curve_low: l.axis[0],
        curve_step: (l.axis[1] - l.axis[0]) / (PAPER_CURVE_NODES - 1) as f32,
        floor: l.floor,
        crosstalk: l.crosstalk,
        primaries: l.primaries,
        latitude: l.latitude,
        filtration: l.filtration,
    })
}

/// A paper's data-sheet profile as its file keeps it, the bake's source; none for a machine's, or a key no file defines.
pub fn source(paper: PaperStock) -> Option<std::sync::Arc<str>> {
    paper.entry()?.profile
}

/// Whether `json` reads as a paper's look, and why not.
pub fn validate(json: &[u8]) -> Result<(), String> {
    parse(json).map(|_| ())
}

/// The paper's tables, read at first use and again once its file changes; none for a key no file defines.
pub fn sheet(paper: PaperStock) -> Option<&'static Sheet> {
    static SHEETS: Baked<(PaperStock, u64), Sheet> = Baked::new();
    let entry = paper.entry()?;
    Some(SHEETS.get((entry.paper, entry.revision), || parse(entry.look.as_bytes()).expect("a look reads once its file is taken")))
}
