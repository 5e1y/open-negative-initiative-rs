//! Before format 83 a colour negative's virtual film developed on its data sheet's curves, its couplers derived from the
//! sheet alone; its curves and couplers are fitted now on a reference the owner chose. A digital photo moves onto its
//! stock's new reference, every hand's move kept.

use super::chain::{MODEL_S_VERSION, REFITTED_VERSION};
use super::datasheet::onto;
use crate::PipelineSettings;
use crate::virtual_film::{StockReference, VirtualStock};

const fn reference(stops: [f32; 3], black: [f32; 3], white: [f32; 3]) -> StockReference {
    StockReference { stops, black, white }
}

/// Each colour negative whose reference moved, as format 81 laid it.
const SHEETED: [(VirtualStock, StockReference); 48] = [
    (VirtualStock::of("fujifilm-160nc"), reference([-1.3792799, 0.0, 0.7861345], [0.29142445, 0.29131776, 0.2911923], [0.6643193, 0.6878947, 0.81421727])),
    (VirtualStock::of("fujifilm-160nl"), reference([-1.4712143, 0.0, 0.73505986], [0.29717758, 0.29706177, 0.2969415], [0.6480077, 0.68786746, 0.7627669])),
    (VirtualStock::of("fujifilm-160npc"), reference([-1.0019999, 0.0, 0.7992498], [0.26814178, 0.26806924, 0.2679621], [0.6735468, 0.66483474, 0.6346908])),
    (VirtualStock::of("fujifilm-160ns"), reference([-1.6073362, 0.0, 0.49210146], [0.30878958, 0.30865645, 0.308576], [0.6572961, 0.67324674, 0.74458814])),
    (VirtualStock::of("fujifilm-200"), reference([-1.6278199, 0.0, 1.291325], [0.28103825, 0.28093004, 0.2806988], [0.6086984, 0.66858, 0.6576606])),
    (VirtualStock::of("fujifilm-400"), reference([-1.6299585, 0.0, 1.2937372], [0.28164223, 0.28153345, 0.28130046], [0.6327297, 0.6905727, 0.66212237])),
    (VirtualStock::of("fujifilm-c200"), reference([-1.1932906, 0.0, 0.82381696], [0.2886855, 0.28859, 0.28845945], [0.6500412, 0.678772, 0.6997228])),
    (VirtualStock::of("fujifilm-fujicolor-100"), reference([-1.3735836, 0.0, 0.4488362], [0.2749478, 0.27485406, 0.27479842], [0.7212043, 0.73071754, 0.7825066])),
    (VirtualStock::of("fujifilm-natura-1600"), reference([-1.3223323, 0.0, 0.8191301], [0.29236692, 0.29226214, 0.2921288], [0.69111145, 0.752855, 0.7356154])),
    (VirtualStock::of("fujifilm-new-pro-400"), reference([-1.4247342, 0.0, 0.65678394], [0.31351474, 0.31338578, 0.31326744], [0.73438126, 0.693772, 0.7082674])),
    (VirtualStock::of("fujifilm-nexia-a200"), reference([-1.3628769, 0.0, 0.962039], [0.38858294, 0.3883589, 0.38801217], [0.7867055, 0.81025016, 0.8463531])),
    (VirtualStock::of("fujifilm-nexia-zoom-master-800"), reference([-1.3996706, 0.0, 0.95962954], [0.35039377, 0.3502242, 0.3499666], [0.7720116, 0.7732918, 0.7832751])),
    (VirtualStock::of("fujifilm-nexia-400"), reference([-1.4713197, 0.0, 0.7856557], [0.38152787, 0.38130593, 0.38105515], [0.806038, 0.824712, 0.828783])),
    (VirtualStock::of("fujifilm-pro-160c"), reference([-2.000622, 0.0, 0.38605812], [0.37624553, 0.37599555, 0.37589344], [0.75401986, 0.7709358, 0.787115])),
    (VirtualStock::of("fujifilm-pro-160nc"), reference([-1.8809433, 0.0, 0.35680497], [0.35776827, 0.3575577, 0.3574767], [0.7536094, 0.7502436, 0.778559])),
    (VirtualStock::of("fujifilm-pro-160nh"), reference([-2.000196, 0.0, 0.38750622], [0.3778727, 0.37761962, 0.3775158], [0.78803176, 0.749753, 0.7674807])),
    (VirtualStock::of("fujifilm-pro-160ns"), reference([-1.8809433, 0.0, 0.35680497], [0.35776827, 0.3575577, 0.3574767], [0.7310284, 0.69807595, 0.7287279])),
    (VirtualStock::of("fujifilm-pro-160s"), reference([-1.8810931, 0.0, 0.35679382], [0.3577708, 0.35756022, 0.35747924], [0.70245045, 0.7062221, 0.71145844])),
    (VirtualStock::of("fujifilm-pro-400h"), reference([-1.726871, 0.0, 0.8866628], [0.32161018, 0.32145754, 0.32127213], [0.69619304, 0.68267655, 0.6748518])),
    (VirtualStock::of("fujifilm-pro-800"), reference([-1.4761916, 0.0, 1.076656], [0.30600044, 0.3058762, 0.30566138], [0.725608, 0.7095791, 0.690498])),
    (VirtualStock::of("fujifilm-pro-800z"), reference([-1.5104349, 0.0, 1.0074133], [0.3083764, 0.30824822, 0.3080489], [0.7315104, 0.70943934, 0.67879707])),
    (VirtualStock::of("fujifilm-reala-ace"), reference([-1.2690433, 0.0, 0.64141136], [0.28537396, 0.28527716, 0.2851846], [0.7052544, 0.6969519, 0.73148084])),
    (VirtualStock::of("fujifilm-superia-premium-400"), reference([-1.1137832, 0.0, 0.81565183], [0.28200915, 0.28192243, 0.28180003], [0.71374416, 0.7089661, 0.7335992])),
    (VirtualStock::of("fujifilm-superia-reala"), reference([-1.1346924, 0.0, 0.8648379], [0.2900187, 0.28992534, 0.28978467], [0.6474634, 0.6861946, 0.6556374])),
    (VirtualStock::of("fujifilm-superia-venus-400"), reference([-1.0992037, 0.0, 0.75454164], [0.28102145, 0.28093615, 0.2808263], [0.65827394, 0.7113078, 0.72510004])),
    (VirtualStock::of("fujifilm-superia-venus-800"), reference([-1.2171005, 0.0, 0.73600084], [0.28959256, 0.28949514, 0.28938153], [0.7095271, 0.74951404, 0.74137926])),
    (VirtualStock::of("fujifilm-xtra-400"), reference([-1.1123011, 0.0, 0.81458044], [0.28159305, 0.2815067, 0.2813849], [0.6645847, 0.6983946, 0.711174])),
    (VirtualStock::of("fujifilm-xtra-800"), reference([-1.597177, 0.0, 0.9625207], [0.3835957, 0.3833596, 0.38302577], [0.82020944, 0.85791194, 0.866549])),
    (VirtualStock::of("fujifilm-superia-100"), reference([-1.3135004, 0.0, 0.43891886], [0.29736584, 0.29725736, 0.29719293], [0.68133533, 0.7292905, 0.7453986])),
    (VirtualStock::of("fujifilm-superia-1600"), reference([-1.3223146, 0.0, 0.8190294], [0.29241094, 0.29230613, 0.29217276], [0.6911553, 0.75292915, 0.73569226])),
    (VirtualStock::of("fujifilm-superia-200"), reference([-1.3810236, 0.0, 0.48828283], [0.29497826, 0.29486853, 0.29479682], [0.7147266, 0.7459794, 0.73713815])),
    (VirtualStock::of("kodak-ektar-100"), reference([-1.3283683, 0.0, 0.6989243], [0.2884103, 0.28830838, 0.2882029], [0.6514724, 0.6598385, 0.744687])),
    (VirtualStock::of("kodak-gold-200"), reference([-1.2797741, 0.0, 0.96080315], [0.29314482, 0.2930415, 0.29287547], [0.6379265, 0.6588083, 0.67230463])),
    (VirtualStock::of("kodak-portra-160"), reference([-1.4879699, 0.0, 0.5957119], [0.29058945, 0.29047865, 0.29039067], [0.6313577, 0.6508943, 0.7076992])),
    (VirtualStock::of("kodak-portra-160nc"), reference([-1.3736078, 0.0, 0.41744423], [0.31498957, 0.31486192, 0.31479222], [0.66196364, 0.67724514, 0.75516224])),
    (VirtualStock::of("kodak-portra-160vc"), reference([-1.3606787, 0.0, 0.53756595], [0.31073585, 0.310613, 0.3105223], [0.6698621, 0.684068, 0.7671856])),
    (VirtualStock::of("kodak-portra-400"), reference([-1.4159338, 0.0, 0.59378445], [0.29157928, 0.29147077, 0.2913825], [0.6383229, 0.65336794, 0.7295403])),
    (VirtualStock::of("kodak-portra-400nc"), reference([-1.3674496, 0.0, 0.50528044], [0.3113134, 0.31118968, 0.311105], [0.6580239, 0.673111, 0.74926144])),
    (VirtualStock::of("kodak-portra-400vc"), reference([-1.3922976, 0.0, 0.5040005], [0.3114536, 0.31132838, 0.31124386], [0.6748412, 0.68759805, 0.7687631])),
    (VirtualStock::of("kodak-portra-800"), reference([-1.2376142, 0.0, 1.1220267], [0.28075016, 0.28065822, 0.2804706], [0.60612756, 0.63429236, 0.6627823])),
    (VirtualStock::of("kodak-portra-800-push-1"), reference([-1.2376142, 0.0, 1.1220267], [0.28075016, 0.28065822, 0.2804706], [0.6498678, 0.6687885, 0.6926611])),
    (VirtualStock::of("kodak-portra-800-push-2"), reference([-1.2376142, 0.0, 1.1220267], [0.28075016, 0.28065822, 0.2804706], [0.6252563, 0.658376, 0.65815806])),
    (VirtualStock::of("kodak-ultramax-400"), reference([-1.1943938, 0.0, 1.0827296], [0.27989948, 0.27981016, 0.27963305], [0.6047238, 0.6332909, 0.66053927])),
    (VirtualStock::of("kodak-verita-200d"), reference([-1.4364593, 0.0, 0.7845882], [0.311969, 0.31184098, 0.31169435], [0.6558714, 0.63527346, 0.69084996])),
    (VirtualStock::of("kodak-vision3-200t"), reference([-1.3039414, 0.0, 0.8445374], [0.2724119, 0.2723229, 0.2722039], [0.584204, 0.63273275, 0.61821824])),
    (VirtualStock::of("kodak-vision3-250d"), reference([-1.1358682, 0.0, 0.99280095], [0.26910377, 0.26902422, 0.2688799], [0.5758388, 0.6234414, 0.610686])),
    (VirtualStock::of("kodak-vision3-500t"), reference([-1.2924234, 0.0, 0.9274828], [0.26946574, 0.26937914, 0.26924723], [0.56463325, 0.61318266, 0.6011504])),
    (VirtualStock::of("kodak-vision3-50d"), reference([-1.3795171, 0.0, 0.8667354], [0.27086514, 0.2707741, 0.27065235], [0.5844037, 0.63136506, 0.61853534])),
];

