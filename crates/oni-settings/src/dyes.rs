//! The measured dye sets the dye separation unmixes: one per stock profile ONI ships, spektrafilm's or written
//! from a data sheet, and a generic colour-negative set, spektrafilm's mean, for a stock no profile measures.

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

mod stocks;
#[cfg(test)]
mod tests;

pub use stocks::{STOCK_DYE_SETS, dye_set};
