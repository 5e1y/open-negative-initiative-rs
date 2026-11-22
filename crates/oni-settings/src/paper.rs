//! The colour papers a photo may be printed on in place of ONI's own print: spektrafilm's profiles and the Frontier
//! SP-3000's print refitted from its measured table for a negative, Kodak's reversal paper for a slide.

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
    #[serde(rename = "kodak-ektachrome-radiance-iii")]
    KodakEktachromeRadianceIII,
}

impl PaperStock {
    pub const ALL: [PaperStock; 7] = [
        PaperStock::KodakPortraEndura,
        PaperStock::KodakSupraEndura,
        PaperStock::KodakUltraEndura,
        PaperStock::KodakEnduraPremier,
        PaperStock::FujifilmCrystalArchiveTypeII,
        PaperStock::FujifilmFrontierSp3000,
        PaperStock::KodakEktachromeRadianceIII,
    ];

    /// A reversal paper: its density falls with its light, so it prints a slide as a positive.
    pub fn reversal(self) -> bool {
        self == PaperStock::KodakEktachromeRadianceIII
    }

    /// The papers that print a slide (`positive`) or a negative, in the list's order.
    pub fn listed(positive: bool) -> Vec<PaperStock> {
        PaperStock::ALL.into_iter().filter(|p| p.reversal() == positive).collect()
    }

    /// The paper a slide (`positive`) or a negative takes where it has none of its own.
    pub fn default_for(positive: bool) -> PaperStock {
        if positive { PaperStock::KodakEktachromeRadianceIII } else { PaperStock::default() }
    }

    /// The paper's trade name, never translated.
    pub fn name(self) -> &'static str {
        match self {
            PaperStock::KodakPortraEndura => "Kodak Portra Endura",
            PaperStock::KodakSupraEndura => "Kodak Supra Endura",
            PaperStock::KodakUltraEndura => "Kodak Ultra Endura",
            PaperStock::KodakEnduraPremier => "Kodak Endura Premier",
            PaperStock::FujifilmCrystalArchiveTypeII => "Fujifilm Crystal Archive Type II",
            PaperStock::FujifilmFrontierSp3000 => "Fujifilm Frontier SP-3000",
            PaperStock::KodakEktachromeRadianceIII => "Kodak Ektachrome Radiance III",
        }
    }

    /// The profile's file, in spektrafilm's layout: spektrafilm's own, or Kodak's data sheet read by ONI for Radiance III;
    /// none for the Frontier, a machine measured, not a profile.
    pub fn profile(self) -> Option<&'static str> {
        match self {
            PaperStock::KodakPortraEndura => Some("kodak_portra_endura"),
            PaperStock::KodakSupraEndura => Some("kodak_supra_endura"),
            PaperStock::KodakUltraEndura => Some("kodak_ultra_endura"),
            PaperStock::KodakEnduraPremier => Some("kodak_endura_premier"),
            PaperStock::FujifilmCrystalArchiveTypeII => Some("fujifilm_crystal_archive_typeii"),
            PaperStock::FujifilmFrontierSp3000 => None,
            PaperStock::KodakEktachromeRadianceIII => Some("kodak_ektachrome_radiance_iii"),
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
    fn a_slide_lists_the_reversal_paper_alone_and_a_negative_every_other() {
        assert_eq!(PaperStock::listed(true), [PaperStock::KodakEktachromeRadianceIII]);
        assert_eq!(PaperStock::default_for(true), PaperStock::KodakEktachromeRadianceIII);
        let negative = PaperStock::listed(false);
        assert_eq!(negative.len(), PaperStock::ALL.len() - 1);
        assert!(negative.iter().all(|p| !p.reversal()) && negative.contains(&PaperStock::default_for(false)));
        // Twin: the negative's default is no reversal paper.
        assert!(!PaperStock::default_for(false).reversal());
    }
}
