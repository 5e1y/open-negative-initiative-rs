//! A digital photo before format 60 stood on spektrafilm's reference of its stock: it moves onto the data sheet's, every
//! hand's move kept (owner's call); a twin's key stood there already, and every other key reads its data sheet by itself.

use super::chain::DATASHEET_VERSION;
use crate::PipelineSettings;
use crate::levels::Levels;
use crate::virtual_film::{StockReference, VirtualStock};
use serde_json::Value;

/// Within a float's round trip through the sidecar: a value laid from the former reference lands on the new one exactly.
const SAME: f32 = 1e-6;

const fn reference(stops: [f32; 3], black: [f32; 3], white: [f32; 3]) -> StockReference {
    StockReference { stops, black, white }
}

/// Each stock spektrafilm profiled, with the reference formats 54 to 59 derived from that profile.
const SPEKTRAFILM: [(VirtualStock, StockReference); 16] = {
    use VirtualStock::*;
    [
        (KodakPortra800, reference([-0.84404904, 0.0, 1.1973134], [0.2667488, 0.2666853, 0.2665002], [0.64768046, 0.7268683, 0.7630561])),
        (KodakPortra800Push1, reference([-0.84404904, 0.0, 1.1973134], [0.2667488, 0.2666853, 0.2665002], [0.6494186, 0.7278812, 0.75367403])),
        (KodakPortra800Push2, reference([-0.84404904, 0.0, 1.1973134], [0.2667488, 0.2666853, 0.2665002], [0.6495546, 0.7270855, 0.74462855])),
        (KodakPortra160, reference([-1.0279491, 0.0, 0.77555937], [0.2723183, 0.27224207, 0.27213567], [0.61698914, 0.68821293, 0.8399785])),
        (KodakPortra400, reference([-0.98707014, 0.0, 0.7491194], [0.27438924, 0.27431393, 0.2742105], [0.62938607, 0.7078299, 0.8256584])),
        (KodakEktar100, reference([-0.8816751, 0.0, 0.77718276], [0.27767417, 0.27760288, 0.2774917], [0.61034083, 0.6918498, 0.8607544])),
        (KodakGold200, reference([-0.82962483, 0.0, 1.0129256], [0.27911428, 0.2790453, 0.278885], [0.6387663, 0.72390294, 0.81404686])),
        (KodakUltraMax400, reference([-0.82562864, 0.0, 1.1634097], [0.26399082, 0.2639297, 0.26375595], [0.6370392, 0.72782284, 0.77483225])),
        (KodakVerita200D, reference([-1.0170532, 0.0, 0.9371615], [0.29557034, 0.2954798, 0.29531628], [0.7012342, 0.734775, 0.78206813])),
        (KodakVision3_50D, reference([-0.90168256, 0.0, 0.9253492], [0.2607587, 0.2606951, 0.26057217], [0.6738153, 0.73703754, 0.7914196])),
        (KodakVision3_200T, reference([-0.82313, 0.0, 0.88816845], [0.26512843, 0.2650669, 0.26494658], [0.67345315, 0.75740576, 0.80266327])),
        (KodakVision3_250D, reference([-0.7476403, 0.0, 1.0219592], [0.26277408, 0.26271784, 0.26257473], [0.6620739, 0.7465552, 0.7985428])),
        (KodakVision3_500T, reference([-0.7838841, 0.0, 0.92112136], [0.26567653, 0.26561692, 0.26549003], [0.68338394, 0.76721215, 0.7902225])),
        (FujifilmC200, reference([-0.8304623, 0.0, 0.95183784], [0.27410483, 0.27403846, 0.27389687], [0.7544914, 0.755274, 0.69163656])),
        (FujifilmPro400H, reference([-1.1333568, 0.0, 1.0707351], [0.3005907, 0.3004895, 0.300285], [0.69857633, 0.7685451, 0.8476269])),
        (FujifilmXtra400, reference([-0.76884884, 0.0, 0.8751772], [0.2673272, 0.26726773, 0.26714772], [0.7469927, 0.7451898, 0.6953603])),
    ]
};

/// The reference a document before format 60 laid for `stock`: spektrafilm's where it profiled the stock, else its own.
pub fn former(stock: VirtualStock) -> StockReference {
    SPEKTRAFILM.iter().find(|(s, _)| *s == stock).map_or_else(|| stock.reference(), |(_, r)| *r)
}

/// `settings` shot on `stock` as a document before format 60 placed it, on its former reference.
pub fn on_former(settings: &PipelineSettings, stock: VirtualStock) -> PipelineSettings {
    let mut out = settings.on_stock(stock);
    out.gains.stops = former(stock).stops;
    out.levels = former(stock).levels();
    out
}

/// `value` laid on `was` moved onto `now`, the hand's distance from it kept.
fn moved(value: f32, was: f32, now: f32) -> f32 {
    if (value - was).abs() < SAME { now } else { value + now - was }
}

/// `window` laid on `was` moved onto `now`, each end on its own.
fn moved_window(window: Levels, was: Levels, now: Levels) -> Levels {
    Levels { black: moved(window.black, was.black, now.black), white: moved(window.white, was.white, now.white), ..window }
}

