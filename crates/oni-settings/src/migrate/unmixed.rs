//! A digital photo before format 66 stood on its stock's reference with the sheet's curves developed as the dyes' amounts:
//! it moves onto the reference of the curves unmixed, every hand's move kept; its render moves on purpose (owner's call).

use super::chain::UNMIXED_VERSION;
use super::datasheet::onto;
use crate::PipelineSettings;
use crate::virtual_film::{StockReference, VirtualStock};

const fn reference(stops: [f32; 3], black: [f32; 3], white: [f32; 3]) -> StockReference {
    StockReference { stops, black, white }
}

/// Each colour stock with the reference formats 60 to 65 derived, its sheet's curves taken as the dyes' amounts.
const MIXED: [(VirtualStock, StockReference); 48] = {
    [
        (VirtualStock::of("kodak-portra-800"), reference([-0.8467025, 0.0, 1.1942314], [0.2664324, 0.26636896, 0.266185], [0.60928464, 0.7152737, 0.78298473])),
        (VirtualStock::of("kodak-portra-800-push-1"), reference([-0.8467025, 0.0, 1.1942314], [0.2664324, 0.26636896, 0.266185], [0.63157296, 0.72760016, 0.7819255])),
        (VirtualStock::of("kodak-portra-800-push-2"), reference([-0.8467025, 0.0, 1.1942314], [0.2664324, 0.26636896, 0.266185], [0.56868476, 0.6586961, 0.68465805])),
        (VirtualStock::of("kodak-portra-160"), reference([-1.0291739, 0.0, 0.7729276], [0.27216747, 0.2720913, 0.27198547], [0.6228159, 0.72310346, 0.8541058])),
        (VirtualStock::of("kodak-portra-400"), reference([-0.98764074, 0.0, 0.75364494], [0.2759493, 0.27587304, 0.27576756], [0.65476054, 0.75495756, 0.89623237])),
        (VirtualStock::of("kodak-ektar-100"), reference([-0.8861552, 0.0, 0.77332544], [0.27716777, 0.27709648, 0.27698642], [0.675184, 0.7815031, 0.93147725])),
        (VirtualStock::of("kodak-gold-200"), reference([-0.830452, 0.0, 1.0133026], [0.2786753, 0.27860653, 0.27844664], [0.64087546, 0.743152, 0.79781675])),
        (VirtualStock::of("kodak-ultramax-400"), reference([-0.82510906, 0.0, 1.1587664], [0.26436394, 0.26430267, 0.26412937], [0.6067583, 0.71256006, 0.7782918])),
        (VirtualStock::of("kodak-verita-200d"), reference([-1.0199594, 0.0, 0.9399972], [0.29461864, 0.29452857, 0.29436558], [0.71227753, 0.7734161, 0.8850033])),
        (VirtualStock::of("kodak-vision3-50d"), reference([-0.91636825, 0.0, 0.94096464], [0.25927153, 0.2592079, 0.25908357], [0.636564, 0.72591823, 0.7465085])),
        (VirtualStock::of("kodak-vision3-200t"), reference([-0.82578504, 0.0, 0.89388233], [0.26431274, 0.2642514, 0.26413083], [0.62405646, 0.7236301, 0.7633354])),
        (VirtualStock::of("kodak-vision3-250d"), reference([-0.7535622, 0.0, 1.0177397], [0.2626649, 0.26260838, 0.2624662], [0.6228944, 0.7168755, 0.7307595])),
        (VirtualStock::of("kodak-vision3-500t"), reference([-0.77878654, 0.0, 0.9240054], [0.2656662, 0.2656069, 0.26547948], [0.61609286, 0.72874504, 0.70622176])),
        (VirtualStock::of("fujifilm-c200"), reference([-0.83061093, 0.0, 0.9496308], [0.27427864, 0.27421215, 0.2740708], [0.7100866, 0.80541384, 0.82627577])),
        (VirtualStock::of("fujifilm-pro-400h"), reference([-1.1394027, 0.0, 1.0356495], [0.305082, 0.30497682, 0.30477485], [0.7237952, 0.7966877, 0.8200742])),
        (VirtualStock::of("fujifilm-xtra-400"), reference([-0.782707, 0.0, 0.9280978], [0.2685544, 0.26849353, 0.26836243], [0.7469467, 0.85469705, 0.83810484])),
        (VirtualStock::of("fujifilm-160nc"), reference([-0.9830299, 0.0, 0.9446567], [0.27338138, 0.27330685, 0.2731675], [0.70459193, 0.8207795, 1.0357051])),
        (VirtualStock::of("fujifilm-160nl"), reference([-1.0258617, 0.0, 0.88072264], [0.2809332, 0.28085187, 0.28071746], [0.66692007, 0.7865639, 0.9021461])),
        (VirtualStock::of("fujifilm-160npc"), reference([-0.69356275, 0.0, 0.88195366], [0.25589237, 0.25584206, 0.25573105], [0.70791245, 0.78549016, 0.7873607])),
        (VirtualStock::of("fujifilm-160ns"), reference([-1.1013058, 0.0, 0.6771016], [0.2900523, 0.28996074, 0.2898581], [0.69712126, 0.8211872, 0.9409702])),
        (VirtualStock::of("fujifilm-200"), reference([-0.98764145, 0.0, 1.3207269], [0.26841062, 0.26833865, 0.2681215], [0.57176256, 0.7251559, 0.8275577])),
        (VirtualStock::of("fujifilm-400"), reference([-0.98744607, 0.0, 1.3159344], [0.2693434, 0.2692709, 0.26905343], [0.60454845, 0.7682002, 0.8620297])),
        (VirtualStock::of("fujifilm-fujicolor-100"), reference([-0.8553253, 0.0, 0.5461921], [0.26282758, 0.2627654, 0.26270142], [0.7540529, 0.8629871, 0.9425986])),
        (VirtualStock::of("fujifilm-natura-1600"), reference([-0.89495873, 0.0, 0.9772441], [0.2745942, 0.2745238, 0.27437645], [0.7150767, 0.8429984, 0.8390635])),
        (VirtualStock::of("fujifilm-new-pro-400"), reference([-0.9659846, 0.0, 0.82500136], [0.2955389, 0.29545158, 0.29531366], [0.72537076, 0.7859453, 0.85675734])),
        (VirtualStock::of("fujifilm-nexia-400"), reference([-1.1186444, 0.0, 1.0489243], [0.35295036, 0.35280007, 0.35250288], [0.7807977, 0.8590638, 1.0610714])),
        (VirtualStock::of("fujifilm-nexia-a200"), reference([-1.0643239, 0.0, 1.2049339], [0.36077076, 0.36061642, 0.36023104], [0.8076468, 0.892673, 0.9828505])),
        (VirtualStock::of("fujifilm-nexia-zoom-master-800"), reference([-1.060208, 0.0, 1.2241347], [0.3226104, 0.3224957, 0.3222016], [0.74215615, 0.8154189, 0.89253145])),
        (VirtualStock::of("fujifilm-pro-160c"), reference([-1.3470296, 0.0, 0.6656624], [0.3497056, 0.3495408, 0.34938172], [0.757183, 0.84764206, 0.87450624])),
        (VirtualStock::of("fujifilm-pro-160nc"), reference([-1.2484103, 0.0, 0.5905031], [0.33505836, 0.33491787, 0.33479527], [0.74852407, 0.83184606, 0.8884373])),
        (VirtualStock::of("fujifilm-pro-160nh"), reference([-1.3468778, 0.0, 0.6698033], [0.35126054, 0.3510937, 0.3509315], [0.7790135, 0.8370369, 0.86286545])),
        (VirtualStock::of("fujifilm-pro-160ns"), reference([-1.2484103, 0.0, 0.5905031], [0.33505836, 0.33491787, 0.33479527], [0.7229442, 0.77923137, 0.83645236])),
        (VirtualStock::of("fujifilm-pro-160s"), reference([-1.2484833, 0.0, 0.59052044], [0.33505875, 0.33491823, 0.33479562], [0.71236014, 0.78729, 0.8013349])),
        (VirtualStock::of("fujifilm-pro-800"), reference([-0.99509996, 0.0, 1.2042607], [0.29084924, 0.2907633, 0.2905385], [0.73315144, 0.8231449, 0.8414066])),
        (VirtualStock::of("fujifilm-pro-800z"), reference([-1.0227036, 0.0, 1.1428204], [0.29216453, 0.292076, 0.29186568], [0.7162864, 0.79871434, 0.8176887])),
        (VirtualStock::of("fujifilm-reala-ace"), reference([-0.8249654, 0.0, 0.74971867], [0.27130067, 0.271236, 0.27113494], [0.7211622, 0.801655, 0.845042])),
        (VirtualStock::of("fujifilm-superia-100"), reference([-0.87347025, 0.0, 0.5669083], [0.28469506, 0.28462034, 0.28454116], [0.7076585, 0.8131212, 0.8033495])),
        (VirtualStock::of("fujifilm-superia-200"), reference([-0.84955186, 0.0, 0.61150295], [0.2812986, 0.2812272, 0.28114265], [0.77946216, 0.8752912, 0.89661235])),
        (VirtualStock::of("fujifilm-superia-1600"), reference([-0.89496434, 0.0, 0.9771375], [0.2746378, 0.27456737, 0.27442], [0.7151464, 0.8430934, 0.8391708])),
        (VirtualStock::of("fujifilm-superia-premium-400"), reference([-0.7838326, 0.0, 0.9296579], [0.26892984, 0.26886877, 0.268737], [0.7731065, 0.87261534, 0.86194336])),
        (VirtualStock::of("fujifilm-superia-reala"), reference([-0.72140634, 0.0, 0.90098035], [0.28289422, 0.2828304, 0.28268972], [0.68116415, 0.80632806, 0.7574748])),
        (VirtualStock::of("fujifilm-superia-venus-400"), reference([-0.7705156, 0.0, 0.86905223], [0.26798925, 0.2679294, 0.2678099], [0.7237674, 0.85133123, 0.84807336])),
        (VirtualStock::of("fujifilm-superia-venus-800"), reference([-0.8487747, 0.0, 0.8897301], [0.27266905, 0.27260232, 0.2724745], [0.7130627, 0.8329747, 0.85304165])),
        (VirtualStock::of("fujifilm-xtra-800"), reference([-1.1181284, 0.0, 1.1705339], [0.3584864, 0.35832962, 0.35796675], [0.8199728, 0.94027627, 1.1021227])),
        (VirtualStock::of("kodak-portra-160nc"), reference([-0.55807227, 0.0, 0.43169335], [0.31164914, 0.31158417, 0.3115135], [0.9687787, 1.0826732, 0.97218776])),
        (VirtualStock::of("kodak-portra-160vc"), reference([-0.565028, 0.0, 0.5502797], [0.30715853, 0.30709514, 0.30700427], [1.0041873, 1.1008996, 0.99254084])),
        (VirtualStock::of("kodak-portra-400nc"), reference([-0.5557344, 0.0, 0.5149134], [0.30773997, 0.3076771, 0.3075928], [0.9609211, 1.0723351, 0.9655603])),
        (VirtualStock::of("kodak-portra-400vc"), reference([-0.5713503, 0.0, 0.5229958], [0.30710933, 0.30704534, 0.3069599], [0.98488104, 1.0995827, 0.9914603])),
    ]
};

