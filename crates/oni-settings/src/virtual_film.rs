//! The virtual film a digital photo is always shot on: the stock and its exposure, with the white and the
//! reference each stock brings, which its negative is read on as a roll's measure is.

use crate::axis::graduation;
use crate::levels::{Levels, LevelsSet};
use crate::modes::ConversionMode;
use crate::white::{self, White};
use serde::{Deserialize, Serialize};

/// The stocks a digital photo can be shot on, by the key a sidecar stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum VirtualStock {
    #[default]
    #[serde(rename = "kodak-portra-800")]
    KodakPortra800,
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

impl VirtualStock {
    pub const ALL: [VirtualStock; 1] = [VirtualStock::KodakPortra800];

    /// The light the stock is balanced for, its profile's own: the scene is lit at it, as the film sees a
    /// scene, and the print balances what it leaves.
    pub fn white(self) -> White {
        match self {
            // D55, the daylight Kodak balances Portra 800 for (5500 K).
            VirtualStock::KodakPortra800 => white::of_uv(white::uv_of_xy(0.33242, 0.34743)),
        }
    }

    pub fn reference(self) -> StockReference {
        match self {
            VirtualStock::KodakPortra800 => StockReference {
                stops: [-0.844_049_04, 0.0, 1.197_313_4],
                black: [0.266_748_8, 0.266_685_3, 0.266_500_2],
                white: [0.647_680_46, 0.726_868_3, 0.763_056_1],
            },
        }
    }

    /// The stock in the film catalogue, which names its dyes and the roll's icon.
    pub fn catalogue_id(self) -> &'static str {
        match self {
            VirtualStock::KodakPortra800 => "kodak-portra",
        }
    }

    /// The dyes its profile measures, which its negative's separation unmixes.
    pub fn dye_set(self) -> crate::dyes::DyeSet {
        match self {
            VirtualStock::KodakPortra800 => crate::dyes::DyeSet::KodakPortra800,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            VirtualStock::KodakPortra800 => "Kodak Portra 800",
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stocks_reference_lies_on_a_negatives_window_and_names_its_dyes() {
        let r = VirtualStock::KodakPortra800.reference();
        let levels = r.levels();
        assert_eq!(levels.green.black, graduation::value(r.black[1], ConversionMode::Negative));
        assert!(levels.windows().iter().all(|w| w.white > w.black), "{levels:?}");
        assert_eq!(VirtualStock::KodakPortra800.dye_set(), crate::DyeSet::KodakPortra800);
        // Twin: read on a slide's axis the same shares land elsewhere.
        assert_ne!(levels.green.black, graduation::value(r.black[1], ConversionMode::Positive));
    }

    #[test]
    fn a_film_missing_its_exposure_reads_at_the_films_speed_and_a_stock_by_its_key() {
        let f: VirtualFilm = serde_json::from_str(r#"{"stock":"kodak-portra-800"}"#).unwrap();
        assert_eq!(f, VirtualFilm::default());
        // Twin: an unknown stock is refused, never read as another.
        assert!(serde_json::from_str::<VirtualFilm>(r#"{"stock":"kodak-gold-200"}"#).is_err());
    }
}
