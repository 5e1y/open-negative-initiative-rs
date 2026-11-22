//! The measured dye sets the dye separation unmixes: one per stock profile ONI ships, spektrafilm's or written
//! from a data sheet, and a generic colour-negative set, spektrafilm's mean, for a stock no profile measures.

use crate::film::FILM_STOCKS;
use serde::{Deserialize, Serialize};

/// A dye set by the key a sidecar stores: its stock's spectral dye densities and base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DyeSet {
    #[default]
    #[serde(rename = "generic-c41")]
    GenericC41,
    #[serde(rename = "kodak-portra-160")]
    KodakPortra160,
    #[serde(rename = "kodak-portra-400")]
    KodakPortra400,
    #[serde(rename = "kodak-portra-800")]
    KodakPortra800,
    #[serde(rename = "kodak-ektar-100")]
    KodakEktar100,
    #[serde(rename = "kodak-gold-200")]
    KodakGold200,
    #[serde(rename = "kodak-ultramax-400")]
    KodakUltraMax400,
    #[serde(rename = "kodak-verita-200d")]
    KodakVerita200D,
    #[serde(rename = "kodak-vision3-50d")]
    KodakVision3_50D,
    #[serde(rename = "kodak-vision3-200t")]
    KodakVision3_200T,
    #[serde(rename = "kodak-vision3-250d")]
    KodakVision3_250D,
    #[serde(rename = "kodak-vision3-500t")]
    KodakVision3_500T,
    #[serde(rename = "fujifilm-c200")]
    FujifilmC200,
    #[serde(rename = "fujifilm-pro-400h")]
    FujifilmPro400H,
    #[serde(rename = "fujifilm-xtra-400")]
    FujifilmXtra400,
    #[serde(rename = "fujifilm-c200-datasheet")]
    FujifilmC200Datasheet,
    #[serde(rename = "fujifilm-pro-400h-datasheet")]
    FujifilmPro400HDatasheet,
    #[serde(rename = "fujifilm-xtra-400-datasheet")]
    FujifilmXtra400Datasheet,
    #[serde(rename = "kodak-ektar-100-datasheet")]
    KodakEktar100Datasheet,
    #[serde(rename = "kodak-gold-200-datasheet")]
    KodakGold200Datasheet,
    #[serde(rename = "kodak-portra-160-datasheet")]
    KodakPortra160Datasheet,
    #[serde(rename = "kodak-portra-400-datasheet")]
    KodakPortra400Datasheet,
    #[serde(rename = "kodak-portra-800-datasheet")]
    KodakPortra800Datasheet,
    #[serde(rename = "kodak-ultramax-400-datasheet")]
    KodakUltraMax400Datasheet,
    #[serde(rename = "kodak-verita-200d-datasheet")]
    KodakVerita200DDatasheet,
    #[serde(rename = "kodak-vision3-50d-datasheet")]
    KodakVision3_50DDatasheet,
    #[serde(rename = "kodak-vision3-200t-datasheet")]
    KodakVision3_200TDatasheet,
    #[serde(rename = "kodak-vision3-250d-datasheet")]
    KodakVision3_250DDatasheet,
    #[serde(rename = "kodak-vision3-500t-datasheet")]
    KodakVision3_500TDatasheet,
    #[serde(rename = "fujifilm-160nc")]
    Fujifilm160NC,
    #[serde(rename = "fujifilm-160nl")]
    Fujifilm160NL,
    #[serde(rename = "fujifilm-160npc")]
    Fujifilm160NPC,
    #[serde(rename = "fujifilm-160ns")]
    Fujifilm160NS,
    #[serde(rename = "fujifilm-200")]
    Fujifilm200,
    #[serde(rename = "fujifilm-400")]
    Fujifilm400,
    #[serde(rename = "fujifilm-fujicolor-100")]
    FujifilmFujicolor100,
    #[serde(rename = "fujifilm-natura-1600")]
    FujifilmNatura1600,
    #[serde(rename = "fujifilm-new-pro-400")]
    FujifilmNewPro400,
    #[serde(rename = "fujifilm-nexia-400")]
    FujifilmNexia400,
    #[serde(rename = "fujifilm-nexia-a200")]
    FujifilmNexiaA200,
    #[serde(rename = "fujifilm-nexia-zoom-master-800")]
    FujifilmNexiaZoomMaster800,
    #[serde(rename = "fujifilm-pro-160c")]
    FujifilmPro160C,
    #[serde(rename = "fujifilm-pro-160nc")]
    FujifilmPro160NC,
    #[serde(rename = "fujifilm-pro-160nh")]
    FujifilmPro160NH,
    #[serde(rename = "fujifilm-pro-160ns")]
    FujifilmPro160NS,
    #[serde(rename = "fujifilm-pro-160s")]
    FujifilmPro160S,
    #[serde(rename = "fujifilm-pro-800")]
    FujifilmPro800,
    #[serde(rename = "fujifilm-pro-800z")]
    FujifilmPro800Z,
    #[serde(rename = "fujifilm-reala-ace")]
    FujifilmRealaAce,
    #[serde(rename = "fujifilm-superia-100")]
    FujifilmSuperia100,
    #[serde(rename = "fujifilm-superia-200")]
    FujifilmSuperia200,
    #[serde(rename = "fujifilm-superia-1600")]
    FujifilmSuperia1600,
    #[serde(rename = "fujifilm-superia-premium-400")]
    FujifilmSuperiaPremium400,
    #[serde(rename = "fujifilm-superia-reala")]
    FujifilmSuperiaReala,
    #[serde(rename = "fujifilm-superia-venus-400")]
    FujifilmSuperiaVenus400,
    #[serde(rename = "fujifilm-superia-venus-800")]
    FujifilmSuperiaVenus800,
    #[serde(rename = "fujifilm-xtra-800")]
    FujifilmXtra800,
    #[serde(rename = "kodak-portra-160nc")]
    KodakPortra160NC,
    #[serde(rename = "kodak-portra-160vc")]
    KodakPortra160VC,
    #[serde(rename = "kodak-portra-400nc")]
    KodakPortra400NC,
    #[serde(rename = "kodak-portra-400vc")]
    KodakPortra400VC,
}

