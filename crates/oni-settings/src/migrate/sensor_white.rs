//! Before format 72 a film's reference laid its windows' white where a scene grey printed a plain RAW's grey on ONI's
//! print; it lies on the density of the sensor's saturation now, no grey placed (owner's call). A digital photo moves onto
//! its stock's new reference, every hand's move kept; a legacy print keeps the reference it was edited on.

use super::chain::EVERY_PAPER_VERSION;
use super::datasheet::onto;
use crate::PipelineSettings;
use crate::virtual_film::{StockReference, VirtualStock};

const fn reference(stops: [f32; 3], black: [f32; 3], white: [f32; 3]) -> StockReference {
    StockReference { stops, black, white }
}

/// Each film whose reference moved, as format 71 laid it.
const GREY_PLACED: [(VirtualStock, StockReference); 75] = {
    [
        (VirtualStock::of("foma-cine-ortho-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.7048051, 0.7048051, 0.7048051])),
        (VirtualStock::of("foma-fomapan-cine-100"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.64884645, 0.64884645, 0.64884645])),
        (VirtualStock::of("foma-fomapan-cine-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.65251607, 0.65251607, 0.65251607])),
        (VirtualStock::of("foma-fomapan-100"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.64124495, 0.64124495, 0.64124495])),
        (VirtualStock::of("foma-fomapan-200"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.6508648, 0.6508648, 0.6508648])),
        (VirtualStock::of("foma-fomapan-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.6400596, 0.6400596, 0.6400596])),
        (VirtualStock::of("foma-ortho-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.6923136, 0.6923136, 0.6923136])),
        (VirtualStock::of("foma-retropan-320"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4840337, 0.4840337, 0.4840337])),
        (VirtualStock::of("fujifilm-160nc"), reference([-0.9640391, 0.0, 0.9483684], [0.2729955, 0.27292222, 0.27278253], [0.6456853, 0.7174794, 0.9525013])),
        (VirtualStock::of("fujifilm-160nl"), reference([-1.005971, 0.0, 0.87908447], [0.28062552, 0.28054544, 0.28041172], [0.6162016, 0.68808544, 0.83712924])),
        (VirtualStock::of("fujifilm-160npc"), reference([-0.67431545, 0.0, 0.90788764], [0.25535512, 0.2553061, 0.25519115], [0.6536232, 0.69056565, 0.71367943])),
        (VirtualStock::of("fujifilm-160ns"), reference([-1.0817531, 0.0, 0.67363626], [0.289793, 0.28970274, 0.28960097], [0.63875926, 0.7066381, 0.8638247])),
        (VirtualStock::of("fujifilm-200"), reference([-0.9595904, 0.0, 1.3374684], [0.26779863, 0.26772845, 0.26750812], [0.53497326, 0.63887775, 0.6928415])),
        (VirtualStock::of("fujifilm-400"), reference([-0.95929855, 0.0, 1.3322794], [0.26873463, 0.26866397, 0.26844335], [0.5654847, 0.6784875, 0.71496844])),
        (VirtualStock::of("fujifilm-acros-ii"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.55640346, 0.55640346, 0.55640346])),
        (VirtualStock::of("fujifilm-c200"), reference([-0.8118351, 0.0, 0.96117127], [0.2738331, 0.27376798, 0.27362478], [0.6435925, 0.69935495, 0.75459963])),
        (VirtualStock::of("fujifilm-fujicolor-100"), reference([-0.8319393, 0.0, 0.57214195], [0.26234335, 0.26228264, 0.26221526], [0.69216037, 0.7600008, 0.8495594])),
        (VirtualStock::of("fujifilm-natura-1600"), reference([-0.87394243, 0.0, 1.0026436], [0.2740301, 0.27396125, 0.27380922], [0.66021097, 0.7581078, 0.76216936])),
        (VirtualStock::of("fujifilm-neopan-ortho"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.53186786, 0.53186786, 0.53186786])),
        (VirtualStock::of("fujifilm-neopan-1600"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5704813, 0.5704813, 0.5704813])),
        (VirtualStock::of("fujifilm-neopan-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.50119233, 0.50119233, 0.50119233])),
        (VirtualStock::of("fujifilm-new-pro-400"), reference([-0.9439666, 0.0, 0.8451253], [0.29503042, 0.29494482, 0.29480302], [0.6781855, 0.7044269, 0.77561307])),
        (VirtualStock::of("fujifilm-nexia-a200"), reference([-1.0455583, 0.0, 1.2149187], [0.3602102, 0.36005837, 0.35966995], [0.75511783, 0.8050372, 0.91034925])),
        (VirtualStock::of("fujifilm-nexia-zoom-master-800"), reference([-1.0405893, 0.0, 1.2458065], [0.32198834, 0.32187563, 0.32157522], [0.6987333, 0.7480892, 0.79786384])),
        (VirtualStock::of("fujifilm-nexia-400"), reference([-1.0979292, 0.0, 1.0560175], [0.35240135, 0.35225362, 0.3519548], [0.7363726, 0.8061963, 0.86546874])),
        (VirtualStock::of("fujifilm-pro-160c"), reference([-1.3210522, 0.0, 0.6898484], [0.3492257, 0.3490634, 0.34889767], [0.7064921, 0.75305337, 0.8431189])),
        (VirtualStock::of("fujifilm-pro-160nc"), reference([-1.2226095, 0.0, 0.61464965], [0.33459115, 0.334453, 0.33432472], [0.69921684, 0.73967373, 0.8397691])),
        (VirtualStock::of("fujifilm-pro-160nh"), reference([-1.321149, 0.0, 0.690973], [0.3507955, 0.3506312, 0.35046315], [0.7282444, 0.736455, 0.82963693])),
        (VirtualStock::of("fujifilm-pro-160ns"), reference([-1.2226095, 0.0, 0.61464965], [0.33459115, 0.334453, 0.33432472], [0.6777413, 0.69032323, 0.7906842])),
        (VirtualStock::of("fujifilm-pro-160s"), reference([-1.2226819, 0.0, 0.6146658], [0.33459148, 0.33445334, 0.33432505], [0.6678294, 0.7042264, 0.75705135])),
        (VirtualStock::of("fujifilm-pro-400h"), reference([-1.1131481, 0.0, 1.0613128], [0.3045687, 0.3044656, 0.30425733], [0.6719427, 0.7094145, 0.74326783])),
        (VirtualStock::of("fujifilm-pro-800"), reference([-0.97177553, 0.0, 1.2240151], [0.29034182, 0.29025754, 0.2900282], [0.67980945, 0.7364429, 0.7532963])),
        (VirtualStock::of("fujifilm-pro-800z"), reference([-0.99882776, 0.0, 1.1691515], [0.2916206, 0.29153392, 0.29131737], [0.6693094, 0.71910405, 0.7369263])),
        (VirtualStock::of("fujifilm-reala-ace"), reference([-0.8025582, 0.0, 0.77261883], [0.27076444, 0.27070135, 0.27059674], [0.66969234, 0.70836794, 0.7723475])),
        (VirtualStock::of("fujifilm-superia-premium-400"), reference([-0.7656148, 0.0, 0.9436236], [0.2684969, 0.2684371, 0.26830307], [0.6998753, 0.74252105, 0.7842599])),
        (VirtualStock::of("fujifilm-superia-reala"), reference([-0.70317215, 0.0, 0.9025418], [0.28257892, 0.28251642, 0.28237578], [0.6181408, 0.6998296, 0.6657789])),
        (VirtualStock::of("fujifilm-superia-venus-400"), reference([-0.75254095, 0.0, 0.8825567], [0.26756233, 0.26750374, 0.26738214], [0.6567042, 0.7378628, 0.772703])),
        (VirtualStock::of("fujifilm-superia-venus-800"), reference([-0.82920986, 0.0, 0.908039], [0.2721581, 0.2720928, 0.2719619], [0.6644267, 0.7507753, 0.7870207])),
        (VirtualStock::of("fujifilm-xtra-400"), reference([-0.76449656, 0.0, 0.9420355], [0.26812172, 0.26806217, 0.26792884], [0.6768441, 0.7314843, 0.7635552])),
        (VirtualStock::of("fujifilm-xtra-800"), reference([-1.092658, 0.0, 1.1915616], [0.35782775, 0.35767418, 0.3573036], [0.7582861, 0.8447292, 0.9157052])),
        (VirtualStock::of("fujifilm-superia-100"), reference([-0.85543466, 0.0, 0.5695103], [0.2844502, 0.2843767, 0.28429726], [0.63832456, 0.6979092, 0.74539405])),
        (VirtualStock::of("fujifilm-superia-1600"), reference([-0.8739472, 0.0, 1.002535], [0.2740737, 0.27400485, 0.2738528], [0.66026735, 0.758198, 0.7622949])),
        (VirtualStock::of("fujifilm-superia-200"), reference([-0.82888764, 0.0, 0.6216013], [0.280883, 0.2808132, 0.28072718], [0.6914965, 0.74411, 0.76442724])),
        (VirtualStock::of("ilford-delta-100"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.54539967, 0.54539967, 0.54539967])),
        (VirtualStock::of("ilford-delta-3200"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.50518507, 0.50518507, 0.50518507])),
        (VirtualStock::of("ilford-delta-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5495109, 0.5495109, 0.5495109])),
        (VirtualStock::of("ilford-fp4-plus"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5620778, 0.5620778, 0.5620778])),
        (VirtualStock::of("ilford-hp5-plus"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.529545, 0.529545, 0.529545])),
        (VirtualStock::of("ilford-ortho-plus"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5154966, 0.5154966, 0.5154966])),
        (VirtualStock::of("ilford-pan-f-plus"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5947504, 0.5947504, 0.5947504])),
        (VirtualStock::of("ilford-sfx-200"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5630885, 0.5630885, 0.5630885])),
        (VirtualStock::of("ilford-xp2-super"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.58853966, 0.58853966, 0.58853966])),
        (VirtualStock::of("kodak-tri-x-320"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.49478894, 0.49478894, 0.49478894])),
        (VirtualStock::of("kodak-tri-x-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5753634, 0.5753634, 0.5753634])),
        (VirtualStock::of("kodak-double-x"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5464296, 0.5464296, 0.5464296])),
        (VirtualStock::of("kodak-ektar-100"), reference([-0.87363106, 0.0, 0.7813517], [0.2769018, 0.27683142, 0.2767201], [0.62975806, 0.693153, 0.84697115])),
        (VirtualStock::of("kodak-gold-200"), reference([-0.8092655, 0.0, 1.0302407], [0.2780939, 0.27802676, 0.27786383], [0.6026584, 0.67489576, 0.7369764])),
        (VirtualStock::of("kodak-portra-160"), reference([-1.013348, 0.0, 0.76629716], [0.27191213, 0.2718369, 0.27173245], [0.5841321, 0.6534282, 0.7797758])),
        (VirtualStock::of("kodak-portra-160nc"), reference([-0.54341143, 0.0, 0.41908747], [0.31149137, 0.31142783, 0.31135964], [0.7479054, 0.78127277, 0.8664988])),
        (VirtualStock::of("kodak-portra-160vc"), reference([-0.54963344, 0.0, 0.539507], [0.3069475, 0.30688557, 0.30679703], [0.7594454, 0.7897959, 0.88024586])),
        (VirtualStock::of("kodak-portra-400"), reference([-0.97359693, 0.0, 0.7493512], [0.27577725, 0.27570185, 0.27559727], [0.6083739, 0.6715792, 0.82896554])),
        (VirtualStock::of("kodak-portra-400nc"), reference([-0.53965646, 0.0, 0.5063192], [0.3075107, 0.30744946, 0.307367], [0.7417953, 0.77519774, 0.85813606])),
        (VirtualStock::of("kodak-portra-400vc"), reference([-0.55462855, 0.0, 0.51768214], [0.3068407, 0.3067784, 0.30669415], [0.75782484, 0.7910595, 0.87955284])),
        (VirtualStock::of("kodak-portra-800"), reference([-0.82687217, 0.0, 1.2164922], [0.26587287, 0.26581076, 0.26562253], [0.57259464, 0.65277714, 0.7224338])),
        (VirtualStock::of("kodak-portra-800-push-1"), reference([-0.82687217, 0.0, 1.2164922], [0.26587287, 0.26581076, 0.26562253], [0.593662, 0.66120166, 0.7185992])),
        (VirtualStock::of("kodak-portra-800-push-2"), reference([-0.82687217, 0.0, 1.2164922], [0.26587287, 0.26581076, 0.26562253], [0.5366557, 0.6035826, 0.63113093])),
        (VirtualStock::of("kodak-tmax-p3200"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.6569006, 0.6569006, 0.6569006])),
        (VirtualStock::of("kodak-tmax-100"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5124686, 0.5124686, 0.5124686])),
        (VirtualStock::of("kodak-tmax-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.47859782, 0.47859782, 0.47859782])),
        (VirtualStock::of("kodak-ultramax-400"), reference([-0.80550206, 0.0, 1.1800703], [0.26377177, 0.26371184, 0.26353472], [0.56984985, 0.6473474, 0.7192695])),
        (VirtualStock::of("kodak-verita-200d"), reference([-1.0046501, 0.0, 0.9290177], [0.2943861, 0.29429713, 0.294137], [0.65419614, 0.686036, 0.7570803])),
        (VirtualStock::of("kodak-vision3-200t"), reference([-0.8056435, 0.0, 0.88720316], [0.26404598, 0.26398596, 0.26386678], [0.5910487, 0.6608767, 0.69089377])),
        (VirtualStock::of("kodak-vision3-250d"), reference([-0.73414564, 0.0, 1.0144322], [0.2623733, 0.26231802, 0.2621768], [0.57940304, 0.6488145, 0.678784])),
        (VirtualStock::of("kodak-vision3-500t"), reference([-0.7581185, 0.0, 0.91422534], [0.265473, 0.265415, 0.2652896], [0.5716021, 0.6423091, 0.67067915])),
        (VirtualStock::of("kodak-vision3-50d"), reference([-0.89372385, 0.0, 0.9421484], [0.2588959, 0.2588336, 0.2587094], [0.58810747, 0.6532861, 0.68248034])),
    ]
};

