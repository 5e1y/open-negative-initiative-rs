//! The colour papers a negative may be printed on in place of ONI's own print, each a spektrafilm profile.

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
}

impl PaperStock {
    pub const ALL: [PaperStock; 5] = [
        PaperStock::KodakPortraEndura,
        PaperStock::KodakSupraEndura,
        PaperStock::KodakUltraEndura,
        PaperStock::KodakEnduraPremier,
        PaperStock::FujifilmCrystalArchiveTypeII,
    ];

    /// The paper's trade name, never translated.
    pub fn name(self) -> &'static str {
        match self {
            PaperStock::KodakPortraEndura => "Kodak Portra Endura",
            PaperStock::KodakSupraEndura => "Kodak Supra Endura",
            PaperStock::KodakUltraEndura => "Kodak Ultra Endura",
            PaperStock::KodakEnduraPremier => "Kodak Endura Premier",
            PaperStock::FujifilmCrystalArchiveTypeII => "Fujifilm Crystal Archive Type II",
        }
    }

    /// The profile's file, spektrafilm's name for it.
    pub fn profile(self) -> &'static str {
        match self {
            PaperStock::KodakPortraEndura => "kodak_portra_endura",
            PaperStock::KodakSupraEndura => "kodak_supra_endura",
            PaperStock::KodakUltraEndura => "kodak_ultra_endura",
            PaperStock::KodakEnduraPremier => "kodak_endura_premier",
            PaperStock::FujifilmCrystalArchiveTypeII => "fujifilm_crystal_archive_typeii",
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
    }
}