impl DyeSet {
    pub const ALL: [DyeSet; 61] = [
        DyeSet::GenericC41,
        DyeSet::KodakPortra160,
        DyeSet::KodakPortra400,
        DyeSet::KodakPortra800,
        DyeSet::KodakEktar100,
        DyeSet::KodakGold200,
        DyeSet::KodakUltraMax400,
        DyeSet::KodakVerita200D,
        DyeSet::KodakVision3_50D,
        DyeSet::KodakVision3_200T,
        DyeSet::KodakVision3_250D,
        DyeSet::KodakVision3_500T,
        DyeSet::FujifilmC200,
        DyeSet::FujifilmPro400H,
        DyeSet::FujifilmXtra400,
        DyeSet::FujifilmC200Datasheet,
        DyeSet::FujifilmPro400HDatasheet,
        DyeSet::FujifilmXtra400Datasheet,
        DyeSet::KodakEktar100Datasheet,
        DyeSet::KodakGold200Datasheet,
        DyeSet::KodakPortra160Datasheet,
        DyeSet::KodakPortra400Datasheet,
        DyeSet::KodakPortra800Datasheet,
        DyeSet::KodakUltraMax400Datasheet,
        DyeSet::KodakVerita200DDatasheet,
        DyeSet::KodakVision3_50DDatasheet,
        DyeSet::KodakVision3_200TDatasheet,
        DyeSet::KodakVision3_250DDatasheet,
        DyeSet::KodakVision3_500TDatasheet,
        DyeSet::Fujifilm160NC,
        DyeSet::Fujifilm160NL,
        DyeSet::Fujifilm160NPC,
        DyeSet::Fujifilm160NS,
        DyeSet::Fujifilm200,
        DyeSet::Fujifilm400,
        DyeSet::FujifilmFujicolor100,
        DyeSet::FujifilmNatura1600,
        DyeSet::FujifilmNewPro400,
        DyeSet::FujifilmNexia400,
        DyeSet::FujifilmNexiaA200,
        DyeSet::FujifilmNexiaZoomMaster800,
        DyeSet::FujifilmPro160C,
        DyeSet::FujifilmPro160NC,
        DyeSet::FujifilmPro160NH,
        DyeSet::FujifilmPro160NS,
        DyeSet::FujifilmPro160S,
        DyeSet::FujifilmPro800,
        DyeSet::FujifilmPro800Z,
        DyeSet::FujifilmRealaAce,
        DyeSet::FujifilmSuperia100,
        DyeSet::FujifilmSuperia200,
        DyeSet::FujifilmSuperia1600,
        DyeSet::FujifilmSuperiaPremium400,
        DyeSet::FujifilmSuperiaReala,
        DyeSet::FujifilmSuperiaVenus400,
        DyeSet::FujifilmSuperiaVenus800,
        DyeSet::FujifilmXtra800,
        DyeSet::KodakPortra160NC,
        DyeSet::KodakPortra160VC,
        DyeSet::KodakPortra400NC,
        DyeSet::KodakPortra400VC,
    ];

