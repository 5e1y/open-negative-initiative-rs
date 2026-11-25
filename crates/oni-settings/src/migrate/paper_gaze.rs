//! Before format 74 a colour negative's virtual scan was a camera's: a 5500 K lamp through its dyes and its orange base,
//! the 1931 observer standing in for the camera. Its paper's gaze reads it now, the bare film on one: a digital photo
//! moves onto its stock's new reference, every hand's move kept.

use super::chain::PAPER_GAZE_VERSION;
use super::datasheet::onto;
use crate::PipelineSettings;
use crate::virtual_film::{StockReference, VirtualStock};

const fn reference(stops: [f32; 3], black: [f32; 3], white: [f32; 3]) -> StockReference {
    StockReference { stops, black, white }
}

/// Each film a paper came to read, its reference as format 73 laid it on the camera's scan.
const CAMERA_SCANNED: [(VirtualStock, StockReference); 43] = {
    [
        (VirtualStock::of("fujifilm-160nc"), reference([-0.9640391, 0.0, 0.9483684], [0.2729955, 0.27292222, 0.27278253], [0.5727201, 0.6162498, 0.746715])),
        (VirtualStock::of("fujifilm-160nl"), reference([-1.005971, 0.0, 0.87908447], [0.28062552, 0.28054544, 0.28041172], [0.5597701, 0.60903186, 0.70729846])),
        (VirtualStock::of("fujifilm-160npc"), reference([-0.67431545, 0.0, 0.90788764], [0.25535512, 0.2553061, 0.25519115], [0.57333016, 0.60554594, 0.6043456])),
        (VirtualStock::of("fujifilm-160ns"), reference([-1.0817531, 0.0, 0.67363626], [0.289793, 0.28970274, 0.28960097], [0.56706023, 0.6027172, 0.6904327])),
        (VirtualStock::of("fujifilm-200"), reference([-0.9595904, 0.0, 1.3374684], [0.26779863, 0.26772845, 0.26750812], [0.48691207, 0.56227726, 0.6228138])),
        (VirtualStock::of("fujifilm-400"), reference([-0.95929855, 0.0, 1.3322794], [0.26873463, 0.26866397, 0.26844335], [0.50271344, 0.5787792, 0.62768567])),
        (VirtualStock::of("fujifilm-c200"), reference([-0.8118351, 0.0, 0.96117127], [0.2738331, 0.27376798, 0.27362478], [0.58652824, 0.6251889, 0.656622])),
        (VirtualStock::of("fujifilm-fujicolor-100"), reference([-0.8319393, 0.0, 0.57214195], [0.26234335, 0.26228264, 0.26221526], [0.6266944, 0.66616046, 0.7376494])),
        (VirtualStock::of("fujifilm-natura-1600"), reference([-0.87394243, 0.0, 1.0026436], [0.2740301, 0.27396125, 0.27380922], [0.6008719, 0.6619165, 0.6925561])),
        (VirtualStock::of("fujifilm-new-pro-400"), reference([-0.9439666, 0.0, 0.8451253], [0.29503042, 0.29494482, 0.29480302], [0.6111342, 0.6291645, 0.67047536])),
        (VirtualStock::of("fujifilm-nexia-400"), reference([-1.0979292, 0.0, 1.0560175], [0.35240135, 0.35225362, 0.3519548], [0.67033947, 0.7218176, 0.77332276])),
        (VirtualStock::of("fujifilm-nexia-a200"), reference([-1.0455583, 0.0, 1.2149187], [0.3602102, 0.36005837, 0.35966995], [0.6779901, 0.7168207, 0.77844137])),
        (VirtualStock::of("fujifilm-nexia-zoom-master-800"), reference([-1.0405893, 0.0, 1.2458065], [0.32198834, 0.32187563, 0.32157522], [0.6489435, 0.6828768, 0.72825944])),
        (VirtualStock::of("fujifilm-pro-160c"), reference([-1.3210522, 0.0, 0.6898484], [0.3492257, 0.3490634, 0.34889767], [0.6489079, 0.67486584, 0.7306926])),
        (VirtualStock::of("fujifilm-pro-160nc"), reference([-1.2226095, 0.0, 0.61464965], [0.33459115, 0.334453, 0.33432472], [0.6393278, 0.6614579, 0.7246478])),
        (VirtualStock::of("fujifilm-pro-160nh"), reference([-1.321149, 0.0, 0.690973], [0.3507955, 0.3506312, 0.35046315], [0.66511893, 0.6680222, 0.7126795])),
        (VirtualStock::of("fujifilm-pro-160ns"), reference([-1.2226095, 0.0, 0.61464965], [0.33459115, 0.334453, 0.33432472], [0.61664987, 0.6242718, 0.67986816])),
        (VirtualStock::of("fujifilm-pro-160s"), reference([-1.2226819, 0.0, 0.6146658], [0.33459148, 0.33445334, 0.33432505], [0.6022388, 0.6245065, 0.6656813])),
        (VirtualStock::of("fujifilm-pro-400h"), reference([-1.1131481, 0.0, 1.0613128], [0.3045687, 0.3044656, 0.30425733], [0.6013355, 0.62359434, 0.6396436])),
        (VirtualStock::of("fujifilm-pro-800"), reference([-0.97177553, 0.0, 1.2240151], [0.29034182, 0.29025754, 0.2900282], [0.61077833, 0.64319277, 0.65526134])),
        (VirtualStock::of("fujifilm-pro-800z"), reference([-0.99882776, 0.0, 1.1691515], [0.2916206, 0.29153392, 0.29131737], [0.6095812, 0.63847905, 0.64538574])),
        (VirtualStock::of("fujifilm-reala-ace"), reference([-0.8025582, 0.0, 0.77261883], [0.27076444, 0.27070135, 0.27059674], [0.59430397, 0.62845576, 0.6924713])),
        (VirtualStock::of("fujifilm-superia-100"), reference([-0.85543466, 0.0, 0.5695103], [0.2844502, 0.2843767, 0.28429726], [0.6252889, 0.6719442, 0.7055954])),
        (VirtualStock::of("fujifilm-superia-1600"), reference([-0.8739472, 0.0, 1.002535], [0.2740737, 0.27400485, 0.2738528], [0.60092044, 0.661978, 0.6926267])),
        (VirtualStock::of("fujifilm-superia-200"), reference([-0.82888764, 0.0, 0.6216013], [0.280883, 0.2808132, 0.28072718], [0.6436673, 0.69111156, 0.6996226])),
        (VirtualStock::of("fujifilm-superia-premium-400"), reference([-0.7656148, 0.0, 0.9436236], [0.2684969, 0.2684371, 0.26830307], [0.62721425, 0.653515, 0.68491524])),
        (VirtualStock::of("fujifilm-superia-reala"), reference([-0.70317215, 0.0, 0.9025418], [0.28257892, 0.28251642, 0.28237578], [0.57959265, 0.6300191, 0.6252227])),
        (VirtualStock::of("fujifilm-superia-venus-400"), reference([-0.75254095, 0.0, 0.8825567], [0.26756233, 0.26750374, 0.26738214], [0.59242266, 0.6455952, 0.6796321])),
        (VirtualStock::of("fujifilm-superia-venus-800"), reference([-0.82920986, 0.0, 0.908039], [0.2721581, 0.2720928, 0.2719619], [0.5981742, 0.6529946, 0.69806427])),
        (VirtualStock::of("fujifilm-xtra-400"), reference([-0.76449656, 0.0, 0.9420355], [0.26812172, 0.26806217, 0.26792884], [0.5967247, 0.6389474, 0.6661002])),
        (VirtualStock::of("fujifilm-xtra-800"), reference([-1.092658, 0.0, 1.1915616], [0.35782775, 0.35767418, 0.3573036], [0.6956444, 0.7590919, 0.80298376])),
        (VirtualStock::of("kodak-ektar-100"), reference([-0.87363106, 0.0, 0.7813517], [0.2769018, 0.27683142, 0.2767201], [0.550269, 0.59671974, 0.70716846])),
        (VirtualStock::of("kodak-gold-200"), reference([-0.8092655, 0.0, 1.0302407], [0.2780939, 0.27802676, 0.27786383], [0.5379109, 0.5873455, 0.64069086])),
        (VirtualStock::of("kodak-portra-160"), reference([-1.013348, 0.0, 0.76629716], [0.27191213, 0.2718369, 0.27173245], [0.5209439, 0.56879175, 0.66964984])),
        (VirtualStock::of("kodak-portra-160nc"), reference([-0.54341143, 0.0, 0.41908747], [0.31149137, 0.31142783, 0.31135964], [0.6583049, 0.67724556, 0.7263157])),
        (VirtualStock::of("kodak-portra-160vc"), reference([-0.54963344, 0.0, 0.539507], [0.3069475, 0.30688557, 0.30679703], [0.6676417, 0.68461347, 0.73645383])),
        (VirtualStock::of("kodak-portra-400"), reference([-0.97359693, 0.0, 0.7493512], [0.27577725, 0.27570185, 0.27559727], [0.5394989, 0.5825401, 0.69326246])),
        (VirtualStock::of("kodak-portra-400nc"), reference([-0.53965646, 0.0, 0.5063192], [0.3075107, 0.30744946, 0.307367], [0.6529777, 0.67259705, 0.7206442])),
        (VirtualStock::of("kodak-portra-400vc"), reference([-0.55462855, 0.0, 0.51768214], [0.3068407, 0.3067784, 0.30669415], [0.66736054, 0.6861979, 0.7376618])),
        (VirtualStock::of("kodak-portra-800"), reference([-0.82687217, 0.0, 1.2164922], [0.26587287, 0.26581076, 0.26562253], [0.5095668, 0.56159407, 0.6319505])),
        (VirtualStock::of("kodak-portra-800-push-1"), reference([-0.82687217, 0.0, 1.2164922], [0.26587287, 0.26581076, 0.26562253], [0.5391925, 0.59079474, 0.6599107])),
        (VirtualStock::of("kodak-portra-800-push-2"), reference([-0.82687217, 0.0, 1.2164922], [0.26587287, 0.26581076, 0.26562253], [0.52452505, 0.58000964, 0.6292395])),
        (VirtualStock::of("kodak-ultramax-400"), reference([-0.80550206, 0.0, 1.1800703], [0.26377177, 0.26371184, 0.26353472], [0.50806046, 0.5602547, 0.627706])),
    ]
};