/// The reference a document of format 71 laid for `stock`; a stock this step leaves unmoved, its own.
pub fn grey_placed(stock: VirtualStock) -> StockReference {
    GREY_PLACED.iter().find(|(s, _)| *s == stock).map_or_else(|| stock.reference(), |(_, r)| *r)
}

/// `settings` of `version` on its stock's reference of the sensor's saturation, but on a legacy print.
pub fn caught_up(version: i64, settings: PipelineSettings) -> PipelineSettings {
    match settings.film() {
        Some(film) if version < EVERY_PAPER_VERSION && settings.legacy_print.is_none() => onto(settings, grey_placed(film.stock), super::paper_gaze::camera_scanned(film.stock)),
        _ => settings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConversionMode, LegacyPrint};

    #[test]
    fn a_digital_photo_lands_on_its_sensors_reference_its_hand_kept_and_a_legacy_print_keeps_its_own() {
        let stock = VirtualStock::of("kodak-portra-800");
        let mut photo = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(stock);
        let was = grey_placed(stock);
        photo.gains.stops = was.stops.map(|v| v + 0.25);
        photo.levels = was.levels();
        photo.levels.green.white += 0.03;
        let landed = caught_up(EVERY_PAPER_VERSION - 1, photo.clone());
        let now = super::super::paper_gaze::camera_scanned(stock);
        assert!(now.white != was.white, "the sensor's saturation moves Portra 800's white");
        assert!((0..3).all(|c| (landed.gains.stops[c] - (now.stops[c] + 0.25)).abs() < 1e-6), "{:?}", landed.gains.stops);
        assert!((landed.levels.green.white - (now.levels().green.white + 0.03)).abs() < 1e-5, "{:?}", landed.levels.green);
        // Twins: a photo of this format and a legacy print stay where they stand, and a scan moves nothing.
        assert_eq!(caught_up(EVERY_PAPER_VERSION, photo.clone()), photo);
        let legacy = PipelineSettings { legacy_print: Some(LegacyPrint::default()), ..photo };
        assert_eq!(caught_up(EVERY_PAPER_VERSION - 1, legacy.clone()), legacy);
        let scan = PipelineSettings::rest(ConversionMode::Negative, true);
        assert_eq!(caught_up(EVERY_PAPER_VERSION - 1, scan.clone()), scan);
    }
}
