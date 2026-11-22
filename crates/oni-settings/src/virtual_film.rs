//! The virtual film a digital photo is always shot on: the stock and its exposure, with the white and the
//! reference each stock brings, which its negative is read on as a roll's measure is.

use crate::axis::graduation;
use crate::dyes::DyeSet;
use crate::levels::{Levels, LevelsSet};
use crate::modes::ConversionMode;
use crate::white::{self, White};
use serde::{Deserialize, Serialize};

/// The stocks a digital photo can be shot on, by the key a sidecar stores: every colour negative spektrafilm
/// profiles, its pushed developments included.
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

/// What a stock is: its name, its place in the film catalogue, its dyes, its profile and its light.
struct Entry {
    name: &'static str,
    catalogue: &'static str,
    dyes: DyeSet,
    profile: &'static str,
    balance: Balance,
    reference: StockReference,
}

const fn entry(name: &'static str, catalogue: &'static str, dyes: DyeSet, profile: &'static str, balance: Balance, reference: StockReference) -> Entry {
    Entry { name, catalogue, dyes, profile, balance, reference }
}

const fn reference(stops: [f32; 3], black: [f32; 3], white: [f32; 3]) -> StockReference {
    StockReference { stops, black, white }
}

/// Every stock in `VirtualStock::ALL`'s order, which is the enum's. A push develops Portra 800's own dyes on its curves.
const ENTRIES: [Entry; VirtualStock::ALL.len()] = {
    use Balance::*;
    [
        entry("Kodak Portra 800", "kodak-portra800", DyeSet::KodakPortra800, "kodak_portra_800", Daylight, reference(
            [-0.84404904, 0.0, 1.1973134], [0.2667488, 0.2666853, 0.2665002], [0.64768046, 0.7268683, 0.7630561])),
        entry("Kodak Portra 800 (Push 1)", "kodak-portra800-push1", DyeSet::KodakPortra800, "kodak_portra_800_push1", Daylight, reference(
            [-0.84404904, 0.0, 1.1973134], [0.2667488, 0.2666853, 0.2665002], [0.6494186, 0.7278812, 0.75367403])),
        entry("Kodak Portra 800 (Push 2)", "kodak-portra800-push2", DyeSet::KodakPortra800, "kodak_portra_800_push2", Daylight, reference(
            [-0.84404904, 0.0, 1.1973134], [0.2667488, 0.2666853, 0.2665002], [0.6495546, 0.7270855, 0.74462855])),
        entry("Kodak Portra 160", "kodak-portra160", DyeSet::KodakPortra160, "kodak_portra_160", Daylight, reference(
            [-1.0279491, 0.0, 0.77555937], [0.2723183, 0.27224207, 0.27213567], [0.61698914, 0.68821293, 0.8399785])),
        entry("Kodak Portra 400", "kodak-portra400", DyeSet::KodakPortra400, "kodak_portra_400", Daylight, reference(
            [-0.98707014, 0.0, 0.7491194], [0.27438924, 0.27431393, 0.2742105], [0.62938607, 0.7078299, 0.8256584])),
        entry("Kodak Ektar 100", "kodak-ektar100", DyeSet::KodakEktar100, "kodak_ektar_100", Daylight, reference(
            [-0.8816751, 0.0, 0.77718276], [0.27767417, 0.27760288, 0.2774917], [0.61034083, 0.6918498, 0.8607544])),
        entry("Kodak Gold 200", "kodak-gold200", DyeSet::KodakGold200, "kodak_gold_200", Daylight, reference(
            [-0.82962483, 0.0, 1.0129256], [0.27911428, 0.2790453, 0.278885], [0.6387663, 0.72390294, 0.81404686])),
        entry("Kodak UltraMax 400", "kodak-ultramax400", DyeSet::KodakUltraMax400, "kodak_ultramax_400", Daylight, reference(
            [-0.82562864, 0.0, 1.1634097], [0.26399082, 0.2639297, 0.26375595], [0.6370392, 0.72782284, 0.77483225])),
        entry("Kodak Verita 200D", "kodak-verita200d", DyeSet::KodakVerita200D, "kodak_verita_200d", Daylight, reference(
            [-1.0170532, 0.0, 0.9371615], [0.29557034, 0.2954798, 0.29531628], [0.7012342, 0.734775, 0.78206813])),
        entry("Kodak Vision3 50D", "kodak-vision3-50d", DyeSet::KodakVision3_50D, "kodak_vision3_50d", Daylight, reference(
            [-0.90168256, 0.0, 0.9253492], [0.2607587, 0.2606951, 0.26057217], [0.6738153, 0.73703754, 0.7914196])),
        entry("Kodak Vision3 200T", "kodak-vision3-200t", DyeSet::KodakVision3_200T, "kodak_vision3_200t", Tungsten, reference(
            [-0.82313, 0.0, 0.88816845], [0.26512843, 0.2650669, 0.26494658], [0.67345315, 0.75740576, 0.80266327])),
        entry("Kodak Vision3 250D", "kodak-vision3-250d", DyeSet::KodakVision3_250D, "kodak_vision3_250d", Daylight, reference(
            [-0.7476403, 0.0, 1.0219592], [0.26277408, 0.26271784, 0.26257473], [0.6620739, 0.7465552, 0.7985428])),
        entry("Kodak Vision3 500T", "kodak-vision3-500t", DyeSet::KodakVision3_500T, "kodak_vision3_500t", Tungsten, reference(
            [-0.7838841, 0.0, 0.92112136], [0.26567653, 0.26561692, 0.26549003], [0.68338394, 0.76721215, 0.7902225])),
        entry("Fujifilm C200", "fujifilm-c200", DyeSet::FujifilmC200, "fujifilm_c200", Daylight, reference(
            [-0.8304623, 0.0, 0.95183784], [0.27410483, 0.27403846, 0.27389687], [0.7544914, 0.755274, 0.69163656])),
        entry("Fujifilm Pro 400H", "fujifilm-pro400h", DyeSet::FujifilmPro400H, "fujifilm_pro_400h", Daylight, reference(
            [-1.1333568, 0.0, 1.0707351], [0.3005907, 0.3004895, 0.300285], [0.69857633, 0.7685451, 0.8476269])),
        entry("Fujifilm Superia X-TRA 400", "fujifilm-superia-xtra400", DyeSet::FujifilmXtra400, "fujifilm_xtra_400", Daylight, reference(
            [-0.76884884, 0.0, 0.8751772], [0.2673272, 0.26726773, 0.26714772], [0.7469927, 0.7451898, 0.6953603])),
    ]
};

