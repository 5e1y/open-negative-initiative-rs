//! The virtual film a digital photo is always shot on: the stock and its exposure, with the white and the
//! reference each stock brings, which its negative is read on as a roll's measure is; a new photo's is the neutral film.

use crate::axis::graduation;
use crate::blocks::{Bloom, Halation};
use crate::dyes::DyeSet;
use crate::levels::{Levels, LevelsSet};
use crate::modes::ConversionMode;
use crate::stocks::VirtualEntry;
use crate::white::{self, White};
use serde::{Deserialize, Serialize};

crate::stocks::keyed!(
    /// A stock a digital photo can be shot on, by the key a sidecar stores: every colour and black and white negative
    /// whose file holds its data sheet's profile (`crate::stocks`). A key no file defines is kept as stored, so a file
    /// brought back finds its photos, and renders on the default stock meanwhile, as a photo storing no key does.
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
    /// No light of its own: the scene as its decode balanced it, a neutral film's.
    Scene,
}

impl Balance {
    /// Its CIE 1931 chromaticity, read off its spectrum by the 2° observer; none where the scene keeps its own.
    pub fn xy(self) -> Option<(f64, f64)> {
        match self {
            Balance::Daylight => Some((0.33242, 0.34743)),
            Balance::Tungsten => Some((0.450_732_6, 0.408_049_6)),
            Balance::Scene => None,
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

/// The rests of a digital photo's film effects on a stock: its halation and its emulsion's blur.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FilmEffects {
    pub halation: Halation,
    pub bloom: Bloom,
}

#[cfg(test)]
mod tests;

impl VirtualEntry {
    /// A digital photo is offered it: a colour film whose file states its couplers, none for a negative the scanner
    /// reads; a black and white or a neutral one, which have none; or a look.
    pub fn offered(&self) -> bool {
        self.silver || self.neutral || self.couplers.is_some() || self.look.is_some()
    }
}

impl VirtualStock {
    /// The film a scanned slide is projected as when its roll names no reversal film whose sheet is profiled: Kodak's
    /// general-purpose slide film, its sheet whole, the one that kept the owner's Velvia slides closest to their former
    /// render.
    pub const SLIDE: VirtualStock = VirtualStock::of("kodak-ektachrome-e100");

    /// The film a new digital photo is shot on: the neutral one, which gives its scene back as its decode holds it.
    pub const NEUTRAL: VirtualStock = VirtualStock::of("neutral");

    /// Every stock a digital photo can be shot on, in the menus' order.
    pub fn all() -> Vec<VirtualStock> {
        crate::stocks::current().virtuals().iter().map(|v| v.stock).collect()
    }

    /// Every stock a data sheet profiles: all but a neutral film, which reads none.
    pub fn sheets() -> Vec<VirtualStock> {
        crate::stocks::current().virtuals().iter().filter(|v| !v.neutral).map(|v| v.stock).collect()
    }

    /// Its file's entry; a key no file defines reads the default stock's.
    pub fn entry(self) -> VirtualEntry {
        let catalogue = crate::stocks::current();
        catalogue.virtual_film(self).or_else(|| catalogue.virtual_film(VirtualStock::default())).cloned().expect("the default stock is shipped")
    }

    /// The rests its file gives a photo's film effects.
    pub fn effects(self) -> FilmEffects {
        self.entry().effects
    }

    /// Its file's entry and the look its `.cube` file gives, where a file defines it with one.
    pub fn look(self) -> Option<(VirtualEntry, crate::stocks::cube::Look)> {
        let entry = crate::stocks::current().virtual_film(self)?.clone();
        let look = entry.look.clone()?;
        Some((entry, look))
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
    /// scene, and the print balances what it leaves; none on a film that keeps the scene's own.
    pub fn white(self) -> Option<White> {
        let (x, y) = self.balance().xy()?;
        Some(white::of_uv(white::uv_of_xy(x, y)))
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