/// `settings` of `version`, read from the stored `raw` settings, on its stock's data-sheet reference.
pub fn caught_up(version: i64, settings: PipelineSettings, raw: Option<&Value>) -> PipelineSettings {
    if version >= DATASHEET_VERSION {
        return settings;
    }
    let Some(film) = settings.film() else { return settings };
    let key = raw.and_then(|r| r.get("virtualFilm")).and_then(|f| f.get("stock")).and_then(Value::as_str);
    if key.is_some_and(|k| k.ends_with("-datasheet")) {
        return settings;
    }
    let (was, now) = (former(film.stock), film.stock.reference());
    let mut out = settings;
    out.gains.stops = std::array::from_fn(|c| moved(out.gains.stops[c], was.stops[c], now.stops[c]));
    let ([red, green, blue], [was_red, was_green, was_blue], [now_red, now_green, now_blue]) =
        (out.levels.windows(), was.levels().windows(), now.levels().windows());
    out.levels.red = moved_window(red, was_red, now_red);
    out.levels.green = moved_window(green, was_green, now_green);
    out.levels.blue = moved_window(blue, was_blue, now_blue);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::VERSION;
    use crate::{ConversionMode, DyeSet, PaperStock};
    use serde_json::json;

    fn stored(s: &PipelineSettings, version: i64, key: Option<&str>) -> Vec<u8> {
        let mut stored = super::super::reshape::as_stored(&crate::Document::of(s.clone()), version);
        if let Some(key) = key {
            stored["settings"]["virtualFilm"]["stock"] = json!(key);
        }
        serde_json::to_vec(&stored).unwrap()
    }

    fn read(bytes: &[u8]) -> PipelineSettings {
        super::super::document(bytes).unwrap().document.settings
    }

    fn photo(stock: VirtualStock) -> PipelineSettings {
        on_former(&PipelineSettings::rest(ConversionMode::Digital, true), stock)
    }

    #[test]
    fn a_digital_photo_on_spektrafilms_reference_lands_on_the_data_sheets_its_hand_kept() {
        let older = DATASHEET_VERSION - 1;
        let gold = photo(VirtualStock::KodakGold200);
        let landed = read(&stored(&gold, older, None));
        let own = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(VirtualStock::KodakGold200);
        assert_eq!((landed.gains, landed.levels, landed.dye_set), (own.gains, own.levels, own.dye_set), "a reference laid whole lands whole");
        assert_eq!(PipelineSettings { gains: gold.gains, levels: gold.levels, ..landed.clone() }, gold, "nothing else moves");
        let mut hand = gold.clone();
        (hand.gains.stops[2], hand.levels.red.white) = (hand.gains.stops[2] + 0.25, hand.levels.red.white - 0.1);
        let kept = read(&stored(&hand, older, None));
        assert!((kept.gains.stops[2] - own.gains.stops[2] - 0.25).abs() < 1e-6, "{:?}", kept.gains);
        assert!((kept.levels.red.white - own.levels.red.white + 0.1).abs() < 1e-6, "{:?}", kept.levels.red);
        assert_eq!(kept.levels.red.black, own.levels.red.black);
        // Twins: the gains really moved, a twin's key stood on the data sheet already, a stock spektrafilm never
        // profiled and a file of this format stay as they are.
        assert_ne!(gold.gains, own.gains);
        let twin = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(VirtualStock::KodakPortra800);
        assert_eq!(read(&stored(&twin, older, Some("kodak-portra-800-datasheet"))), twin);
        let reala = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(VirtualStock::FujifilmSuperiaReala);
        assert_eq!(read(&stored(&reala, older, None)), reala);
        assert_eq!(read(&stored(&gold, VERSION, None)), gold);
    }

    #[test]
    fn a_document_through_every_step_lands_once_on_the_data_sheets_reference() {
        // A photo before its film's own gains, on another stock's spektrafilm reference: moved onto its own, then onto
        // its data sheet's, never twice.
        let mut laid = photo(VirtualStock::KodakVerita200D);
        (laid.gains.stops, laid.dye_set) = (former(VirtualStock::KodakVision3_200T).stops, DyeSet::KodakVision3_200T);
        let landed = read(&stored(&laid, super::super::chain::OWN_FILM_VERSION - 1, None));
        assert_eq!(landed.gains.stops, VirtualStock::KodakVerita200D.reference().stops);
        // Twin: its spektrafilm reference is another.
        assert_ne!(landed.gains.stops, former(VirtualStock::KodakVerita200D).stops);
    }

    #[test]
    fn every_retired_key_reads_as_its_data_sheet_stock_and_paper() {
        let mut s = PipelineSettings::rest(ConversionMode::Digital, true);
        s.print_paper = Some(PaperStock::KodakEnduraPremier);
        let mut stored: Value = serde_json::from_slice(&stored(&s, DATASHEET_VERSION - 1, Some("kodak-ultramax-400-datasheet"))).unwrap();
        stored["settings"]["dyeSet"] = json!("kodak-ultramax-400-datasheet");
        stored["settings"]["printPaper"] = json!("fujifilm-crystal-archive-type-ii");
        let read = read(&serde_json::to_vec(&stored).unwrap());
        assert_eq!(
            (read.film().map(|f| f.stock), read.dye_set, read.print_paper),
            (Some(VirtualStock::KodakUltraMax400), DyeSet::KodakUltraMax400, Some(PaperStock::FujifilmCrystalArchiveDigitalDPII))
        );
        // Twin: a key no stock carries is refused, never read as another.
        stored["settings"]["dyeSet"] = json!("kodak-ultramax-900-datasheet");
        assert!(super::super::document(&serde_json::to_vec(&stored).unwrap()).is_err());
    }
}
