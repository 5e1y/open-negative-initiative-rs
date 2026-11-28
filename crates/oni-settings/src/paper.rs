//! The papers a print may be made on: the colour papers each profiled from its maker's data sheet, and the Frontier
//! SP-3000's print refitted from its measured table. A paper is a choice laid over the pure inversion, never a default.

use crate::stocks::{PaperEntry, Print};

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

    /// A machine measured, the Frontier SP-3000, rather than a data sheet's profile.
    pub fn is_machine(self) -> bool {
        self.entry().is_some_and(|e| matches!(e.print, Print::Frontier(_)))
    }
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