/// The reference a document of format 73 laid for `stock`; a stock this step leaves unmoved, its own.
pub fn camera_scanned(stock: VirtualStock) -> StockReference {
    CAMERA_SCANNED.iter().find(|(s, _)| *s == stock).map_or_else(|| super::scanner::formerly_read(stock), |(_, r)| *r)
}

/// `settings` of `version` on its stock's reference as its paper's gaze read it, format 77's.
pub fn caught_up(version: i64, settings: PipelineSettings) -> PipelineSettings {
    match settings.film() {
        Some(film) if version < PAPER_GAZE_VERSION => onto(settings, camera_scanned(film.stock), super::scanner::formerly_read(film.stock)),
        _ => settings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConversionMode;

    #[test]
    fn a_digital_photo_lands_on_its_papers_reading_its_hand_kept() {
        let stock = VirtualStock::of("kodak-portra-800");
        let mut photo = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(stock);
        let (was, now) = (camera_scanned(stock), stock.reference());
        assert!(was.stops[0] < -0.5 && now.stops[0].abs() < 1e-3, "the camera saw the orange base, the paper's gaze reads the bare film on one");
        photo.gains.stops = was.stops.map(|v| v + 0.25);
        photo.levels = was.levels();
        photo.levels.green.white += 0.03;
        let landed = caught_up(PAPER_GAZE_VERSION - 1, photo.clone());
        assert!((0..3).all(|c| (landed.gains.stops[c] - (now.stops[c] + 0.25)).abs() < 1e-5), "{:?}", landed.gains.stops);
        assert!((landed.levels.green.white - (now.levels().green.white + 0.03)).abs() < 1e-5, "{:?}", landed.levels.green);
        assert!((landed.levels.red.black - now.levels().red.black).abs() < 1e-5, "{:?}", landed.levels.red);
        // Twins: a photo of this format stays where it stands, a film no paper reads and a scan move nothing.
        assert_eq!(caught_up(PAPER_GAZE_VERSION, photo.clone()), photo);
        let cine = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(VirtualStock::of("kodak-vision3-50d"));
        assert_eq!(caught_up(PAPER_GAZE_VERSION - 1, cine.clone()), cine);
        let scan = PipelineSettings::rest(ConversionMode::Negative, true);
        assert_eq!(caught_up(PAPER_GAZE_VERSION - 1, scan.clone()), scan);
    }
}
