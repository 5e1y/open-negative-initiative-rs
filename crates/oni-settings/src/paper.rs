//! The colour papers a photo may be printed on in place of ONI's own print: each profiled from its maker's data sheet,
//! and the Frontier SP-3000's print refitted from its measured table; a slide reaches them through an internegative.

use crate::stocks::{PaperEntry, Print};

crate::stocks::keyed!(
    /// A colour paper by the key a sidecar stores, its file's (`crate::stocks`); the default is Kodak's softest data sheet,
    /// Royal Digital. A key no file defines is kept as stored and prints on the default meanwhile.
    PaperStock, Paper, "kodak-royal-digital"
);

impl PaperStock {
    /// Kodak Ektachrome Radiance III's key, which a document before format 58 may store.
    pub const RETIRED_REVERSAL_KEY: &str = "kodak-ektachrome-radiance-iii";

    /// Every paper of the catalogue in use, in the menu's order.
    pub fn all() -> Vec<PaperStock> {
        crate::stocks::current().papers().iter().map(|p| p.paper).collect()
    }

    /// Its file's entry; a key no file defines reads the default paper's.
    pub fn entry(self) -> PaperEntry {
        let catalogue = crate::stocks::current();
        catalogue.paper(self).or_else(|| catalogue.paper(PaperStock::default())).cloned().expect("the default paper is shipped")
    }

    pub fn is_known(self) -> bool {
        crate::stocks::current().paper(self).is_some()
    }

    /// The paper's trade name, never translated; a key no file defines is named by itself.
    pub fn name(self) -> &'static str {
        if self.is_known() { self.entry().name } else { self.key() }
    }

    pub fn brand(self) -> &'static str {
        self.entry().brand
    }

    /// A machine measured, the Frontier SP-3000, rather than a data sheet's profile.
    pub fn is_machine(self) -> bool {
        matches!(self.entry().print, Print::Frontier(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_paper_keeps_its_key_and_the_default_is_royal_digital() {
        assert_eq!(serde_json::to_string(&PaperStock::default()).unwrap(), "\"kodak-royal-digital\"");
        for p in PaperStock::all() {
            let back: PaperStock = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
            assert_eq!(back, p);
        }
        // Twin: another paper's key reads another paper.
        let other: PaperStock = serde_json::from_str("\"kodak-endura-premier\"").unwrap();
        assert_ne!(other, PaperStock::default());
        let frontier: PaperStock = serde_json::from_str("\"fujifilm-frontier-sp-3000\"").unwrap();
        assert!(frontier.is_machine() && !PaperStock::default().is_machine());
    }

    #[test]
    fn a_retired_papers_key_reads_as_the_paper_of_its_line() {
        let read = |key: &str| serde_json::from_str::<PaperStock>(&format!("\"{key}\"")).unwrap();
        for key in ["kodak-supra-endura", "kodak-ultra-endura", "kodak-endura-premier-datasheet"] {
            assert_eq!(read(key), PaperStock::of("kodak-endura-premier"), "{key}");
        }
        // The former defaults' keys read as the default, Royal Digital; twin: never as Endura Premier.
        for key in ["kodak-portra-endura", PaperStock::RETIRED_REVERSAL_KEY] {
            assert_eq!(read(key), PaperStock::default(), "{key}");
            assert_ne!(read(key), PaperStock::of("kodak-endura-premier"), "{key}");
        }
        let crystal = read("fujifilm-crystal-archive-type-ii");
        assert_eq!(crystal, PaperStock::of("fujifilm-crystal-archive-digital-dpii"));
        assert_eq!(serde_json::to_string(&crystal).unwrap(), "\"fujifilm-crystal-archive-digital-dpii\"", "written under its own key");
        // Twin: an unknown key is kept as stored, never read as another, and prints on the default.
        let unknown = read("kodak-ektachrome-radiance-iv");
        assert_eq!((unknown.key(), unknown.is_known()), ("kodak-ektachrome-radiance-iv", false));
        assert_eq!(serde_json::to_string(&unknown).unwrap(), "\"kodak-ektachrome-radiance-iv\"");
    }
}
