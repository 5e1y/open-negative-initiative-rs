//! The colour papers a negative may be printed on in place of ONI's own print: spektrafilm's profiles, and the Frontier
//! SP-3000's print refitted from its measured table.

use serde::{Deserialize, Serialize};

/// A colour paper by the key a sidecar stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum PaperStock {
    #[default]
    #[serde(rename = "kodak-portra-endura")]
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

    /// The profile's file, spektrafilm's name for it; none for the Frontier, a machine measured, not a profile.
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
}
