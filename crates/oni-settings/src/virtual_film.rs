//! A digital photo shot on a virtual film: the stock, its exposure, and the negative's reference the
//! scan of that film is read on, as a roll's reference is.

use crate::axis::graduation;
use crate::levels::{Levels, LevelsSet};
use crate::modes::ConversionMode;
use serde::{Deserialize, Serialize};

/// The stocks a digital photo can be shot on, by the key a sidecar stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VirtualStock {
    #[serde(rename = "kodak-portra-800")]
    KodakPortra800,
}

impl VirtualStock {
    pub const ALL: [VirtualStock; 1] = [VirtualStock::KodakPortra800];

    /// The light the stock is balanced for, its profile's reference: the scene is exposed at it, as the
    /// film sees a scene, and the print balances what it leaves.
    pub fn daylight(self) -> crate::white::White {
        match self {
            // D55, the daylight Kodak balances Portra 800 for (5500 K).
            VirtualStock::KodakPortra800 => crate::white::of_uv(crate::white::uv_of_xy(0.33242, 0.34743)),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            VirtualStock::KodakPortra800 => "Kodak Portra 800",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VirtualFilm {
    pub stock: VirtualStock,
    /// Stops of light the film takes over the scene's own, its grey on the film's speed at zero.
    pub exposure: f32,
    /// The negative's gains in stops, then each channel's black and white as shares of the graduation
    /// window, as a roll's reference holds them; the stock's own derive them, and the hand moves them.
    pub stops: [f32; 3],
    pub black: [f32; 3],
    pub white: [f32; 3],
}

impl VirtualFilm {
    pub const EXPOSURE_RANGE: (f32, f32) = (-4.0, 4.0);
    /// A share of the window either end may reach, as a roll's levels may.
    pub const SHARE_RANGE: (f32, f32) = (0.0, 1.0);

    /// The negative's windows on a negative's axis, each channel's inner handles at rest.
    pub fn levels(&self) -> LevelsSet {
        let at = |c: usize| {
            let value = |f: f32| graduation::value(f, ConversionMode::Negative);
            Levels::with_ends(value(self.black[c]), value(self.white[c]))
        };
        LevelsSet { red: at(0), green: at(1), blue: at(2) }
    }
}