impl VirtualStock {
    /// Portra 800 first, the default a digital photo opens on.
    pub const ALL: [VirtualStock; 16] = [
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

    /// Its spektrafilm profile's file, the name spektrafilm gives it.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every stock's reference lies on a negative's windows, and its place in the catalogue is its own precise
    /// stock, of its name and on its dyes.
    #[test]
    fn a_stocks_reference_lies_on_a_negatives_window_and_names_its_dyes() {
        for (i, stock) in VirtualStock::ALL.into_iter().enumerate() {
            assert_eq!(stock as usize, i, "a stock's entry is its place");
            let r = stock.reference();
            let levels = r.levels();
            assert_eq!(levels.green.black, graduation::value(r.black[1], ConversionMode::Negative));
            assert!(levels.windows().iter().all(|w| w.white > w.black), "{stock:?}: {levels:?}");
            let kind = crate::film::film_stock(Some(stock.catalogue_id())).expect("in the catalogue");
            assert_eq!((kind.display_name, crate::dyes::dye_set(kind.id)), (stock.name(), Some(stock.dye_set())), "{stock:?}");
            assert!(!kind.line && kind.offered, "{stock:?}: a precise stock the picker lists, never its line");
            assert!(stock.dye_set().profile().is_some_and(|p| stock.profile().starts_with(p)), "{stock:?} develops its own dyes");
        }
        assert_eq!(VirtualStock::ALL[0], VirtualStock::default(), "Portra 800 opens first");
        let r = VirtualStock::KodakPortra800.reference();
        // Twin: read on a slide's axis the same shares land elsewhere.
        assert_ne!(r.levels().green.black, graduation::value(r.black[1], ConversionMode::Positive));
    }

    /// A tungsten stock is balanced warmer than a daylight one, at about 2800 K.
    #[test]
    fn a_tungsten_stock_stands_at_its_lamp_and_a_daylight_one_at_d55() {
        let kelvin = |s: VirtualStock| s.white().kelvin;
        assert!((kelvin(VirtualStock::KodakPortra800) - 5500.0).abs() < 60.0, "{}", kelvin(VirtualStock::KodakPortra800));
        for stock in [VirtualStock::KodakVision3_200T, VirtualStock::KodakVision3_500T] {
            assert_eq!(stock.balance(), Balance::Tungsten);
            assert!((2700.0..2900.0).contains(&kelvin(stock)), "{stock:?}: {}", kelvin(stock));
        }
        // Twin: a daylight cine stock stands at D55.
        assert_eq!(VirtualStock::KodakVision3_250D.white(), VirtualStock::KodakPortra800.white());
    }

    #[test]
    fn a_film_missing_its_exposure_reads_at_the_films_speed_and_a_stock_by_its_key() {
        let f: VirtualFilm = serde_json::from_str(r#"{"stock":"kodak-portra-800"}"#).unwrap();
        assert_eq!(f, VirtualFilm::default());
        for stock in VirtualStock::ALL {
            let key = serde_json::to_string(&stock).unwrap();
            assert_eq!(serde_json::from_str::<VirtualStock>(&key).unwrap(), stock);
        }
        let gold: VirtualFilm = serde_json::from_str(r#"{"stock":"kodak-gold-200","exposure":0.5}"#).unwrap();
        assert_eq!((gold.stock, gold.exposure), (VirtualStock::KodakGold200, 0.5));
        // Twin: an unknown stock is refused, never read as another.
        assert!(serde_json::from_str::<VirtualFilm>(r#"{"stock":"kodak-gold-900"}"#).is_err());
    }
}
