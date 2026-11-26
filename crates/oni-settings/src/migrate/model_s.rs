//! Before format 81 a colour negative's virtual film was read by a scanner: its dyes and orange base under a lamp, seen by
//! a camera's sensor and decoded. It is read now as its data sheet's Status M densities, each channel its own layer, the
//! layers inhibiting one another: a digital photo moves onto its stock's new reference, every hand's move kept.

use super::chain::{MODEL_S_VERSION, TWO_POINTS_VERSION};
use super::datasheet::onto;
use crate::PipelineSettings;
use crate::virtual_film::{StockReference, VirtualStock};

const fn reference(stops: [f32; 3], black: [f32; 3], white: [f32; 3]) -> StockReference {
    StockReference { stops, black, white }
}

/// Each colour negative the scanner read, its reference as format 80 laid it.
const SCANNED: [(VirtualStock, StockReference); 48] = [
    (VirtualStock::of("fujifilm-160nc"), reference([-0.9013497, 0.0, 0.74141145], [0.27732164, 0.2772494, 0.27714503], [0.58018416, 0.6238499, 0.7260578])),
    (VirtualStock::of("fujifilm-160nl"), reference([-0.9415304, 0.0, 0.68423134], [0.28466877, 0.28458995, 0.28449014], [0.56744397, 0.6162522, 0.6901432])),
    (VirtualStock::of("fujifilm-160npc"), reference([-0.6119418, 0.0, 0.65248317], [0.25784624, 0.25779998, 0.2577235], [0.5795194, 0.60825646, 0.6016027])),
    (VirtualStock::of("fujifilm-160ns"), reference([-1.0166289, 0.0, 0.50314903], [0.29373732, 0.29364806, 0.2935745], [0.57375795, 0.6082541, 0.6779792])),
    (VirtualStock::of("fujifilm-200"), reference([-0.90284276, 0.0, 0.99397933], [0.26984784, 0.26977956, 0.26963413], [0.4973862, 0.57127863, 0.60827917])),
    (VirtualStock::of("fujifilm-400"), reference([-0.9024665, 0.0, 0.987446], [0.27065694, 0.27058825, 0.27044326], [0.51273346, 0.58714515, 0.613111])),
    (VirtualStock::of("fujifilm-c200"), reference([-0.74759644, 0.0, 0.729221], [0.27735943, 0.27729654, 0.27719432], [0.5917958, 0.6277139, 0.64536417])),
    (VirtualStock::of("fujifilm-fujicolor-100"), reference([-0.7673213, 0.0, 0.33388895], [0.2634096, 0.26335204, 0.26331568], [0.63256574, 0.66949254, 0.72466207])),
    (VirtualStock::of("fujifilm-natura-1600"), reference([-0.8162689, 0.0, 0.7372811], [0.27780533, 0.27773792, 0.27763388], [0.6090808, 0.6694461, 0.690226])),
    (VirtualStock::of("fujifilm-new-pro-400"), reference([-0.881575, 0.0, 0.5931525], [0.29821005, 0.29812655, 0.29803374], [0.61912704, 0.6353182, 0.66928095])),
    (VirtualStock::of("fujifilm-nexia-400"), reference([-1.0252551, 0.0, 0.7960425], [0.35692075, 0.35677463, 0.35656348], [0.67976767, 0.7348, 0.7959017])),
    (VirtualStock::of("fujifilm-nexia-a200"), reference([-0.9751745, 0.0, 0.9615985], [0.36561725, 0.36546636, 0.3651759], [0.6860781, 0.7260414, 0.7752663])),
    (VirtualStock::of("fujifilm-nexia-zoom-master-800"), reference([-0.9762343, 0.0, 0.96782064], [0.32764593, 0.32753322, 0.32731444], [0.6588382, 0.69410366, 0.7426484])),
    (VirtualStock::of("fujifilm-pro-160c"), reference([-1.2605646, 0.0, 0.43353972], [0.3532261, 0.3530635, 0.35296577], [0.65584236, 0.67939097, 0.7106306])),
    (VirtualStock::of("fujifilm-pro-160nc"), reference([-1.1572819, 0.0, 0.36817965], [0.3377096, 0.337573, 0.33750105], [0.6455471, 0.664877, 0.7030663])),
    (VirtualStock::of("fujifilm-pro-160nh"), reference([-1.2606562, 0.0, 0.43497506], [0.35479656, 0.35463196, 0.35453266], [0.67153996, 0.6709754, 0.6946577])),
    (VirtualStock::of("fujifilm-pro-160ns"), reference([-1.1572819, 0.0, 0.36817965], [0.3377096, 0.337573, 0.33750105], [0.622425, 0.6268614, 0.6608116])),
    (VirtualStock::of("fujifilm-pro-160s"), reference([-1.1573535, 0.0, 0.36819723], [0.3377099, 0.3375733, 0.33750132], [0.6082144, 0.6273966, 0.6491165])),
    (VirtualStock::of("fujifilm-pro-400h"), reference([-1.060365, 0.0, 0.7722454], [0.30813426, 0.30803165, 0.3078922], [0.6092463, 0.6289468, 0.639689])),
    (VirtualStock::of("fujifilm-pro-800"), reference([-0.9183769, 0.0, 0.9395107], [0.29449436, 0.29441077, 0.29424804], [0.6196794, 0.6499687, 0.656842])),
    (VirtualStock::of("fujifilm-pro-800z"), reference([-0.9458803, 0.0, 0.8758466], [0.29552937, 0.29544333, 0.29529408], [0.6190107, 0.6460711, 0.65178835])),
    (VirtualStock::of("fujifilm-reala-ace"), reference([-0.73520136, 0.0, 0.52698916], [0.27265817, 0.27259827, 0.2725322], [0.6008987, 0.63317734, 0.6794656])),
    (VirtualStock::of("fujifilm-superia-100"), reference([-0.7801985, 0.0, 0.40093], [0.287541, 0.28747076, 0.28741688], [0.62948793, 0.6732946, 0.6920051])),
    (VirtualStock::of("fujifilm-superia-1600"), reference([-0.8162715, 0.0, 0.7371742], [0.27784833, 0.2777809, 0.27767685], [0.60912967, 0.6695098, 0.69029677])),
    (VirtualStock::of("fujifilm-superia-200"), reference([-0.7665635, 0.0, 0.4230161], [0.28290722, 0.2828403, 0.282785], [0.6480223, 0.6898578, 0.6872888])),
    (VirtualStock::of("fujifilm-superia-premium-400"), reference([-0.70059645, 0.0, 0.7215313], [0.2721502, 0.27209273, 0.2719958], [0.63196325, 0.65495616, 0.6733824])),
    (VirtualStock::of("fujifilm-superia-reala"), reference([-0.6406145, 0.0, 0.70882696], [0.28539586, 0.28533652, 0.28523162], [0.5835701, 0.6285322, 0.60786647])),
    (VirtualStock::of("fujifilm-superia-venus-400"), reference([-0.68656653, 0.0, 0.66605395], [0.27097407, 0.27091798, 0.2708312], [0.59776086, 0.64820683, 0.66805744])),
    (VirtualStock::of("fujifilm-superia-venus-800"), reference([-0.766672, 0.0, 0.6703135], [0.2756432, 0.27557993, 0.2754892], [0.6071416, 0.66140866, 0.69627106])),
    (VirtualStock::of("fujifilm-xtra-400"), reference([-0.699455, 0.0, 0.7200861], [0.2717626, 0.2717054, 0.271609], [0.60188234, 0.64105475, 0.6558954])),
    (VirtualStock::of("fujifilm-xtra-800"), reference([-1.020476, 0.0, 0.8769872], [0.36145642, 0.36130562, 0.36105722], [0.70450515, 0.76852566, 0.8063147])),
    (VirtualStock::of("kodak-ektar-100"), reference([-0.808746, 0.0, 0.5619003], [0.27944127, 0.27937347, 0.27929822], [0.55894876, 0.6055327, 0.7022578])),
    (VirtualStock::of("kodak-gold-200"), reference([-0.7437748, 0.0, 0.73763025], [0.2794227, 0.27935907, 0.27925366], [0.54631126, 0.5951021, 0.6408475])),
    (VirtualStock::of("kodak-portra-160"), reference([-0.95093185, 0.0, 0.59615415], [0.2760334, 0.27595904, 0.27588034], [0.5306029, 0.5797424, 0.6683553])),
    (VirtualStock::of("kodak-portra-160nc"), reference([-0.5152746, 0.0, 0.30540606], [0.31191257, 0.3118516, 0.31180376], [0.6606774, 0.6793214, 0.7481296])),
    (VirtualStock::of("kodak-portra-160vc"), reference([-0.5219312, 0.0, 0.41159153], [0.30779487, 0.30773517, 0.30767027], [0.66976625, 0.68653584, 0.7557075])),
    (VirtualStock::of("kodak-portra-400"), reference([-0.915065, 0.0, 0.58948374], [0.2808767, 0.28080165, 0.28072104], [0.54937404, 0.5935447, 0.6925543])),
    (VirtualStock::of("kodak-portra-400nc"), reference([-0.51076317, 0.0, 0.3734156], [0.30799887, 0.30794016, 0.30788198], [0.6555686, 0.67475384, 0.7418885])),
    (VirtualStock::of("kodak-portra-400vc"), reference([-0.52548945, 0.0, 0.37599155], [0.30737153, 0.3073117, 0.30725333], [0.6701788, 0.6885495, 0.7596558])),
    (VirtualStock::of("kodak-portra-800"), reference([-0.7681999, 0.0, 0.91295016], [0.2688804, 0.26882026, 0.26869172], [0.5191182, 0.5714021, 0.6326997])),
    (VirtualStock::of("kodak-portra-800-push-1"), reference([-0.7681999, 0.0, 0.91295016], [0.2688804, 0.26882026, 0.26869172], [0.5490901, 0.6010188, 0.6620983])),
    (VirtualStock::of("kodak-portra-800-push-2"), reference([-0.7681999, 0.0, 0.91295016], [0.2688804, 0.26882026, 0.26869172], [0.5342523, 0.5899976, 0.63354045])),
    (VirtualStock::of("kodak-ultramax-400"), reference([-0.74341524, 0.0, 0.87521887], [0.26637736, 0.2663198, 0.2662007], [0.5171694, 0.56953526, 0.62842077])),
    (VirtualStock::of("kodak-verita-200d"), reference([-0.94407433, 0.0, 0.74446404], [0.29920754, 0.29911914, 0.29899496], [0.56565654, 0.5933309, 0.66580796])),
    (VirtualStock::of("kodak-vision3-200t"), reference([-0.75497735, 0.0, 0.6959914], [0.26693597, 0.2668775, 0.26678857], [0.5265618, 0.5782619, 0.59845513])),
    (VirtualStock::of("kodak-vision3-250d"), reference([-0.6818431, 0.0, 0.80664283], [0.26541534, 0.2653619, 0.26525572], [0.52122426, 0.5731309, 0.5917949])),
    (VirtualStock::of("kodak-vision3-500t"), reference([-0.71135527, 0.0, 0.7303293], [0.26834157, 0.26828513, 0.26818955], [0.5148739, 0.5660705, 0.58229285])),
    (VirtualStock::of("kodak-vision3-50d"), reference([-0.8358752, 0.0, 0.711386], [0.26140577, 0.26134527, 0.2612577], [0.523322, 0.5722243, 0.59109336])),
];

