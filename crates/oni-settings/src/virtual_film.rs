//! The virtual film a digital photo is always shot on: the stock and its exposure, with the white and the
//! reference each stock brings, which its negative is read on as a roll's measure is.

use crate::axis::graduation;
use crate::dyes::DyeSet;
use crate::levels::{Levels, LevelsSet};
use crate::modes::ConversionMode;
use crate::white::{self, White};
use serde::{Deserialize, Serialize};

/// The stocks a digital photo can be shot on, by the key a sidecar stores: every colour negative spektrafilm
/// profiles, its pushed developments included, then every one written from its maker's data sheet, a stock both
/// profile its twin keyed `-datasheet`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum VirtualStock {
    #[default]
    #[serde(rename = "kodak-portra-800")]
    KodakPortra800,
    #[serde(rename = "kodak-portra-800-push-1")]
    KodakPortra800Push1,
    #[serde(rename = "kodak-portra-800-push-2")]
    KodakPortra800Push2,
    #[serde(rename = "kodak-portra-160")]
    KodakPortra160,
    #[serde(rename = "kodak-portra-400")]
    KodakPortra400,
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
    #[serde(rename = "kodak-portra-800-push-1-datasheet")]
    KodakPortra800Push1Datasheet,
    #[serde(rename = "kodak-portra-800-push-2-datasheet")]
    KodakPortra800Push2Datasheet,
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

/// The light a stock is balanced for, its profile's reference illuminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Balance {
    /// D55, the 5500 K daylight.
    Daylight,
    /// spektrafilm's "T", NIST's incandescent lamp.
    Tungsten,
}

impl Balance {
    /// Its CIE 1931 chromaticity, read off its spectrum by the 2° observer.
    pub fn xy(self) -> (f64, f64) {
        match self {
            Balance::Daylight => (0.33242, 0.34743),
            Balance::Tungsten => (0.450_732_6, 0.408_049_6),
        }
    }
}

/// A stock's own reference, as a roll's measure holds one: the negative's gains in stops, then each channel's
/// black and white as shares of the graduation window. Derived from the stock's profile by
/// `oni_pipeline::film`, whose check holds these numbers to the derivation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StockReference {
    pub stops: [f32; 3],
    pub black: [f32; 3],
    pub white: [f32; 3],
}

impl StockReference {
    /// The windows on a negative's axis, each channel's inner handles at rest.
    pub fn levels(&self) -> LevelsSet {
        let at = |c: usize| {
            let value = |f: f32| graduation::value(f, ConversionMode::Negative);
            Levels::with_ends(value(self.black[c]), value(self.white[c]))
        };
        LevelsSet { red: at(0), green: at(1), blue: at(2) }
    }
}

mod entries;
#[cfg(test)]
mod tests;

use entries::{ENTRIES, Entry};

