//! The virtual film a digital photo is always shot on: the stock and its exposure, with the white and the
//! reference each stock brings, which its negative is read on as a roll's measure is.

use crate::axis::graduation;
use crate::dyes::DyeSet;
use crate::levels::{Levels, LevelsSet};
use crate::modes::ConversionMode;
use crate::stocks::VirtualEntry;
use crate::white::{self, White};
use serde::{Deserialize, Serialize};

crate::stocks::keyed!(
    /// A stock a digital photo can be shot on, by the key a sidecar stores: every colour and black and white negative
    /// whose file holds its data sheet's profile (`crate::stocks`). A key no file defines is kept as stored, so a file
    /// brought back finds its photos, and renders on the default stock meanwhile.
    VirtualStock, Virtual, "kodak-portra-800"
);

/// The light a stock is balanced for, its profile's reference illuminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

#[cfg(test)]
mod tests;

impl VirtualStock {
    /// Every stock a digital photo can be shot on, in the menus' order.
    pub fn all() -> Vec<VirtualStock> {
        crate::stocks::current().virtuals().iter().map(|v| v.stock).collect()
    }

    /// Its file's entry; a key no file defines reads the default stock's.
    pub fn entry(self) -> VirtualEntry {
        let catalogue = crate::stocks::current();
        catalogue.virtual_film(self).or_else(|| catalogue.virtual_film(VirtualStock::default())).cloned().expect("the default stock is shipped")
    }

    /// A file defines it.
    pub fn is_known(self) -> bool {
        crate::stocks::current().virtual_film(self).is_some()
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

    /// The stock's name; a key no file defines is named by itself.
    pub fn name(self) -> &'static str {
        if self.is_known() { self.entry().name } else { self.key() }
    }

    /// A black and white film, its negative silver alone.
    pub fn is_silver(self) -> bool {
        self.entry().silver
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
