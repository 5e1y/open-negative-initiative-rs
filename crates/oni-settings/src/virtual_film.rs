//! The virtual film a digital photo is always shot on: the stock and its exposure, with the white and the
//! reference each stock brings, which its negative is read on as a roll's measure is.

use crate::axis::graduation;
use crate::dyes::DyeSet;
use crate::levels::{Levels, LevelsSet};
use crate::modes::ConversionMode;
use crate::white::{self, White};
use serde::{Deserialize, Serialize};

/// The stocks a digital photo can be shot on, by the key a sidecar stores: every colour and black and white negative
/// profiled from its maker's data sheet, Portra 800's pushed developments included. A data-sheet twin's key, which a document before
/// format 60 may store, reads as its stock's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum VirtualStock {
    #[default]
    #[serde(rename = "kodak-portra-800", alias = "kodak-portra-800-datasheet")]
    KodakPortra800,
    #[serde(rename = "kodak-portra-800-push-1", alias = "kodak-portra-800-push-1-datasheet")]
    KodakPortra800Push1,
    #[serde(rename = "kodak-portra-800-push-2", alias = "kodak-portra-800-push-2-datasheet")]
    KodakPortra800Push2,
    #[serde(rename = "kodak-portra-160", alias = "kodak-portra-160-datasheet")]
    KodakPortra160,
    #[serde(rename = "kodak-portra-400", alias = "kodak-portra-400-datasheet")]
    KodakPortra400,
    #[serde(rename = "kodak-ektar-100", alias = "kodak-ektar-100-datasheet")]
    KodakEktar100,
    #[serde(rename = "kodak-gold-200", alias = "kodak-gold-200-datasheet")]
    KodakGold200,
    #[serde(rename = "kodak-ultramax-400", alias = "kodak-ultramax-400-datasheet")]
    KodakUltraMax400,
    #[serde(rename = "kodak-verita-200d", alias = "kodak-verita-200d-datasheet")]
    KodakVerita200D,
    #[serde(rename = "kodak-vision3-50d", alias = "kodak-vision3-50d-datasheet")]
    KodakVision3_50D,
    #[serde(rename = "kodak-vision3-200t", alias = "kodak-vision3-200t-datasheet")]
    KodakVision3_200T,
    #[serde(rename = "kodak-vision3-250d", alias = "kodak-vision3-250d-datasheet")]
    KodakVision3_250D,
    #[serde(rename = "kodak-vision3-500t", alias = "kodak-vision3-500t-datasheet")]
    KodakVision3_500T,
    #[serde(rename = "fujifilm-c200", alias = "fujifilm-c200-datasheet")]
    FujifilmC200,
    #[serde(rename = "fujifilm-pro-400h", alias = "fujifilm-pro-400h-datasheet")]
    FujifilmPro400H,
    #[serde(rename = "fujifilm-xtra-400", alias = "fujifilm-xtra-400-datasheet")]
    FujifilmXtra400,
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
    #[serde(rename = "foma-fomapan-100")]
    FomaFomapan100,
    #[serde(rename = "foma-fomapan-200")]
    FomaFomapan200,
    #[serde(rename = "foma-fomapan-400")]
    FomaFomapan400,
    #[serde(rename = "foma-fomapan-cine-100")]
    FomaFomapanCine100,
    #[serde(rename = "foma-fomapan-cine-400")]
    FomaFomapanCine400,
    #[serde(rename = "foma-ortho-400")]
    FomaOrtho400,
    #[serde(rename = "foma-cine-ortho-400")]
    FomaCineOrtho400,
    #[serde(rename = "foma-retropan-320")]
    FomaRetropan320,
    #[serde(rename = "fujifilm-acros-ii")]
    FujifilmAcrosII,
    #[serde(rename = "fujifilm-neopan-400")]
    FujifilmNeopan400,
    #[serde(rename = "fujifilm-neopan-1600")]
    FujifilmNeopan1600,
    #[serde(rename = "fujifilm-neopan-ortho")]
    FujifilmNeopanOrtho,
    #[serde(rename = "ilford-delta-100")]
    IlfordDelta100,
    #[serde(rename = "ilford-delta-400")]
    IlfordDelta400,
    #[serde(rename = "ilford-delta-3200")]
    IlfordDelta3200,
    #[serde(rename = "ilford-fp4-plus")]
    IlfordFP4Plus,
    #[serde(rename = "ilford-hp5-plus")]
    IlfordHP5Plus,
    #[serde(rename = "ilford-ortho-plus")]
    IlfordOrthoPlus,
    #[serde(rename = "ilford-pan-f-plus")]
    IlfordPanFPlus,
    #[serde(rename = "ilford-sfx-200")]
    IlfordSFX200,
    #[serde(rename = "ilford-xp2-super")]
    IlfordXP2Super,
    #[serde(rename = "kodak-double-x")]
    KodakDoubleX,
    #[serde(rename = "kodak-tmax-100")]
    KodakTMax100,
    #[serde(rename = "kodak-tmax-400")]
    KodakTMax400,
    #[serde(rename = "kodak-tmax-p3200")]
    KodakTMaxP3200,
    #[serde(rename = "kodak-tri-x-320")]
    KodakTriX320,
    #[serde(rename = "kodak-tri-x-400")]
    KodakTriX400,
}

/// The light a stock is balanced for, its profile's reference illuminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Balance {
    /// D55, the 5500 K daylight.
    Daylight,
    /// NIST's incandescent lamp, a tungsten sheet's "T".
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
    pub const ALL: [VirtualStock; 75] = [
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
        VirtualStock::FomaFomapan100,
        VirtualStock::FomaFomapan200,
        VirtualStock::FomaFomapan400,
        VirtualStock::FomaFomapanCine100,
        VirtualStock::FomaFomapanCine400,
        VirtualStock::FomaOrtho400,
        VirtualStock::FomaCineOrtho400,
        VirtualStock::FomaRetropan320,
        VirtualStock::FujifilmAcrosII,
        VirtualStock::FujifilmNeopan400,
        VirtualStock::FujifilmNeopan1600,
        VirtualStock::FujifilmNeopanOrtho,
        VirtualStock::IlfordDelta100,
        VirtualStock::IlfordDelta400,
        VirtualStock::IlfordDelta3200,
        VirtualStock::IlfordFP4Plus,
        VirtualStock::IlfordHP5Plus,
        VirtualStock::IlfordOrthoPlus,
        VirtualStock::IlfordPanFPlus,
        VirtualStock::IlfordSFX200,
        VirtualStock::IlfordXP2Super,
        VirtualStock::KodakDoubleX,
        VirtualStock::KodakTMax100,
        VirtualStock::KodakTMax400,
        VirtualStock::KodakTMaxP3200,
        VirtualStock::KodakTriX320,
        VirtualStock::KodakTriX400,
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

    /// The scanned stock it is, which a roll shot on it names.
    pub fn dye_set(self) -> DyeSet {
        self.entry().dyes
    }

    /// Its profile's file, written from the stock's data sheet.
    pub fn profile(self) -> &'static str {
        self.entry().profile
    }

    pub fn name(self) -> &'static str {
        self.entry().name
    }

    /// A black and white film, its negative silver alone: its catalogue stock is one.
    pub fn is_silver(self) -> bool {
        crate::film::film_stock(Some(self.catalogue_id())).is_some_and(|k| k.kind == crate::film::FilmType::BlackAndWhite)
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
