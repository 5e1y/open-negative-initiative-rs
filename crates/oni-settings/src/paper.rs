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
    #[serde(rename = "kodak-endura-premier-datasheet")]
    KodakEnduraPremierDatasheet,
    #[serde(rename = "kodak-endura-premier-metallic")]
    KodakEnduraPremierMetallic,
    #[serde(rename = "kodak-ektacolor-edge")]
    KodakEktacolorEdge,
    #[serde(rename = "kodak-royal-digital")]
    KodakRoyalDigital,
    #[serde(rename = "kodak-photo-book")]
    KodakPhotoBook,
    #[serde(rename = "fujifilm-crystal-archive-digital-dpii")]
    FujifilmCrystalArchiveDigitalDPII,
    #[serde(rename = "fujifilm-crystal-archive-digital-pearl")]
    FujifilmCrystalArchiveDigitalPearl,
    #[serde(rename = "fujifilm-fujiclear")]
    FujifilmFujiclear,
}

impl PaperStock {
    /// The order the paper menu lists them in, a data-sheet twin after the paper of its name.
    pub const ALL: [PaperStock; 14] = [
        PaperStock::KodakPortraEndura,
        PaperStock::KodakSupraEndura,
        PaperStock::KodakUltraEndura,
        PaperStock::KodakEnduraPremier,
        PaperStock::KodakEnduraPremierDatasheet,
        PaperStock::KodakEnduraPremierMetallic,
        PaperStock::KodakEktacolorEdge,
        PaperStock::KodakRoyalDigital,
        PaperStock::KodakPhotoBook,
        PaperStock::FujifilmCrystalArchiveTypeII,
        PaperStock::FujifilmCrystalArchiveDigitalDPII,
        PaperStock::FujifilmCrystalArchiveDigitalPearl,
        PaperStock::FujifilmFujiclear,
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
            PaperStock::KodakEnduraPremierDatasheet => "Kodak Endura Premier",
            PaperStock::KodakEnduraPremierMetallic => "Kodak Endura Premier Metallic",
            PaperStock::KodakEktacolorEdge => "Kodak Ektacolor Edge",
            PaperStock::KodakRoyalDigital => "Kodak Royal Digital",
            PaperStock::KodakPhotoBook => "Kodak Photo Book",
            PaperStock::FujifilmCrystalArchiveDigitalDPII => "Fujifilm Crystal Archive Digital Type DP II",
            PaperStock::FujifilmCrystalArchiveDigitalPearl => "Fujifilm Crystal Archive Digital Pearl",
            PaperStock::FujifilmFujiclear => "Fujifilm Fujiclear",
        }
    }

    /// The paper of its name rendered from its maker's data sheet: the menu names that source.
    pub fn sheet_twin(self) -> bool {
        self == PaperStock::KodakEnduraPremierDatasheet
    }

    /// The profile's file, spektrafilm's or under `datasheet/` one written from its maker's data sheet; none for the
    /// Frontier, a machine measured, not a profile.
    pub fn profile(self) -> Option<&'static str> {
        match self {
            PaperStock::KodakPortraEndura => Some("kodak_portra_endura"),
            PaperStock::KodakSupraEndura => Some("kodak_supra_endura"),
            PaperStock::KodakUltraEndura => Some("kodak_ultra_endura"),
            PaperStock::KodakEnduraPremier => Some("kodak_endura_premier"),
            PaperStock::FujifilmCrystalArchiveTypeII => Some("fujifilm_crystal_archive_typeii"),
            PaperStock::FujifilmFrontierSp3000 => None,
            PaperStock::KodakEnduraPremierDatasheet => Some("datasheet/kodak_endura_premier"),
            PaperStock::KodakEnduraPremierMetallic => Some("datasheet/kodak_endura_premier_metallic"),
            PaperStock::KodakEktacolorEdge => Some("datasheet/kodak_ektacolor_edge"),
            PaperStock::KodakRoyalDigital => Some("datasheet/kodak_royal_digital"),
            PaperStock::KodakPhotoBook => Some("datasheet/kodak_photo_book"),
            PaperStock::FujifilmCrystalArchiveDigitalDPII => Some("datasheet/fujifilm_crystal_archive_digital_paper_type_dpii"),
            PaperStock::FujifilmCrystalArchiveDigitalPearl => Some("datasheet/fujifilm_crystal_archive_digital_pearl"),
            PaperStock::FujifilmFujiclear => Some("datasheet/fujifilm_fujiclear_display_material_for_digital_printers"),
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