    /// The stock whose profile measured the dyes; the generic set names none.
    pub fn stock_name(self) -> Option<&'static str> {
        Some(match self {
            DyeSet::GenericC41 => return None,
            DyeSet::KodakPortra160 => "Kodak Portra 160",
            DyeSet::KodakPortra400 => "Kodak Portra 400",
            DyeSet::KodakPortra800 => "Kodak Portra 800",
            DyeSet::KodakEktar100 => "Kodak Ektar 100",
            DyeSet::KodakGold200 => "Kodak Gold 200",
            DyeSet::KodakUltraMax400 => "Kodak UltraMax 400",
            DyeSet::KodakVerita200D => "Kodak Verita 200D",
            DyeSet::KodakVision3_50D => "Kodak Vision3 50D",
            DyeSet::KodakVision3_200T => "Kodak Vision3 200T",
            DyeSet::KodakVision3_250D => "Kodak Vision3 250D",
            DyeSet::KodakVision3_500T => "Kodak Vision3 500T",
            DyeSet::FujifilmC200 => "Fujifilm C200",
            DyeSet::FujifilmPro400H => "Fujifilm Pro 400H",
            DyeSet::FujifilmXtra400 => "Fujifilm Superia X-TRA 400",
            DyeSet::FujifilmC200Datasheet => "Fujifilm C200",
            DyeSet::FujifilmPro400HDatasheet => "Fujifilm Pro 400H",
            DyeSet::FujifilmXtra400Datasheet => "Fujifilm Superia X-TRA 400",
            DyeSet::KodakEktar100Datasheet => "Kodak Ektar 100",
            DyeSet::KodakGold200Datasheet => "Kodak Gold 200",
            DyeSet::KodakPortra160Datasheet => "Kodak Portra 160",
            DyeSet::KodakPortra400Datasheet => "Kodak Portra 400",
            DyeSet::KodakPortra800Datasheet => "Kodak Portra 800",
            DyeSet::KodakUltraMax400Datasheet => "Kodak UltraMax 400",
            DyeSet::KodakVerita200DDatasheet => "Kodak Verita 200D",
            DyeSet::KodakVision3_50DDatasheet => "Kodak Vision3 50D",
            DyeSet::KodakVision3_200TDatasheet => "Kodak Vision3 200T",
            DyeSet::KodakVision3_250DDatasheet => "Kodak Vision3 250D",
            DyeSet::KodakVision3_500TDatasheet => "Kodak Vision3 500T",
            DyeSet::Fujifilm160NC => "Fujifilm 160NC",
            DyeSet::Fujifilm160NL => "Fujifilm 160NL",
            DyeSet::Fujifilm160NPC => "Fujifilm 160NPC",
            DyeSet::Fujifilm160NS => "Fujifilm 160NS",
            DyeSet::Fujifilm200 => "Fujifilm 200",
            DyeSet::Fujifilm400 => "Fujifilm 400",
            DyeSet::FujifilmFujicolor100 => "Fujifilm Fujicolor 100",
            DyeSet::FujifilmNatura1600 => "Fujifilm Natura 1600",
            DyeSet::FujifilmNewPro400 => "Fujifilm New Pro 400",
            DyeSet::FujifilmNexia400 => "Fujifilm Nexia 400",
            DyeSet::FujifilmNexiaA200 => "Fujifilm Nexia A200",
            DyeSet::FujifilmNexiaZoomMaster800 => "Fujifilm Nexia Zoom Master 800",
            DyeSet::FujifilmPro160C => "Fujifilm Pro 160C",
            DyeSet::FujifilmPro160NC => "Fujifilm Pro 160NC",
            DyeSet::FujifilmPro160NH => "Fujifilm Pro 160NH",
            DyeSet::FujifilmPro160NS => "Fujifilm Pro 160NS",
            DyeSet::FujifilmPro160S => "Fujifilm Pro 160S",
            DyeSet::FujifilmPro800 => "Fujifilm Pro 800",
            DyeSet::FujifilmPro800Z => "Fujifilm Pro 800Z",
            DyeSet::FujifilmRealaAce => "Fujifilm Reala Ace",
            DyeSet::FujifilmSuperia100 => "Fujifilm Superia 100",
            DyeSet::FujifilmSuperia200 => "Fujifilm Superia 200",
            DyeSet::FujifilmSuperia1600 => "Fujifilm Superia 1600",
            DyeSet::FujifilmSuperiaPremium400 => "Fujifilm Superia Premium 400",
            DyeSet::FujifilmSuperiaReala => "Fujifilm Superia Reala",
            DyeSet::FujifilmSuperiaVenus400 => "Fujifilm Superia Venus 400",
            DyeSet::FujifilmSuperiaVenus800 => "Fujifilm Superia Venus 800",
            DyeSet::FujifilmXtra800 => "Fujifilm Superia X-TRA 800",
            DyeSet::KodakPortra160NC => "Kodak Portra 160NC",
            DyeSet::KodakPortra160VC => "Kodak Portra 160VC",
            DyeSet::KodakPortra400NC => "Kodak Portra 400NC",
            DyeSet::KodakPortra400VC => "Kodak Portra 400VC",
        })
    }

    /// The profile's file: spektrafilm's name for it, or under `datasheet/` the one written from its data sheet.
    pub fn profile(self) -> Option<&'static str> {
        Some(match self {
            DyeSet::GenericC41 => return None,
            DyeSet::KodakPortra160 => "kodak_portra_160",
            DyeSet::KodakPortra400 => "kodak_portra_400",
            DyeSet::KodakPortra800 => "kodak_portra_800",
            DyeSet::KodakEktar100 => "kodak_ektar_100",
            DyeSet::KodakGold200 => "kodak_gold_200",
            DyeSet::KodakUltraMax400 => "kodak_ultramax_400",
            DyeSet::KodakVerita200D => "kodak_verita_200d",
            DyeSet::KodakVision3_50D => "kodak_vision3_50d",
            DyeSet::KodakVision3_200T => "kodak_vision3_200t",
            DyeSet::KodakVision3_250D => "kodak_vision3_250d",
            DyeSet::KodakVision3_500T => "kodak_vision3_500t",
            DyeSet::FujifilmC200 => "fujifilm_c200",
            DyeSet::FujifilmPro400H => "fujifilm_pro_400h",
            DyeSet::FujifilmXtra400 => "fujifilm_xtra_400",
            DyeSet::FujifilmC200Datasheet => "datasheet/fujifilm_c200",
            DyeSet::FujifilmPro400HDatasheet => "datasheet/fujifilm_pro_400h",
            DyeSet::FujifilmXtra400Datasheet => "datasheet/fujifilm_superia_x_tra_400",
            DyeSet::KodakEktar100Datasheet => "datasheet/kodak_ektar_100",
            DyeSet::KodakGold200Datasheet => "datasheet/kodak_gold_200",
            DyeSet::KodakPortra160Datasheet => "datasheet/kodak_portra_160",
            DyeSet::KodakPortra400Datasheet => "datasheet/kodak_portra_400",
            DyeSet::KodakPortra800Datasheet => "datasheet/kodak_portra_800",
            DyeSet::KodakUltraMax400Datasheet => "datasheet/kodak_ultra_max_400",
            DyeSet::KodakVerita200DDatasheet => "datasheet/kodak_verita_200d",
            DyeSet::KodakVision3_50DDatasheet => "datasheet/kodak_vision3_50d",
            DyeSet::KodakVision3_200TDatasheet => "datasheet/kodak_vision3_200t",
            DyeSet::KodakVision3_250DDatasheet => "datasheet/kodak_vision3_250d",
            DyeSet::KodakVision3_500TDatasheet => "datasheet/kodak_vision3_500t",
            DyeSet::Fujifilm160NC => "datasheet/fujifilm_160nc",
            DyeSet::Fujifilm160NL => "datasheet/fujifilm_160nl",
            DyeSet::Fujifilm160NPC => "datasheet/fujifilm_160npc",
            DyeSet::Fujifilm160NS => "datasheet/fujifilm_160ns",
            DyeSet::Fujifilm200 => "datasheet/fujifilm_200",
            DyeSet::Fujifilm400 => "datasheet/fujifilm_400",
            DyeSet::FujifilmFujicolor100 => "datasheet/fujifilm_fujicolor_100",
            DyeSet::FujifilmNatura1600 => "datasheet/fujifilm_natura_1600",
            DyeSet::FujifilmNewPro400 => "datasheet/fujifilm_new_pro_400",
            DyeSet::FujifilmNexia400 => "datasheet/fujifilm_nexia_400",
            DyeSet::FujifilmNexiaA200 => "datasheet/fujifilm_nexia_a200",
            DyeSet::FujifilmNexiaZoomMaster800 => "datasheet/fujifilm_nexia_zoom_master_800",
            DyeSet::FujifilmPro160C => "datasheet/fujifilm_pro_160c",
            DyeSet::FujifilmPro160NC => "datasheet/fujifilm_pro_160nc",
            DyeSet::FujifilmPro160NH => "datasheet/fujifilm_pro_160nh",
            DyeSet::FujifilmPro160NS => "datasheet/fujifilm_pro_160ns",
            DyeSet::FujifilmPro160S => "datasheet/fujifilm_pro_160s",
            DyeSet::FujifilmPro800 => "datasheet/fujifilm_pro_800",
            DyeSet::FujifilmPro800Z => "datasheet/fujifilm_pro_800z",
            DyeSet::FujifilmRealaAce => "datasheet/fujifilm_reala_ace",
            DyeSet::FujifilmSuperia100 => "datasheet/fujifilm_superia_100",
            DyeSet::FujifilmSuperia200 => "datasheet/fujifilm_superia_200",
            DyeSet::FujifilmSuperia1600 => "datasheet/fujifilm_superia_1600",
            DyeSet::FujifilmSuperiaPremium400 => "datasheet/fujifilm_superia_premium_400",
            DyeSet::FujifilmSuperiaReala => "datasheet/fujifilm_superia_reala",
            DyeSet::FujifilmSuperiaVenus400 => "datasheet/fujifilm_superia_venus_400",
            DyeSet::FujifilmSuperiaVenus800 => "datasheet/fujifilm_superia_venus_800",
            DyeSet::FujifilmXtra800 => "datasheet/fujifilm_superia_x_tra_800",
            DyeSet::KodakPortra160NC => "datasheet/kodak_portra_160nc",
            DyeSet::KodakPortra160VC => "datasheet/kodak_portra_160vc",
            DyeSet::KodakPortra400NC => "datasheet/kodak_portra_400nc",
            DyeSet::KodakPortra400VC => "datasheet/kodak_portra_400vc",
        })
    }

    /// No profile measured these dyes: they stand for any colour negative, approximately.
    pub fn is_generic(self) -> bool {
        self == DyeSet::GenericC41
    }
}

