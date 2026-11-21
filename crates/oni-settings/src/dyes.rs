//! The measured dye sets the dye separation unmixes: one per stock profile ONI ships, and a generic
//! colour-negative set, their mean, for a stock no profile measures.

use crate::film::FILM_STOCKS;
use serde::{Deserialize, Serialize};

/// A dye set by the key a sidecar stores: its stock's spectral dye densities and base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DyeSet {
    #[default]
    #[serde(rename = "generic-c41")]
    GenericC41,
    #[serde(rename = "kodak-portra-400")]
    KodakPortra400,
    #[serde(rename = "kodak-portra-800")]
    KodakPortra800,
    #[serde(rename = "kodak-gold-200")]
    KodakGold200,
    #[serde(rename = "kodak-ultramax-400")]
    KodakUltraMax400,
    #[serde(rename = "kodak-vision3-50d")]
    KodakVision3_50D,
    #[serde(rename = "kodak-vision3-500t")]
    KodakVision3_500T,
    #[serde(rename = "fujifilm-c200")]
    FujifilmC200,
    #[serde(rename = "fujifilm-pro-400h")]
    FujifilmPro400H,
    #[serde(rename = "fujifilm-xtra-400")]
    FujifilmXtra400,
}

impl DyeSet {
    pub const ALL: [DyeSet; 10] = [
        DyeSet::GenericC41,
        DyeSet::KodakPortra400,
        DyeSet::KodakPortra800,
        DyeSet::KodakGold200,
        DyeSet::KodakUltraMax400,
        DyeSet::KodakVision3_50D,
        DyeSet::KodakVision3_500T,
        DyeSet::FujifilmC200,
        DyeSet::FujifilmPro400H,
        DyeSet::FujifilmXtra400,
    ];

    /// The stock whose profile measured the dyes; the generic set names none.
    pub fn stock_name(self) -> Option<&'static str> {
        Some(match self {
            DyeSet::GenericC41 => return None,
            DyeSet::KodakPortra400 => "Kodak Portra 400",
            DyeSet::KodakPortra800 => "Kodak Portra 800",
            DyeSet::KodakGold200 => "Kodak Gold 200",
            DyeSet::KodakUltraMax400 => "Kodak UltraMax 400",
            DyeSet::KodakVision3_50D => "Kodak Vision3 50D",
            DyeSet::KodakVision3_500T => "Kodak Vision3 500T",
            DyeSet::FujifilmC200 => "Fujifilm C200",
            DyeSet::FujifilmPro400H => "Fujifilm Pro 400H",
            DyeSet::FujifilmXtra400 => "Fujifilm Superia X-TRA 400",
        })
    }

    /// The profile's file, spektrafilm's name for it.
    pub fn profile(self) -> Option<&'static str> {
        Some(match self {
            DyeSet::GenericC41 => return None,
            DyeSet::KodakPortra400 => "kodak_portra_400",
            DyeSet::KodakPortra800 => "kodak_portra_800",
            DyeSet::KodakGold200 => "kodak_gold_200",
            DyeSet::KodakUltraMax400 => "kodak_ultramax_400",
            DyeSet::KodakVision3_50D => "kodak_vision3_50d",
            DyeSet::KodakVision3_500T => "kodak_vision3_500t",
            DyeSet::FujifilmC200 => "fujifilm_c200",
            DyeSet::FujifilmPro400H => "fujifilm_pro_400h",
            DyeSet::FujifilmXtra400 => "fujifilm_xtra_400",
        })
    }

    /// No profile measured these dyes: they stand for any colour negative, approximately.
    pub fn is_generic(self) -> bool {
        self == DyeSet::GenericC41
    }
}

