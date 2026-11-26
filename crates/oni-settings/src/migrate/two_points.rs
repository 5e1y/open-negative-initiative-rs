//! Before format 80 a colour negative's virtual film offset its layers, each its own exposure, so a scene's mid grey held
//! one share of the three windows, and compressed the scene's gamut before its spectra. A neutral now exposes every layer
//! at the same log exposure and the scene's chromaticity is read as it is: a digital photo moves onto its stock's new
//! reference, every hand's move kept.

use super::chain::{TWO_POINTS_VERSION, UNIVERSAL_SCANNER_VERSION};
use super::datasheet::onto;
use crate::PipelineSettings;
use crate::virtual_film::{StockReference, VirtualStock};

const fn reference(stops: [f32; 3], black: [f32; 3], white: [f32; 3]) -> StockReference {
    StockReference { stops, black, white }
}

/// Each colour negative whose layers were offset, its reference as format 79 laid it.
const OFFSET: [(VirtualStock, StockReference); 48] = [
        (VirtualStock::of("fujifilm-160nc"), reference([-0.9013497, 0.0, 0.74141145], [0.27732164, 0.2772494, 0.27714503], [0.59065187, 0.6245294, 0.6913385])),
        (VirtualStock::of("fujifilm-160nl"), reference([-0.9415304, 0.0, 0.68423134], [0.28466877, 0.28458995, 0.28449014], [0.5767469, 0.61864233, 0.6765017])),
        (VirtualStock::of("fujifilm-160npc"), reference([-0.6119418, 0.0, 0.65248317], [0.25784624, 0.25779998, 0.2577235], [0.5702408, 0.6023489, 0.57863915])),
        (VirtualStock::of("fujifilm-160ns"), reference([-1.0166289, 0.0, 0.50314903], [0.29373732, 0.29364806, 0.2935745], [0.6042179, 0.6188395, 0.6651906])),
        (VirtualStock::of("fujifilm-200"), reference([-0.90284276, 0.0, 0.99397933], [0.26984784, 0.26977956, 0.26963413], [0.5093761, 0.581, 0.6452124])),
        (VirtualStock::of("fujifilm-400"), reference([-0.9024665, 0.0, 0.987446], [0.27065694, 0.27058825, 0.27044326], [0.53228784, 0.6015377, 0.66127414])),
        (VirtualStock::of("fujifilm-c200"), reference([-0.74759644, 0.0, 0.729221], [0.27735943, 0.27729654, 0.27719432], [0.60421467, 0.6317985, 0.6415432])),
        (VirtualStock::of("fujifilm-fujicolor-100"), reference([-0.7673213, 0.0, 0.33388895], [0.2634096, 0.26335204, 0.26331568], [0.6564494, 0.6796073, 0.7351894])),
        (VirtualStock::of("fujifilm-natura-1600"), reference([-0.8162689, 0.0, 0.7372811], [0.27780533, 0.27773792, 0.27763388], [0.6436113, 0.6894799, 0.74097407])),
        (VirtualStock::of("fujifilm-new-pro-400"), reference([-0.881575, 0.0, 0.5931525], [0.29821005, 0.29812655, 0.29803374], [0.62047905, 0.6342871, 0.6515575])),
        (VirtualStock::of("fujifilm-nexia-400"), reference([-1.0252551, 0.0, 0.7960425], [0.35692075, 0.35677463, 0.35656348], [0.6824561, 0.7368651, 0.8146817])),
        (VirtualStock::of("fujifilm-nexia-a200"), reference([-0.9751745, 0.0, 0.9615985], [0.36561725, 0.36546636, 0.3651759], [0.6802679, 0.7215984, 0.75513875])),
        (VirtualStock::of("fujifilm-nexia-zoom-master-800"), reference([-0.9762343, 0.0, 0.96782064], [0.32764593, 0.32753322, 0.32731444], [0.6702687, 0.70098895, 0.7641557])),
        (VirtualStock::of("fujifilm-pro-160c"), reference([-1.2605646, 0.0, 0.43353972], [0.3532261, 0.3530635, 0.35296577], [0.6744581, 0.68599725, 0.7025325])),
        (VirtualStock::of("fujifilm-pro-160nc"), reference([-1.1572819, 0.0, 0.36817965], [0.3377096, 0.337573, 0.33750105], [0.65986437, 0.6686458, 0.69126827])),
        (VirtualStock::of("fujifilm-pro-160nh"), reference([-1.2606562, 0.0, 0.43497506], [0.35479656, 0.35463196, 0.35453266], [0.67155254, 0.6664423, 0.66313195])),
        (VirtualStock::of("fujifilm-pro-160ns"), reference([-1.1572819, 0.0, 0.36817965], [0.3377096, 0.337573, 0.33750105], [0.6209731, 0.62252104, 0.63378483])),
        (VirtualStock::of("fujifilm-pro-160s"), reference([-1.1573535, 0.0, 0.36819723], [0.3377099, 0.3375733, 0.33750132], [0.61796045, 0.6314739, 0.6523778])),
        (VirtualStock::of("fujifilm-pro-400h"), reference([-1.060365, 0.0, 0.7722454], [0.30813426, 0.30803165, 0.3078922], [0.6186785, 0.631987, 0.6329302])),
        (VirtualStock::of("fujifilm-pro-800"), reference([-0.9183769, 0.0, 0.9395107], [0.29449436, 0.29441077, 0.29424804], [0.64030313, 0.659165, 0.6655999])),
        (VirtualStock::of("fujifilm-pro-800z"), reference([-0.9458803, 0.0, 0.8758466], [0.29552937, 0.29544333, 0.29529408], [0.635004, 0.6529632, 0.65178436])),
        (VirtualStock::of("fujifilm-reala-ace"), reference([-0.73520136, 0.0, 0.52698916], [0.27265817, 0.27259827, 0.2725322], [0.59506094, 0.63271874, 0.69360644])),
        (VirtualStock::of("fujifilm-superia-100"), reference([-0.7801985, 0.0, 0.40093], [0.287541, 0.28747076, 0.28741688], [0.6432301, 0.67699206, 0.68645364])),
        (VirtualStock::of("fujifilm-superia-1600"), reference([-0.8162715, 0.0, 0.7371742], [0.27784833, 0.2777809, 0.27767685], [0.6436899, 0.6895567, 0.7410432])),
        (VirtualStock::of("fujifilm-superia-200"), reference([-0.7665635, 0.0, 0.4230161], [0.28290722, 0.2828403, 0.282785], [0.6435053, 0.68678707, 0.6765244])),
        (VirtualStock::of("fujifilm-superia-premium-400"), reference([-0.70059645, 0.0, 0.7215313], [0.2721502, 0.27209273, 0.2719958], [0.64700645, 0.66137135, 0.67810833])),
        (VirtualStock::of("fujifilm-superia-reala"), reference([-0.6406145, 0.0, 0.70882696], [0.28539586, 0.28533652, 0.28523162], [0.61065406, 0.64157027, 0.64270717])),
        (VirtualStock::of("fujifilm-superia-venus-400"), reference([-0.68656653, 0.0, 0.66605395], [0.27097407, 0.27091798, 0.2708312], [0.62121814, 0.6592978, 0.6850444])),
        (VirtualStock::of("fujifilm-superia-venus-800"), reference([-0.766672, 0.0, 0.6703135], [0.2756432, 0.27557993, 0.2754892], [0.6313384, 0.67610216, 0.7310553])),
        (VirtualStock::of("fujifilm-xtra-400"), reference([-0.699455, 0.0, 0.7200861], [0.2717626, 0.2717054, 0.271609], [0.6031177, 0.6420585, 0.6598871])),
        (VirtualStock::of("fujifilm-xtra-800"), reference([-1.020476, 0.0, 0.8769872], [0.36145642, 0.36130562, 0.36105722], [0.7096558, 0.76997083, 0.80349797])),
        (VirtualStock::of("kodak-ektar-100"), reference([-0.808746, 0.0, 0.5619003], [0.27944127, 0.27937347, 0.27929822], [0.558933, 0.6051261, 0.69804806])),
        (VirtualStock::of("kodak-gold-200"), reference([-0.7437748, 0.0, 0.73763025], [0.2794227, 0.27935907, 0.27925366], [0.55819297, 0.60291994, 0.66992956])),
        (VirtualStock::of("kodak-portra-160"), reference([-0.95093185, 0.0, 0.59615415], [0.2760334, 0.27595904, 0.27588034], [0.5390713, 0.58475006, 0.6838595])),
        (VirtualStock::of("kodak-portra-160nc"), reference([-0.5152746, 0.0, 0.30540606], [0.31191257, 0.3118516, 0.31180376], [0.667047, 0.6745561, 0.7239373])),
        (VirtualStock::of("kodak-portra-160vc"), reference([-0.5219312, 0.0, 0.41159153], [0.30779487, 0.30773517, 0.30767027], [0.673951, 0.6799469, 0.72823])),
        (VirtualStock::of("kodak-portra-400"), reference([-0.915065, 0.0, 0.58948374], [0.2808767, 0.28080165, 0.28072104], [0.5582507, 0.59707016, 0.68597])),
        (VirtualStock::of("kodak-portra-400nc"), reference([-0.51076317, 0.0, 0.3734156], [0.30799887, 0.30794016, 0.30788198], [0.6608472, 0.6699437, 0.7187813])),
        (VirtualStock::of("kodak-portra-400vc"), reference([-0.52548945, 0.0, 0.37599155], [0.30737153, 0.3073117, 0.30725333], [0.6766017, 0.68404704, 0.736608])),
        (VirtualStock::of("kodak-portra-800"), reference([-0.7681999, 0.0, 0.91295016], [0.2688804, 0.26882026, 0.26869172], [0.5382111, 0.5837439, 0.67924094])),
        (VirtualStock::of("kodak-portra-800-push-1"), reference([-0.7681999, 0.0, 0.91295016], [0.2688804, 0.26882026, 0.26869172], [0.5562423, 0.6087431, 0.7173045])),
        (VirtualStock::of("kodak-portra-800-push-2"), reference([-0.7681999, 0.0, 0.91295016], [0.2688804, 0.26882026, 0.26869172], [0.5423264, 0.59650683, 0.66994])),
        (VirtualStock::of("kodak-ultramax-400"), reference([-0.74341524, 0.0, 0.87521887], [0.26637736, 0.2663198, 0.2662007], [0.53211564, 0.5789603, 0.6595211])),
        (VirtualStock::of("kodak-verita-200d"), reference([-0.94407433, 0.0, 0.74446404], [0.29920754, 0.29911914, 0.29899496], [0.5655937, 0.60126215, 0.73275346])),
        (VirtualStock::of("kodak-vision3-200t"), reference([-0.75497735, 0.0, 0.6959914], [0.26693597, 0.2668775, 0.26678857], [0.5263995, 0.578796, 0.60719895])),
        (VirtualStock::of("kodak-vision3-250d"), reference([-0.6818431, 0.0, 0.80664283], [0.26541534, 0.2653619, 0.26525572], [0.52193844, 0.5741955, 0.60073966])),
        (VirtualStock::of("kodak-vision3-500t"), reference([-0.71135527, 0.0, 0.7303293], [0.26834157, 0.26828513, 0.26818955], [0.51748246, 0.5681906, 0.59124154])),
        (VirtualStock::of("kodak-vision3-50d"), reference([-0.8358752, 0.0, 0.711386], [0.26140577, 0.26134527, 0.2612577], [0.5217362, 0.57261175, 0.6018331])),
];

