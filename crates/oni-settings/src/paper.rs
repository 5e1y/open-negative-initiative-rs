//! The papers a print may be made on: the colour papers each profiled from its maker's data sheet, and the Frontier
//! SP-3000's print refitted from its measured table. A paper is a choice laid over the pure inversion, never a default.

use crate::stocks::PaperEntry;
use std::sync::Arc;

crate::stocks::keyed!(
    /// A paper by the key a sidecar stores, its file's (`crate::stocks`). A key no file defines is kept as stored and
    /// prints on no paper until its file is back.
    PaperStock, Paper
);

impl PaperStock {
    /// Every paper of the catalogue in use, in the menu's order.
    pub fn all() -> Vec<PaperStock> {
        crate::stocks::current().papers().iter().map(|p| p.paper).collect()
    }

    /// Its file's entry, none where no file defines it.
    pub fn entry(self) -> Option<PaperEntry> {
        crate::stocks::current().paper(self).cloned()
    }

    pub fn is_known(self) -> bool {
        crate::stocks::current().paper(self).is_some()
    }

    /// The paper's trade name, never translated; a key no file defines is named by itself.
    pub fn name(self) -> &'static str {
        self.entry().map_or(self.key(), |e| e.name)
    }

    /// Its maker's name; none for a key no file defines.
    pub fn brand(self) -> &'static str {
        self.entry().map_or("", |e| e.brand)
    }

    /// Its box's SVG beside its file, none where it has none or no file defines it.
    pub fn canister(self) -> Option<&'static str> {
        self.entry().and_then(|e| e.canister)
    }

    /// The colour compression scale its file's look bakes; none where it bakes none, which then prints as baked.
    pub fn compression(self) -> Option<Arc<CompressionScale>> {
        self.entry()?.compression
    }

    /// A machine measured, the Frontier SP-3000, rather than a data sheet's profile.
    pub fn is_machine(self) -> bool {
        self.entry().is_some_and(|e| e.profile.is_none())
    }
}

/// A paper's colour compression as its look bakes it (`scripts/paper-looks.py`): the strengths of its crosstalk the slider
/// runs through, from the pure dyes', 0, to its reach, its own at 1; and the share of a ring of saturated colours' chroma
/// each takes from the pure dyes' print, rising: the slider's absolute scale.
#[derive(Debug, Clone, PartialEq)]
pub struct CompressionScale {
    pub strengths: Vec<f32>,
    pub removed: Vec<f32>,
}

impl CompressionScale {
    /// The scale, refused unless it starts on the pure dyes, holds the paper's own strength, and rises on both.
    pub fn new(strengths: Vec<f32>, removed: Vec<f32>) -> Option<CompressionScale> {
        let rising = |v: &[f32]| v.windows(2).all(|w| w[1] > w[0]) && v.iter().all(|x| x.is_finite());
        let held = strengths.len() == removed.len() && strengths.len() >= 2 && strengths[0] == 0.0 && removed[0] == 0.0;
        (held && rising(&strengths) && rising(&removed) && strengths.contains(&1.0) && removed.iter().all(|r| *r <= 1.0))
            .then_some(CompressionScale { strengths, removed })
    }

    /// The share the paper takes at its own strength: its rest.
    pub fn rest(&self) -> f32 {
        self.removed[self.strengths.iter().position(|k| *k == 1.0).unwrap_or(0)]
    }

    /// The most it takes: its reach.
    pub fn reach(&self) -> f32 {
        self.removed[self.removed.len() - 1]
    }

    /// The strength taking `removed`, between the scale's nodes, held at its ends; a node's own strength exactly.
    pub fn strength(&self, removed: f32) -> f32 {
        along(&self.removed, &self.strengths, removed)
    }

    /// The share `strength` takes, between the scale's nodes, held at its ends.
    pub fn removed_at(&self, strength: f32) -> f32 {
        along(&self.strengths, &self.removed, strength)
    }
}

/// `y` at `x` along the rising `xs`, straight between nodes, held at either end; a node's own `y` exactly.
fn along(xs: &[f32], ys: &[f32], x: f32) -> f32 {
    let x = x.clamp(xs[0], xs[xs.len() - 1]);
    let i = xs.partition_point(|v| *v <= x).clamp(1, xs.len() - 1) - 1;
    ys[i] + (x - xs[i]) / (xs[i + 1] - xs[i]) * (ys[i + 1] - ys[i])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_paper_keeps_its_key_and_an_unknown_one_is_kept_as_stored() {
        for p in PaperStock::all() {
            let back: PaperStock = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
            assert_eq!(back, p);
            assert!(p.is_known() && !p.brand().is_empty());
        }
        let frontier: PaperStock = serde_json::from_str("\"fujifilm-frontier-sp-3000\"").unwrap();
        assert!(frontier.is_machine() && !PaperStock::of("kodak-endura-premier").is_machine());
        // Twin: an unknown key is kept as stored, never read as another, and names itself.
        let unknown: PaperStock = serde_json::from_str("\"oni-standard\"").unwrap();
        assert_eq!((unknown.key(), unknown.is_known(), unknown.name(), unknown.is_machine()), ("oni-standard", false, "oni-standard", false));
        assert_eq!(serde_json::to_string(&unknown).unwrap(), "\"oni-standard\"");
    }
}