/// Each stock's measured dyes, which picking it writes on the roll's frames: its own profile's where ONI ships
/// one, the generic set for a colour negative none measures; none in black and white or on a slide, never unmixed.
pub const STOCK_DYE_SETS: [(&str, Option<DyeSet>); FILM_STOCKS.len()] = {
    use DyeSet::*;
    [
        ("generic-azure", Some(GenericC41)),
        ("generic-black-and-white", None),
        ("generic-color-negative", Some(GenericC41)),
        ("generic-color-positive", None),
        ("generic-redscale", Some(GenericC41)),
        ("agfaphoto-apx", None),
        ("cinestill-400d", Some(GenericC41)),
        // CineStill's 50D and 800T are Kodak's Vision3 50D and 500T, their remjet removed.
        ("cinestill-50d", Some(KodakVision3_50D)),
        ("cinestill-800t", Some(KodakVision3_500T)),
        ("foma-fomapan200", None),
        ("foma-fomapan400", None),
        ("foma-fomapan401", None),
        ("fujifilm-acros", None),
        ("fujifilm-fujicolor", Some(FujifilmC200)),
        ("fujifilm-fujicolor-pro", Some(FujifilmPro400H)),
        ("fujifilm-provia100", None),
        ("fujifilm-superia", Some(FujifilmXtra400)),
        ("fujifilm-velvia100", None),
        ("fujifilm-velvia50", None),
        ("harman-phoenix", Some(GenericC41)),
        ("harman-phoenixII", Some(GenericC41)),
        ("harman-red", Some(GenericC41)),
        ("ilford-delta100", None),
        ("ilford-delta3200", None),
        ("ilford-delta400", None),
        ("ilford-fp4", None),
        ("ilford-hp5", None),
        ("ilford-ortho", None),
        ("ilford-panf", None),
        ("ilford-sfx", None),
        ("ilford-xp2", None),
        ("kodak-400tx", None),
        ("kodak-colorplus", Some(GenericC41)),
        ("kodak-ektachrome", None),
        ("kodak-gold", Some(KodakGold200)),
        ("kodak-portra", Some(KodakPortra400)),
        ("kodak-ultramax", Some(KodakUltraMax400)),
        ("lomography-lomo-turquoise", Some(GenericC41)),
        ("lucky-color200", Some(GenericC41)),
        ("mira-color400", Some(GenericC41)),
        ("mira-color800", Some(GenericC41)),
    ]
};

/// The measured dyes `stock_id` writes on its roll's frames: none in black and white, on a slide or for a key the
/// catalogue lacks.
pub fn dye_set(stock_id: &str) -> Option<DyeSet> {
    STOCK_DYE_SETS.iter().find(|(id, _)| *id == stock_id).and_then(|(_, set)| *set)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_colour_negative_names_its_measured_dyes_and_only_a_colour_negative_does() {
        let unmixed = |id: &str| !id.contains("positive") && !["velvia", "provia", "ektachrome"].iter().any(|s| id.contains(s));
        for (kind, (id, set)) in FILM_STOCKS.iter().zip(STOCK_DYE_SETS) {
            assert_eq!(kind.id, id, "the dye sets follow the catalogue's order");
            if set.is_some() {
                assert!(unmixed(id), "{id}: a slide's dyes are never unmixed");
            }
        }
        let colour = ["generic-color-negative", "generic-azure", "generic-redscale", "kodak-colorplus", "harman-phoenix", "mira-color800"];
        assert!(colour.iter().all(|id| dye_set(id).is_some()), "every colour negative names its dyes");
        let grey = ["generic-black-and-white", "ilford-hp5", "ilford-xp2", "kodak-400tx", "fujifilm-acros"];
        assert!(grey.iter().all(|id| dye_set(id).is_none()), "black and white names none");
        assert_eq!(dye_set("kodak-portra"), Some(DyeSet::KodakPortra400));
        assert_eq!(dye_set("cinestill-800t"), Some(DyeSet::KodakVision3_500T));
        assert_eq!(dye_set("lucky-color200"), Some(DyeSet::GenericC41), "a stock no profile measures takes the generic set");
        assert_eq!(dye_set("fujifilm-velvia50"), None, "a slide's dyes are never unmixed");
        // Twin: a key the catalogue lacks names no set.
        assert_eq!(dye_set("kodak-portra-900"), None);
    }

    #[test]
    fn every_set_but_the_generic_names_its_profile_and_reads_by_its_key() {
        for set in DyeSet::ALL {
            assert_eq!(set.profile().is_some(), !set.is_generic(), "{set:?}");
            assert_eq!(set.stock_name().is_some(), set.profile().is_some(), "{set:?}");
            let key = serde_json::to_string(&set).unwrap();
            assert_eq!(serde_json::from_str::<DyeSet>(&key).unwrap(), set);
        }
        // Twin: a key no set carries is refused, never read as another.
        assert!(serde_json::from_str::<DyeSet>("\"kodak-portra-900\"").is_err());
    }
}