/// The reference a document of formats 60 to 65 laid for `stock`; a black and white film's never moved.
pub fn mixed(stock: VirtualStock) -> StockReference {
    MIXED.iter().find(|(s, _)| *s == stock).map_or_else(|| super::layer_densities::scanned(stock), |(_, r)| *r)
}

/// `settings` of `version` on its stock's reference of the unmixed curves, as formats 66 to 68 read them.
pub fn caught_up(version: i64, settings: PipelineSettings) -> PipelineSettings {
    match settings.film() {
        Some(film) if version < UNMIXED_VERSION => onto(settings, mixed(film.stock), super::layer_densities::scanned(film.stock)),
        _ => settings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConversionMode;
    use crate::document::VERSION;

    fn read(s: &PipelineSettings, version: i64) -> PipelineSettings {
        let stored = super::super::reshape::as_stored(&crate::Document::of(s.clone()), version);
        super::super::document(&serde_json::to_vec(&stored).unwrap()).unwrap().document.settings
    }

    fn on_mixed(stock: VirtualStock) -> PipelineSettings {
        let mut s = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(stock);
        (s.gains.stops, s.levels) = (mixed(stock).stops, mixed(stock).levels());
        s
    }

    #[test]
    fn a_digital_photo_on_the_mixed_reference_lands_on_the_unmixed_one_its_hand_kept() {
        let older = UNMIXED_VERSION - 1;
        for stock in [VirtualStock::of("kodak-portra-800"), VirtualStock::of("fujifilm-superia-reala")] {
            let own = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(stock);
            let landed = read(&on_mixed(stock), older);
            assert_eq!((landed.gains, landed.levels), (own.gains, own.levels), "{stock:?}: a reference laid whole lands whole");
            // Nothing else moves but the film effects, which format 75 lays on the film's rests.
            let (rests, stored) = (stock.effects(), on_mixed(stock));
            let effects = PipelineSettings { halation: rests.halation, bloom: rests.bloom, ..stored };
            assert_eq!(PipelineSettings { gains: own.gains, levels: own.levels, ..effects }, landed, "{stock:?}: nothing else moves");
            let mut hand = on_mixed(stock);
            (hand.gains.stops[0], hand.levels.blue.white) = (hand.gains.stops[0] - 0.2, hand.levels.blue.white + 0.05);
            let kept = read(&hand, older);
            assert!((kept.gains.stops[0] - own.gains.stops[0] + 0.2).abs() < 1e-6, "{:?}", kept.gains);
            assert!((kept.levels.blue.white - own.levels.blue.white - 0.05).abs() < 1e-6, "{:?}", kept.levels.blue);
            // Twin: the reference really moved.
            assert_ne!(mixed(stock).white, stock.reference().white, "{stock:?}");
        }
        // Twins: a file of this format, a black and white film and a scan stay as they are.
        let current = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(VirtualStock::of("kodak-portra-800"));
        assert_eq!(read(&current, VERSION), current);
        // A black and white film's reference this step leaves; the sensor's saturation's step lands it on its own.
        let hp5 = VirtualStock::of("ilford-hp5-plus");
        let silver = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(hp5);
        let then = PipelineSettings { levels: super::super::sensor_white::grey_placed(hp5).levels(), ..silver.clone() };
        assert_eq!(read(&then, older), silver.picking(hp5), "format 75 lays the film's effects");
        let scan = PipelineSettings::rest(ConversionMode::Negative, true);
        assert_eq!(read(&scan, older), read(&scan, VERSION));
    }
}
