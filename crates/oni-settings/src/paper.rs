//! The colour papers a photo may be printed on in place of ONI's own print: each profiled from its maker's data sheet,
//! and the Frontier SP-3000's print refitted from its measured table; a slide reaches them through an internegative.

use serde::{Deserialize, Serialize};

/// A colour paper by the key a sidecar stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum PaperStock {
    /// Also the keys of Supra and Ultra Endura, which Kodak folded into it, and of its former profile, so a document naming
    /// one opens on it.
    #[serde(
        rename = "kodak-endura-premier",
        alias = "kodak-supra-endura",
        alias = "kodak-ultra-endura",
        alias = "kodak-endura-premier-datasheet"
    )]
    KodakEnduraPremier,
    #[serde(rename = "kodak-endura-premier-metallic")]
    KodakEnduraPremierMetallic,
    #[serde(rename = "kodak-ektacolor-edge")]
    KodakEktacolorEdge,
    /// The default, Kodak's softest data sheet; also the keys of the former defaults, Portra Endura and Ektachrome Radiance
    /// III, so a photo left on the default stays there; its format's step says what a slide on Radiance III prints.
    #[default]
    #[serde(rename = "kodak-royal-digital", alias = "kodak-portra-endura", alias = "kodak-ektachrome-radiance-iii")]
    KodakRoyalDigital,
    #[serde(rename = "kodak-photo-book")]
    KodakPhotoBook,
    /// Also the key Crystal Archive Type II was stored under, the digital paper of its line.
    #[serde(rename = "fujifilm-crystal-archive-digital-dpii", alias = "fujifilm-crystal-archive-type-ii")]
    FujifilmCrystalArchiveDigitalDPII,
    #[serde(rename = "fujifilm-crystal-archive-digital-pearl")]
    FujifilmCrystalArchiveDigitalPearl,
    #[serde(rename = "fujifilm-fujiclear")]
    FujifilmFujiclear,
    #[serde(rename = "fujifilm-frontier-sp-3000")]
    FujifilmFrontierSp3000,
}

impl PaperStock {
    /// The order the paper menu lists them in.
    pub const ALL: [PaperStock; 9] = [
        PaperStock::KodakEnduraPremier,
        PaperStock::KodakEnduraPremierMetallic,
        PaperStock::KodakEktacolorEdge,
        PaperStock::KodakRoyalDigital,
        PaperStock::KodakPhotoBook,
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
            PaperStock::KodakEnduraPremier => "Kodak Endura Premier",
            PaperStock::KodakEnduraPremierMetallic => "Kodak Endura Premier Metallic",
            PaperStock::KodakEktacolorEdge => "Kodak Ektacolor Edge",
            PaperStock::KodakRoyalDigital => "Kodak Royal Digital",
            PaperStock::KodakPhotoBook => "Kodak Photo Book",
            PaperStock::FujifilmCrystalArchiveDigitalDPII => "Fujifilm Crystal Archive Digital Type DP II",
            PaperStock::FujifilmCrystalArchiveDigitalPearl => "Fujifilm Crystal Archive Digital Pearl",
            PaperStock::FujifilmFujiclear => "Fujifilm Fujiclear",
            PaperStock::FujifilmFrontierSp3000 => "Fujifilm Frontier SP-3000",
        }
    }

    /// The profile's file, written from its maker's data sheet; none for the Frontier, a machine measured, not a profile.
    pub fn profile(self) -> Option<&'static str> {
        match self {
            PaperStock::KodakEnduraPremier => Some("kodak_endura_premier"),
            PaperStock::KodakEnduraPremierMetallic => Some("kodak_endura_premier_metallic"),
            PaperStock::KodakEktacolorEdge => Some("kodak_ektacolor_edge"),
            PaperStock::KodakRoyalDigital => Some("kodak_royal_digital"),
            PaperStock::KodakPhotoBook => Some("kodak_photo_book"),
            PaperStock::FujifilmCrystalArchiveDigitalDPII => Some("fujifilm_crystal_archive_digital_paper_type_dpii"),
            PaperStock::FujifilmCrystalArchiveDigitalPearl => Some("fujifilm_crystal_archive_digital_pearl"),
            PaperStock::FujifilmFujiclear => Some("fujifilm_fujiclear_display_material_for_digital_printers"),
            PaperStock::FujifilmFrontierSp3000 => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_paper_keeps_its_key_and_the_default_is_royal_digital() {
        assert_eq!(serde_json::to_string(&PaperStock::default()).unwrap(), "\"kodak-royal-digital\"");
        for p in PaperStock::ALL {
            let back: PaperStock = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
            assert_eq!(back, p);
        }
        // Twin: another paper's key reads another paper.
        let other: PaperStock = serde_json::from_str("\"kodak-endura-premier\"").unwrap();
        assert_ne!(other, PaperStock::default());
        let frontier: PaperStock = serde_json::from_str("\"fujifilm-frontier-sp-3000\"").unwrap();
        assert_eq!((frontier, frontier.profile()), (PaperStock::FujifilmFrontierSp3000, None));
    }

    #[test]
    fn a_retired_papers_key_reads_as_the_paper_of_its_line() {
        let read = |key: &str| serde_json::from_str::<PaperStock>(&format!("\"{key}\"")).unwrap();
        for key in ["kodak-supra-endura", "kodak-ultra-endura", "kodak-endura-premier-datasheet"] {
            assert_eq!(read(key), PaperStock::KodakEnduraPremier, "{key}");
        }
        // The former defaults' keys read as the default, Royal Digital; twin: never as Endura Premier.
        for key in ["kodak-portra-endura", PaperStock::RETIRED_REVERSAL_KEY] {
            assert_eq!(read(key), PaperStock::default(), "{key}");
            assert_ne!(read(key), PaperStock::KodakEnduraPremier, "{key}");
        }
        let crystal = read("fujifilm-crystal-archive-type-ii");
        assert_eq!(crystal, PaperStock::FujifilmCrystalArchiveDigitalDPII);
        assert_eq!(serde_json::to_string(&crystal).unwrap(), "\"fujifilm-crystal-archive-digital-dpii\"", "written under its own key");
        // Twin: an unknown key is refused.
        assert!(serde_json::from_str::<PaperStock>("\"kodak-ektachrome-radiance-iv\"").is_err());
    }
}