/// The reference a document of format 79 laid for `stock`; a stock this step leaves unmoved, its own.
pub fn offset(stock: VirtualStock) -> StockReference {
    OFFSET.iter().find(|(s, _)| *s == stock).map_or_else(|| stock.reference(), |(_, r)| *r)
}

/// `settings` of `version` on its stock's two points; one older than format 79 is moved there by the scanner's step.
pub fn caught_up(version: i64, settings: PipelineSettings) -> PipelineSettings {
    match settings.film() {
        Some(film) if (UNIVERSAL_SCANNER_VERSION..TWO_POINTS_VERSION).contains(&version) => onto(settings, offset(film.stock), film.stock.reference()),
        _ => settings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConversionMode;

    #[test]
    fn a_digital_photo_lands_on_its_stocks_two_points_its_hand_kept() {
        let stock = VirtualStock::of("kodak-portra-800");
        let mut photo = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(stock);
        let (was, now) = (offset(stock), stock.reference());
        assert!(was.white.iter().zip(&now.white).any(|(a, b)| (a - b).abs() > 0.02), "{was:?} {now:?}");
        photo.gains.stops = was.stops.map(|v| v + 0.25);
        photo.levels = was.levels();
        photo.levels.blue.white += 0.03;
        let landed = caught_up(UNIVERSAL_SCANNER_VERSION, photo.clone());
        assert!((0..3).all(|c| (landed.gains.stops[c] - (now.stops[c] + 0.25)).abs() < 1e-5), "{:?}", landed.gains.stops);
        assert!((landed.levels.blue.white - (now.levels().blue.white + 0.03)).abs() < 1e-5, "{:?}", landed.levels.blue);
        // Twins: a photo of this format, or one the scanner's step already moved, stays; a black and white film and a
        // scan move nothing.
        assert_eq!(caught_up(TWO_POINTS_VERSION, photo.clone()), photo);
        assert_eq!(caught_up(UNIVERSAL_SCANNER_VERSION - 1, photo.clone()), photo);
        let silver = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(VirtualStock::of("ilford-hp5-plus"));
        assert_eq!(caught_up(UNIVERSAL_SCANNER_VERSION, silver.clone()), silver);
        let scan = PipelineSettings::rest(ConversionMode::Negative, true);
        assert_eq!(caught_up(UNIVERSAL_SCANNER_VERSION, scan.clone()), scan);
    }
}