impl VirtualStock {
    /// Portra 800 first, the default a digital photo opens on.
    pub const ALL: [VirtualStock; 64] = [
        VirtualStock::KodakPortra800,
        VirtualStock::KodakPortra800Push1,
        VirtualStock::KodakPortra800Push2,
        VirtualStock::KodakPortra160,
        VirtualStock::KodakPortra400,
        VirtualStock::KodakEktar100,
        VirtualStock::KodakGold200,
        VirtualStock::KodakUltraMax400,
        VirtualStock::KodakVerita200D,
        VirtualStock::KodakVision3_50D,
        VirtualStock::KodakVision3_200T,
        VirtualStock::KodakVision3_250D,
        VirtualStock::KodakVision3_500T,
        VirtualStock::FujifilmC200,
        VirtualStock::FujifilmPro400H,
        VirtualStock::FujifilmXtra400,
        VirtualStock::FujifilmC200Datasheet,
        VirtualStock::FujifilmPro400HDatasheet,
        VirtualStock::FujifilmXtra400Datasheet,
        VirtualStock::KodakEktar100Datasheet,
        VirtualStock::KodakGold200Datasheet,
        VirtualStock::KodakPortra160Datasheet,
        VirtualStock::KodakPortra400Datasheet,
        VirtualStock::KodakPortra800Datasheet,
        VirtualStock::KodakPortra800Push1Datasheet,
        VirtualStock::KodakPortra800Push2Datasheet,
        VirtualStock::KodakUltraMax400Datasheet,
        VirtualStock::KodakVerita200DDatasheet,
        VirtualStock::KodakVision3_50DDatasheet,
        VirtualStock::KodakVision3_200TDatasheet,
        VirtualStock::KodakVision3_250DDatasheet,
        VirtualStock::KodakVision3_500TDatasheet,
        VirtualStock::Fujifilm160NC,
        VirtualStock::Fujifilm160NL,
        VirtualStock::Fujifilm160NPC,
        VirtualStock::Fujifilm160NS,
        VirtualStock::Fujifilm200,
        VirtualStock::Fujifilm400,
        VirtualStock::FujifilmFujicolor100,
        VirtualStock::FujifilmNatura1600,
        VirtualStock::FujifilmNewPro400,
        VirtualStock::FujifilmNexia400,
        VirtualStock::FujifilmNexiaA200,
        VirtualStock::FujifilmNexiaZoomMaster800,
        VirtualStock::FujifilmPro160C,
        VirtualStock::FujifilmPro160NC,
        VirtualStock::FujifilmPro160NH,
        VirtualStock::FujifilmPro160NS,
        VirtualStock::FujifilmPro160S,
        VirtualStock::FujifilmPro800,
        VirtualStock::FujifilmPro800Z,
        VirtualStock::FujifilmRealaAce,
        VirtualStock::FujifilmSuperia100,
        VirtualStock::FujifilmSuperia200,
        VirtualStock::FujifilmSuperia1600,
        VirtualStock::FujifilmSuperiaPremium400,
        VirtualStock::FujifilmSuperiaReala,
        VirtualStock::FujifilmSuperiaVenus400,
        VirtualStock::FujifilmSuperiaVenus800,
        VirtualStock::FujifilmXtra800,
        VirtualStock::KodakPortra160NC,
        VirtualStock::KodakPortra160VC,
        VirtualStock::KodakPortra400NC,
        VirtualStock::KodakPortra400VC,
    ];

    fn entry(self) -> &'static Entry {
        &ENTRIES[self as usize]
    }

    /// The light the stock is balanced for.
    pub fn balance(self) -> Balance {
        self.entry().balance
    }

    /// The light the stock is balanced for, its profile's own: the scene is lit at it, as the film sees a
    /// scene, and the print balances what it leaves.
    pub fn white(self) -> White {
        let (x, y) = self.balance().xy();
        white::of_uv(white::uv_of_xy(x, y))
    }

    pub fn reference(self) -> StockReference {
        self.entry().reference
    }

    /// The stock in the film catalogue, which names its dyes and the roll's icon.
    pub fn catalogue_id(self) -> &'static str {
        self.entry().catalogue
    }

    /// The dyes its profile measures, which its negative's separation unmixes.
    pub fn dye_set(self) -> DyeSet {
        self.entry().dyes
    }

    /// Its profile's file: spektrafilm's name for it, or under `datasheet/` the one written from its data sheet.
    pub fn profile(self) -> &'static str {
        self.entry().profile
    }

    pub fn name(self) -> &'static str {
        self.entry().name
    }
}

/// The film a digital photo is shot on: its stock, and the stops of light it takes over the scene's own.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct VirtualFilm {
    pub stock: VirtualStock,
    /// Its grey on the film's speed at zero.
    #[serde(default)]
    pub exposure: f32,
}

impl VirtualFilm {
    pub const EXPOSURE_RANGE: (f32, f32) = (-4.0, 4.0);
}
