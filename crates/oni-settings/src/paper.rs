//! The colour papers a photo may be printed on in place of ONI's own print: spektrafilm's profiles and the Frontier
//! SP-3000's print refitted from its measured table; a slide reaches them through an internegative.

use serde::{Deserialize, Serialize};

/// A colour paper by the key a sidecar stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum PaperStock {
    /// Also the key Kodak Ektachrome Radiance III was stored under, so a document naming it opens; its format's step
    /// says what it prints.
    #[default]
    #[serde(rename = "kodak-portra-endura", alias = "kodak-ektachrome-radiance-iii")]
    KodakPortraEndura,
    #[serde(rename = "kodak-supra-endura")]
    KodakSupraEndura,
    #[serde(rename = "kodak-ultra-endura")]
    KodakUltraEndura,
    #[serde(rename = "kodak-endura-premier")]
    KodakEnduraPremier,
    #[serde(rename = "fujifilm-crystal-archive-type-ii")]
    FujifilmCrystalArchiveTypeII,
    #[serde(rename = "fujifilm-frontier-sp-3000")]
    FujifilmFrontierSp3000,
}

impl PaperStock {
    pub const ALL: [PaperStock; 6] = [
        PaperStock::KodakPortraEndura,
        PaperStock::KodakSupraEndura,
        PaperStock::KodakUltraEndura,
        PaperStock::KodakEnduraPremier,
        PaperStock::FujifilmCrystalArchiveTypeII,
        PaperStock::FujifilmFrontierSp3000,
    ];

    /// Kodak Ektachrome Radiance III's key, which a document before format 58 may store.
    pub const RETIRED_REVERSAL_KEY: &str = "kodak-ektachrome-radiance-iii";

    /// The paper's trade name, never translated.
    pub fn name(self) -> &'static str {
        match self {
            PaperStock::KodakPortraEndura => "Kodak Portra Endura",
            PaperStock::KodakSupraEndura => "Kodak Supra Endura",
            PaperStock::KodakUltraEndura => "Kodak Ultra Endura",
            PaperStock::KodakEnduraPremier => "Kodak Endura Premier",
            PaperStock::FujifilmCrystalArchiveTypeII => "Fujifilm Crystal Archive Type II",
            PaperStock::FujifilmFrontierSp3000 => "Fujifilm Frontier SP-3000",
        }
    }

    /// The profile's file, spektrafilm's own; none for the Frontier, a machine measured, not a profile.
    pub fn profile(self) -> Option<&'static str> {
        match self {
            PaperStock::KodakPortraEndura => Some("kodak_portra_endura"),
            PaperStock::KodakSupraEndura => Some("kodak_supra_endura"),
            PaperStock::KodakUltraEndura => Some("kodak_ultra_endura"),
            PaperStock::KodakEnduraPremier => Some("kodak_endura_premier"),
            PaperStock::FujifilmCrystalArchiveTypeII => Some("fujifilm_crystal_archive_typeii"),
            PaperStock::FujifilmFrontierSp3000 => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_paper_keeps_its_key_and_the_default_is_portra_endura() {
        assert_eq!(serde_json::to_string(&PaperStock::default()).unwrap(), "\"kodak-portra-endura\"");
        for p in PaperStock::ALL {
            let back: PaperStock = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
            assert_eq!(back, p);
        }
        // Twin: another paper's key reads another paper.
        let other: PaperStock = serde_json::from_str("\"kodak-supra-endura\"").unwrap();
        assert_ne!(other, PaperStock::default());
        let frontier: PaperStock = serde_json::from_str("\"fujifilm-frontier-sp-3000\"").unwrap();
        assert_eq!((frontier, frontier.profile()), (PaperStock::FujifilmFrontierSp3000, None));
    }

    #[test]
    fn the_retired_reversal_papers_key_reads_as_the_default_paper() {
        let read: PaperStock = serde_json::from_str(&format!("\"{}\"", PaperStock::RETIRED_REVERSAL_KEY)).unwrap();
        assert_eq!(read, PaperStock::default());
        assert_eq!(serde_json::to_string(&read).unwrap(), "\"kodak-portra-endura\"");
        // Twin: an unknown key is refused.
        assert!(serde_json::from_str::<PaperStock>("\"kodak-ektachrome-radiance-iv\"").is_err());
    }
}