/// Each stock's measured dyes, which picking it writes on the roll's frames: its own profile's where ONI ships
/// one, the generic set for a colour negative none measures; none in black and white or on a slide, never unmixed.
/// A line keeps the one profile it stood for, so a roll set on it renders as it did.
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
        ("fujifilm-c200", Some(FujifilmC200)),
        ("fujifilm-c200-datasheet", Some(FujifilmC200Datasheet)),
        ("fujifilm-fujicolor", Some(FujifilmC200)),
        ("fujifilm-fujicolor-pro", Some(FujifilmPro400H)),
        ("fujifilm-pro400h", Some(FujifilmPro400H)),
        ("fujifilm-pro400h-datasheet", Some(FujifilmPro400HDatasheet)),
        ("fujifilm-provia100", None),
        ("fujifilm-superia", Some(FujifilmXtra400)),
        ("fujifilm-superia-xtra400", Some(FujifilmXtra400)),
        ("fujifilm-superia-xtra400-datasheet", Some(FujifilmXtra400Datasheet)),
        ("fujifilm-velvia100", None),
        ("fujifilm-velvia50", None),
        ("fujifilm-160nc", Some(Fujifilm160NC)),
        ("fujifilm-160nl", Some(Fujifilm160NL)),
        ("fujifilm-160npc", Some(Fujifilm160NPC)),
        ("fujifilm-160ns", Some(Fujifilm160NS)),
        ("fujifilm-200", Some(Fujifilm200)),
        ("fujifilm-400", Some(Fujifilm400)),
        ("fujifilm-fujicolor100", Some(FujifilmFujicolor100)),
        ("fujifilm-natura1600", Some(FujifilmNatura1600)),
        ("fujifilm-new-pro400", Some(FujifilmNewPro400)),
        ("fujifilm-nexia400", Some(FujifilmNexia400)),
        ("fujifilm-nexia-a200", Some(FujifilmNexiaA200)),
        ("fujifilm-nexia-zoom-master800", Some(FujifilmNexiaZoomMaster800)),
        ("fujifilm-pro160c", Some(FujifilmPro160C)),
        ("fujifilm-pro160nc", Some(FujifilmPro160NC)),
        ("fujifilm-pro160nh", Some(FujifilmPro160NH)),
        ("fujifilm-pro160ns", Some(FujifilmPro160NS)),
        ("fujifilm-pro160s", Some(FujifilmPro160S)),
        ("fujifilm-pro800", Some(FujifilmPro800)),
        ("fujifilm-pro800z", Some(FujifilmPro800Z)),
        ("fujifilm-reala-ace", Some(FujifilmRealaAce)),
        ("fujifilm-superia100", Some(FujifilmSuperia100)),
        ("fujifilm-superia200", Some(FujifilmSuperia200)),
        ("fujifilm-superia1600", Some(FujifilmSuperia1600)),
        ("fujifilm-superia-premium400", Some(FujifilmSuperiaPremium400)),
        ("fujifilm-superia-reala", Some(FujifilmSuperiaReala)),
        ("fujifilm-superia-venus400", Some(FujifilmSuperiaVenus400)),
        ("fujifilm-superia-venus800", Some(FujifilmSuperiaVenus800)),
        ("fujifilm-superia-xtra800", Some(FujifilmXtra800)),
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
        ("kodak-ektar100", Some(KodakEktar100)),
        ("kodak-ektar100-datasheet", Some(KodakEktar100Datasheet)),
        ("kodak-gold", Some(KodakGold200)),
        ("kodak-gold200", Some(KodakGold200)),
        ("kodak-gold200-datasheet", Some(KodakGold200Datasheet)),
        ("kodak-kodachrome64", None),
        ("kodak-portra", Some(KodakPortra400)),
        ("kodak-portra160", Some(KodakPortra160)),
        ("kodak-portra160-datasheet", Some(KodakPortra160Datasheet)),
        ("kodak-portra400", Some(KodakPortra400)),
        ("kodak-portra400-datasheet", Some(KodakPortra400Datasheet)),
        ("kodak-portra800", Some(KodakPortra800)),
        ("kodak-portra800-datasheet", Some(KodakPortra800Datasheet)),
        // A push develops Portra 800's own dyes: its profile differs in its curves alone.
        ("kodak-portra800-push1", Some(KodakPortra800)),
        ("kodak-portra800-push1-datasheet", Some(KodakPortra800Datasheet)),
        ("kodak-portra800-push2", Some(KodakPortra800)),
        ("kodak-portra800-push2-datasheet", Some(KodakPortra800Datasheet)),
        ("kodak-portra160nc", Some(KodakPortra160NC)),
        ("kodak-portra160vc", Some(KodakPortra160VC)),
        ("kodak-portra400nc", Some(KodakPortra400NC)),
        ("kodak-portra400vc", Some(KodakPortra400VC)),
        ("kodak-ultramax", Some(KodakUltraMax400)),
        ("kodak-ultramax400", Some(KodakUltraMax400)),
        ("kodak-ultramax400-datasheet", Some(KodakUltraMax400Datasheet)),
        ("kodak-verita200d", Some(KodakVerita200D)),
        ("kodak-verita200d-datasheet", Some(KodakVerita200DDatasheet)),
        ("kodak-vision3-50d", Some(KodakVision3_50D)),
        ("kodak-vision3-50d-datasheet", Some(KodakVision3_50DDatasheet)),
        ("kodak-vision3-200t", Some(KodakVision3_200T)),
        ("kodak-vision3-200t-datasheet", Some(KodakVision3_200TDatasheet)),
        ("kodak-vision3-250d", Some(KodakVision3_250D)),
        ("kodak-vision3-250d-datasheet", Some(KodakVision3_250DDatasheet)),
        ("kodak-vision3-500t", Some(KodakVision3_500T)),
        ("kodak-vision3-500t-datasheet", Some(KodakVision3_500TDatasheet)),
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
        use crate::film::FilmType;
        for (kind, (id, set)) in FILM_STOCKS.iter().zip(STOCK_DYE_SETS) {
            assert_eq!(kind.id, id, "the dye sets follow the catalogue's order");
            let negative = matches!(kind.kind, FilmType::ColourNegative | FilmType::CineNegative);
            assert_eq!(set.is_some(), negative, "{id}: a colour negative names its dyes, a slide or black and white none");
        }
        assert_eq!(dye_set("kodak-portra"), Some(DyeSet::KodakPortra400));
        assert_eq!(dye_set("cinestill-800t"), Some(DyeSet::KodakVision3_500T));
        assert_eq!(dye_set("lucky-color200"), Some(DyeSet::GenericC41), "a stock no profile measures takes the generic set");
        assert_eq!(dye_set("fujifilm-velvia50"), None, "a slide's dyes are never unmixed");
        // Twin: a key the catalogue lacks names no set.
        assert_eq!(dye_set("kodak-portra-900"), None);
    }

    /// Each precise stock separates on the profile of its own name, each profile reaches a stock the picker lists,
    /// and a line keeps the one profile it stood for.
    #[test]
    fn a_stock_separates_on_its_own_profile_and_a_line_on_the_one_it_stood_for() {
        for kind in FILM_STOCKS.iter().filter(|k| k.offered && !k.id.starts_with("cinestill")) {
            if let Some(set) = dye_set(kind.id).filter(|s| !s.is_generic()) {
                // A push is its stock developed longer, on the stock's own dyes.
                let stock = kind.display_name.split(" (Push").next();
                assert_eq!(set.stock_name(), stock, "{}", kind.id);
            }
        }
        assert_eq!(dye_set("kodak-portra800-push2"), Some(DyeSet::KodakPortra800));
        for set in DyeSet::ALL.into_iter().filter(|s| !s.is_generic()) {
            let named = FILM_STOCKS.iter().filter(|k| k.offered && !k.line && dye_set(k.id) == Some(set)).count();
            assert!(named >= 1, "{set:?}: no stock the picker lists separates on it");
        }
        let lines = [
            ("kodak-portra", DyeSet::KodakPortra400),
            ("kodak-gold", DyeSet::KodakGold200),
            ("kodak-ultramax", DyeSet::KodakUltraMax400),
            ("fujifilm-fujicolor", DyeSet::FujifilmC200),
            ("fujifilm-fujicolor-pro", DyeSet::FujifilmPro400H),
            ("fujifilm-superia", DyeSet::FujifilmXtra400),
        ];
        for (id, set) in lines {
            assert_eq!(dye_set(id), Some(set), "{id} renders as it did");
            assert!(crate::film::film_stock(Some(id)).is_some_and(|k| k.line && !k.offered), "{id} is a line, no longer listed");
        }
        assert_eq!(FILM_STOCKS.iter().filter(|k| k.line).count(), lines.len(), "every line is held to the set it stood for");
        // Twin: two speeds of one line separate on two profiles.
        assert_ne!(dye_set("kodak-portra160"), dye_set("kodak-portra800"));
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