/// The reference a document of format 80 laid for `stock`; a stock this step leaves unmoved, its own.
pub fn scanned(stock: VirtualStock) -> StockReference {
    SCANNED.iter().find(|(s, _)| *s == stock).map_or_else(|| stock.reference(), |(_, r)| *r)
}

/// `settings` of `version` on its stock's reference read off its sheet; one older than format 80 is moved there by the
/// earlier steps, which land on the stock's reference of today.
pub fn caught_up(version: i64, settings: PipelineSettings) -> PipelineSettings {
    match settings.film() {
        Some(film) if (TWO_POINTS_VERSION..MODEL_S_VERSION).contains(&version) => onto(settings, scanned(film.stock), film.stock.reference()),
        _ => settings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConversionMode;
    use crate::migrate::chain::UNIVERSAL_SCANNER_VERSION;

    fn stored(s: &PipelineSettings, version: i64) -> Vec<u8> {
        serde_json::to_vec(&super::super::reshape::as_stored(&crate::Document::of(s.clone()), version)).unwrap()
    }

    fn read(bytes: &[u8]) -> PipelineSettings {
        super::super::document(bytes).unwrap().document.settings
    }

    #[test]
    fn a_stored_photo_of_each_former_format_lands_once_on_its_stocks_reference() {
        let stock = VirtualStock::of("kodak-portra-800");
        let now = stock.reference();
        // The reference each format laid, a hand's move on red's white above it.
        let laid = |r: StockReference| {
            let mut s = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(stock);
            (s.gains.stops, s.levels) = (r.stops, r.levels());
            s.levels.red.white += 0.03;
            s
        };
        let formats = [
            (UNIVERSAL_SCANNER_VERSION - 1, super::super::scanner::formerly_read(stock)),
            (UNIVERSAL_SCANNER_VERSION, super::super::two_points::offset(stock)),
            (TWO_POINTS_VERSION, scanned(stock)),
        ];
        for (version, was) in formats {
            let landed = read(&stored(&laid(was), version));
            assert!((0..3).all(|c| (landed.gains.stops[c] - now.stops[c]).abs() < 1e-5), "format {version}: {:?}", landed.gains.stops);
            assert!((landed.levels.red.white - (now.levels().red.white + 0.03)).abs() < 1e-5, "format {version}: {:?}", landed.levels.red);
            // Twin: the same photo stored as the next format, which laid another reference, does not land there.
            let wrong = read(&stored(&laid(was), version + 1));
            assert!((0..3).any(|c| (wrong.gains.stops[c] - now.stops[c]).abs() > 1e-3) || (wrong.levels.red.white - landed.levels.red.white).abs() > 1e-3, "format {version}: a wrong format lands alike");
        }
    }

    #[test]
    fn a_digital_photo_lands_on_its_stocks_sheet_reading_its_hand_kept() {
        let stock = VirtualStock::of("kodak-portra-800");
        let mut photo = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(stock);
        let (was, now) = (scanned(stock), stock.reference());
        assert!((0..3).any(|c| (was.stops[c] - now.stops[c]).abs() > 0.05), "{was:?} {now:?}");
        photo.gains.stops = was.stops.map(|v| v + 0.25);
        photo.levels = was.levels();
        photo.levels.red.white += 0.03;
        let landed = caught_up(TWO_POINTS_VERSION, photo.clone());
        assert!((0..3).all(|c| (landed.gains.stops[c] - (now.stops[c] + 0.25)).abs() < 1e-5), "{:?}", landed.gains.stops);
        assert!((landed.levels.red.white - (now.levels().red.white + 0.03)).abs() < 1e-5, "{:?}", landed.levels.red);
        // Twins: a photo of this format, or one an earlier step already moved, stays; a black and white film, a slide and
        // a scan move nothing.
        assert_eq!(caught_up(MODEL_S_VERSION, photo.clone()), photo);
        assert_eq!(caught_up(TWO_POINTS_VERSION - 1, photo.clone()), photo);
        for key in ["ilford-hp5-plus", "kodak-ektachrome-e100"] {
            let other = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(VirtualStock::of(key));
            assert_eq!(caught_up(TWO_POINTS_VERSION, other.clone()), other, "{key}");
        }
        let scan = PipelineSettings::rest(ConversionMode::Negative, true);
        assert_eq!(caught_up(TWO_POINTS_VERSION, scan.clone()), scan);
    }
}
