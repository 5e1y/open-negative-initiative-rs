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
        entry("Fujifilm C200", "fujifilm-c200-datasheet", DyeSet::FujifilmC200Datasheet, "datasheet/fujifilm_c200", Daylight, reference(
            [-0.83061093, 0.0, 0.9496308], [0.27427864, 0.27421215, 0.2740708], [0.7100866, 0.80541384, 0.82627577])),
        entry("Fujifilm Pro 400H", "fujifilm-pro400h-datasheet", DyeSet::FujifilmPro400HDatasheet, "datasheet/fujifilm_pro_400h", Daylight, reference(
            [-1.1394027, 0.0, 1.0356495], [0.305082, 0.30497682, 0.30477485], [0.7237952, 0.7966877, 0.8200742])),
        entry("Fujifilm Superia X-TRA 400", "fujifilm-superia-xtra400-datasheet", DyeSet::FujifilmXtra400Datasheet, "datasheet/fujifilm_superia_x_tra_400", Daylight, reference(
            [-0.782707, 0.0, 0.9280978], [0.2685544, 0.26849353, 0.26836243], [0.7469467, 0.85469705, 0.83810484])),
        entry("Kodak Ektar 100", "kodak-ektar100-datasheet", DyeSet::KodakEktar100Datasheet, "datasheet/kodak_ektar_100", Daylight, reference(
            [-0.8861552, 0.0, 0.77332544], [0.27716777, 0.27709648, 0.27698642], [0.675184, 0.7815031, 0.93147725])),
        entry("Kodak Gold 200", "kodak-gold200-datasheet", DyeSet::KodakGold200Datasheet, "datasheet/kodak_gold_200", Daylight, reference(
            [-0.830452, 0.0, 1.0133026], [0.2786753, 0.27860653, 0.27844664], [0.64087546, 0.743152, 0.79781675])),
        entry("Kodak Portra 160", "kodak-portra160-datasheet", DyeSet::KodakPortra160Datasheet, "datasheet/kodak_portra_160", Daylight, reference(
            [-1.0291739, 0.0, 0.7729276], [0.27216747, 0.2720913, 0.27198547], [0.6228159, 0.72310346, 0.8541058])),
        entry("Kodak Portra 400", "kodak-portra400-datasheet", DyeSet::KodakPortra400Datasheet, "datasheet/kodak_portra_400", Daylight, reference(
            [-0.98764074, 0.0, 0.75364494], [0.2759493, 0.27587304, 0.27576756], [0.65476054, 0.75495756, 0.89623237])),
        entry("Kodak Portra 800", "kodak-portra800-datasheet", DyeSet::KodakPortra800Datasheet, "datasheet/kodak_portra_800", Daylight, reference(
            [-0.8467025, 0.0, 1.1942314], [0.2664324, 0.26636896, 0.266185], [0.60928464, 0.7152737, 0.78298473])),
        entry("Kodak Portra 800 (Push 1)", "kodak-portra800-push1-datasheet", DyeSet::KodakPortra800Datasheet, "datasheet/kodak_portra_800_push1", Daylight, reference(
            [-0.8467025, 0.0, 1.1942314], [0.2664324, 0.26636896, 0.266185], [0.63157296, 0.72760016, 0.7819255])),
        entry("Kodak Portra 800 (Push 2)", "kodak-portra800-push2-datasheet", DyeSet::KodakPortra800Datasheet, "datasheet/kodak_portra_800_push2", Daylight, reference(
            [-0.8467025, 0.0, 1.1942314], [0.2664324, 0.26636896, 0.266185], [0.56868476, 0.6586961, 0.68465805])),
        entry("Kodak UltraMax 400", "kodak-ultramax400-datasheet", DyeSet::KodakUltraMax400Datasheet, "datasheet/kodak_ultra_max_400", Daylight, reference(
            [-0.82510906, 0.0, 1.1587664], [0.26436394, 0.26430267, 0.26412937], [0.6067583, 0.71256006, 0.7782918])),
        entry("Kodak Verita 200D", "kodak-verita200d-datasheet", DyeSet::KodakVerita200DDatasheet, "datasheet/kodak_verita_200d", Daylight, reference(
            [-1.0199594, 0.0, 0.9399972], [0.29461864, 0.29452857, 0.29436558], [0.71227753, 0.7734161, 0.8850033])),
        entry("Kodak Vision3 50D", "kodak-vision3-50d-datasheet", DyeSet::KodakVision3_50DDatasheet, "datasheet/kodak_vision3_50d", Daylight, reference(
            [-0.91636825, 0.0, 0.94096464], [0.25927153, 0.2592079, 0.25908357], [0.636564, 0.72591823, 0.7465085])),
        entry("Kodak Vision3 200T", "kodak-vision3-200t-datasheet", DyeSet::KodakVision3_200TDatasheet, "datasheet/kodak_vision3_200t", Tungsten, reference(
            [-0.82578504, 0.0, 0.89388233], [0.26431274, 0.2642514, 0.26413083], [0.62405646, 0.7236301, 0.7633354])),
        entry("Kodak Vision3 250D", "kodak-vision3-250d-datasheet", DyeSet::KodakVision3_250DDatasheet, "datasheet/kodak_vision3_250d", Daylight, reference(
            [-0.7535622, 0.0, 1.0177397], [0.2626649, 0.26260838, 0.2624662], [0.6228944, 0.7168755, 0.7307595])),
        entry("Kodak Vision3 500T", "kodak-vision3-500t-datasheet", DyeSet::KodakVision3_500TDatasheet, "datasheet/kodak_vision3_500t", Tungsten, reference(
            [-0.77878654, 0.0, 0.9240054], [0.2656662, 0.2656069, 0.26547948], [0.61609286, 0.72874504, 0.70622176])),
        entry("Fujifilm 160NC", "fujifilm-160nc", DyeSet::Fujifilm160NC, "datasheet/fujifilm_160nc", Daylight, reference(
            [-0.9830299, 0.0, 0.9446567], [0.27338138, 0.27330685, 0.2731675], [0.70459193, 0.8207795, 1.0357051])),
        entry("Fujifilm 160NL", "fujifilm-160nl", DyeSet::Fujifilm160NL, "datasheet/fujifilm_160nl", Daylight, reference(
            [-1.0258617, 0.0, 0.88072264], [0.2809332, 0.28085187, 0.28071746], [0.66692007, 0.7865639, 0.9021461])),
        entry("Fujifilm 160NPC", "fujifilm-160npc", DyeSet::Fujifilm160NPC, "datasheet/fujifilm_160npc", Daylight, reference(
            [-0.69356275, 0.0, 0.88195366], [0.25589237, 0.25584206, 0.25573105], [0.70791245, 0.78549016, 0.7873607])),
        entry("Fujifilm 160NS", "fujifilm-160ns", DyeSet::Fujifilm160NS, "datasheet/fujifilm_160ns", Daylight, reference(
            [-1.1013058, 0.0, 0.6771016], [0.2900523, 0.28996074, 0.2898581], [0.69712126, 0.8211872, 0.9409702])),
        entry("Fujifilm 200", "fujifilm-200", DyeSet::Fujifilm200, "datasheet/fujifilm_200", Daylight, reference(
            [-0.98764145, 0.0, 1.3207269], [0.26841062, 0.26833865, 0.2681215], [0.57176256, 0.7251559, 0.8275577])),
        entry("Fujifilm 400", "fujifilm-400", DyeSet::Fujifilm400, "datasheet/fujifilm_400", Daylight, reference(
            [-0.98744607, 0.0, 1.3159344], [0.2693434, 0.2692709, 0.26905343], [0.60454845, 0.7682002, 0.8620297])),
        entry("Fujifilm Fujicolor 100", "fujifilm-fujicolor100", DyeSet::FujifilmFujicolor100, "datasheet/fujifilm_fujicolor_100", Daylight, reference(
            [-0.8553253, 0.0, 0.5461921], [0.26282758, 0.2627654, 0.26270142], [0.7540529, 0.8629871, 0.9425986])),
        entry("Fujifilm Natura 1600", "fujifilm-natura1600", DyeSet::FujifilmNatura1600, "datasheet/fujifilm_natura_1600", Daylight, reference(
            [-0.89495873, 0.0, 0.9772441], [0.2745942, 0.2745238, 0.27437645], [0.7150767, 0.8429984, 0.8390635])),
        entry("Fujifilm New Pro 400", "fujifilm-new-pro400", DyeSet::FujifilmNewPro400, "datasheet/fujifilm_new_pro_400", Daylight, reference(
            [-0.9659846, 0.0, 0.82500136], [0.2955389, 0.29545158, 0.29531366], [0.72537076, 0.7859453, 0.85675734])),
        entry("Fujifilm Nexia 400", "fujifilm-nexia400", DyeSet::FujifilmNexia400, "datasheet/fujifilm_nexia_400", Daylight, reference(
            [-1.1186444, 0.0, 1.0489243], [0.35295036, 0.35280007, 0.35250288], [0.7807977, 0.8590638, 1.0610714])),
        entry("Fujifilm Nexia A200", "fujifilm-nexia-a200", DyeSet::FujifilmNexiaA200, "datasheet/fujifilm_nexia_a200", Daylight, reference(
            [-1.0643239, 0.0, 1.2049339], [0.36077076, 0.36061642, 0.36023104], [0.8076468, 0.892673, 0.9828505])),
        entry("Fujifilm Nexia Zoom Master 800", "fujifilm-nexia-zoom-master800", DyeSet::FujifilmNexiaZoomMaster800, "datasheet/fujifilm_nexia_zoom_master_800", Daylight, reference(
            [-1.060208, 0.0, 1.2241347], [0.3226104, 0.3224957, 0.3222016], [0.74215615, 0.8154189, 0.89253145])),
        entry("Fujifilm Pro 160C", "fujifilm-pro160c", DyeSet::FujifilmPro160C, "datasheet/fujifilm_pro_160c", Daylight, reference(
            [-1.3470296, 0.0, 0.6656624], [0.3497056, 0.3495408, 0.34938172], [0.757183, 0.84764206, 0.87450624])),
        entry("Fujifilm Pro 160NC", "fujifilm-pro160nc", DyeSet::FujifilmPro160NC, "datasheet/fujifilm_pro_160nc", Daylight, reference(
            [-1.2484103, 0.0, 0.5905031], [0.33505836, 0.33491787, 0.33479527], [0.74852407, 0.83184606, 0.8884373])),
        entry("Fujifilm Pro 160NH", "fujifilm-pro160nh", DyeSet::FujifilmPro160NH, "datasheet/fujifilm_pro_160nh", Daylight, reference(
            [-1.3468778, 0.0, 0.6698033], [0.35126054, 0.3510937, 0.3509315], [0.7790135, 0.8370369, 0.86286545])),
        entry("Fujifilm Pro 160NS", "fujifilm-pro160ns", DyeSet::FujifilmPro160NS, "datasheet/fujifilm_pro_160ns", Daylight, reference(
            [-1.2484103, 0.0, 0.5905031], [0.33505836, 0.33491787, 0.33479527], [0.7229442, 0.77923137, 0.83645236])),
        entry("Fujifilm Pro 160S", "fujifilm-pro160s", DyeSet::FujifilmPro160S, "datasheet/fujifilm_pro_160s", Daylight, reference(
            [-1.2484833, 0.0, 0.59052044], [0.33505875, 0.33491823, 0.33479562], [0.71236014, 0.78729, 0.8013349])),
        entry("Fujifilm Pro 800", "fujifilm-pro800", DyeSet::FujifilmPro800, "datasheet/fujifilm_pro_800", Daylight, reference(
            [-0.99509996, 0.0, 1.2042607], [0.29084924, 0.2907633, 0.2905385], [0.73315144, 0.8231449, 0.8414066])),
        entry("Fujifilm Pro 800Z", "fujifilm-pro800z", DyeSet::FujifilmPro800Z, "datasheet/fujifilm_pro_800z", Daylight, reference(
            [-1.0227036, 0.0, 1.1428204], [0.29216453, 0.292076, 0.29186568], [0.7162864, 0.79871434, 0.8176887])),
        entry("Fujifilm Reala Ace", "fujifilm-reala-ace", DyeSet::FujifilmRealaAce, "datasheet/fujifilm_reala_ace", Daylight, reference(
            [-0.8249654, 0.0, 0.74971867], [0.27130067, 0.271236, 0.27113494], [0.7211622, 0.801655, 0.845042])),
        entry("Fujifilm Superia 100", "fujifilm-superia100", DyeSet::FujifilmSuperia100, "datasheet/fujifilm_superia_100", Daylight, reference(
            [-0.87347025, 0.0, 0.5669083], [0.28469506, 0.28462034, 0.28454116], [0.7076585, 0.8131212, 0.8033495])),
        entry("Fujifilm Superia 200", "fujifilm-superia200", DyeSet::FujifilmSuperia200, "datasheet/fujifilm_superia_200", Daylight, reference(
            [-0.84955186, 0.0, 0.61150295], [0.2812986, 0.2812272, 0.28114265], [0.77946216, 0.8752912, 0.89661235])),
        entry("Fujifilm Superia 1600", "fujifilm-superia1600", DyeSet::FujifilmSuperia1600, "datasheet/fujifilm_superia_1600", Daylight, reference(
            [-0.89496434, 0.0, 0.9771375], [0.2746378, 0.27456737, 0.27442], [0.7151464, 0.8430934, 0.8391708])),
        entry("Fujifilm Superia Premium 400", "fujifilm-superia-premium400", DyeSet::FujifilmSuperiaPremium400, "datasheet/fujifilm_superia_premium_400", Daylight, reference(
            [-0.7838326, 0.0, 0.9296579], [0.26892984, 0.26886877, 0.268737], [0.7731065, 0.87261534, 0.86194336])),
        entry("Fujifilm Superia Reala", "fujifilm-superia-reala", DyeSet::FujifilmSuperiaReala, "datasheet/fujifilm_superia_reala", Daylight, reference(
            [-0.72140634, 0.0, 0.90098035], [0.28289422, 0.2828304, 0.28268972], [0.68116415, 0.80632806, 0.7574748])),
        entry("Fujifilm Superia Venus 400", "fujifilm-superia-venus400", DyeSet::FujifilmSuperiaVenus400, "datasheet/fujifilm_superia_venus_400", Daylight, reference(
            [-0.7705156, 0.0, 0.86905223], [0.26798925, 0.2679294, 0.2678099], [0.7237674, 0.85133123, 0.84807336])),
        entry("Fujifilm Superia Venus 800", "fujifilm-superia-venus800", DyeSet::FujifilmSuperiaVenus800, "datasheet/fujifilm_superia_venus_800", Daylight, reference(
            [-0.8487747, 0.0, 0.8897301], [0.27266905, 0.27260232, 0.2724745], [0.7130627, 0.8329747, 0.85304165])),
        entry("Fujifilm Superia X-TRA 800", "fujifilm-superia-xtra800", DyeSet::FujifilmXtra800, "datasheet/fujifilm_superia_x_tra_800", Daylight, reference(
            [-1.1181284, 0.0, 1.1705339], [0.3584864, 0.35832962, 0.35796675], [0.8199728, 0.94027627, 1.1021227])),
        entry("Kodak Portra 160NC", "kodak-portra160nc", DyeSet::KodakPortra160NC, "datasheet/kodak_portra_160nc", Daylight, reference(
            [-0.55807227, 0.0, 0.43169335], [0.31164914, 0.31158417, 0.3115135], [0.9687787, 1.0826732, 0.97218776])),
        entry("Kodak Portra 160VC", "kodak-portra160vc", DyeSet::KodakPortra160VC, "datasheet/kodak_portra_160vc", Daylight, reference(
            [-0.565028, 0.0, 0.5502797], [0.30715853, 0.30709514, 0.30700427], [1.0041873, 1.1008996, 0.99254084])),
        entry("Kodak Portra 400NC", "kodak-portra400nc", DyeSet::KodakPortra400NC, "datasheet/kodak_portra_400nc", Daylight, reference(
            [-0.5557344, 0.0, 0.5149134], [0.30773997, 0.3076771, 0.3075928], [0.9609211, 1.0723351, 0.9655603])),
        entry("Kodak Portra 400VC", "kodak-portra400vc", DyeSet::KodakPortra400VC, "datasheet/kodak_portra_400vc", Daylight, reference(
            [-0.5713503, 0.0, 0.5229958], [0.30710933, 0.30704534, 0.3069599], [0.98488104, 1.0995827, 0.9914603])),
    ]
};

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