/// The reference a document of format 81 or 82 laid for `stock`; a stock this step leaves unmoved, its own.
pub fn sheeted(stock: VirtualStock) -> StockReference {
    SHEETED.iter().find(|(s, _)| *s == stock).map_or_else(|| stock.reference(), |(_, r)| *r)
}

/// `settings` of `version` on its stock's refitted reference; one older than format 81 is moved there by the earlier
/// steps, which land on the stock's reference of today.
pub fn caught_up(version: i64, settings: PipelineSettings) -> PipelineSettings {
    match settings.film() {
        Some(film) if (MODEL_S_VERSION..REFITTED_VERSION).contains(&version) => onto(settings, sheeted(film.stock), film.stock.reference()),
        _ => settings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConversionMode;

    fn read(s: &PipelineSettings, version: i64) -> PipelineSettings {
        let stored = super::super::reshape::as_stored(&crate::Document::of(s.clone()), version);
        super::super::document(&serde_json::to_vec(&stored).unwrap()).unwrap().document.settings
    }

    #[test]
    fn a_photo_of_format_81_or_82_lands_on_its_stocks_refitted_reference_its_hand_kept() {
        for (stock, was) in SHEETED {
            let now = stock.reference();
            assert_ne!(was, now, "{stock:?} moved");
            let mut stored = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(stock);
            (stored.gains.stops, stored.levels) = (was.stops, was.levels());
            for version in [MODEL_S_VERSION, REFITTED_VERSION - 1] {
                let landed = read(&stored, version);
                assert_eq!((landed.gains.stops, landed.levels), (now.stops, now.levels()), "{stock:?} at {version}");
            }
            // A hand's move on the gains stays a move.
            let mut hand = stored.clone();
            hand.gains.stops[2] += 0.25;
            let kept = read(&hand, MODEL_S_VERSION);
            assert!((kept.gains.stops[2] - now.stops[2] - 0.25).abs() < 1e-5, "{stock:?}: {:?}", kept.gains);
            // Twin: a document of this format reads as it stands.
            assert_eq!(read(&stored, REFITTED_VERSION).gains, stored.gains);
        }
        // Twin: a scanned negative stays as it is.
        let scan = PipelineSettings::rest(ConversionMode::Negative, true);
        assert_eq!(read(&scan, MODEL_S_VERSION).gains, scan.gains);
    }
}
