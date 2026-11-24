//! A digital photo before format 69 stood on its stock's reference with the virtual negative read through its dyes by a
//! camera: it moves onto the reference of each layer's density coded on its own channel, every hand's move kept; its
//! render moves on purpose (owner's call).

use super::chain::LAYER_DENSITIES_VERSION;
use super::datasheet::onto;
use crate::PipelineSettings;
use crate::virtual_film::{StockReference, VirtualStock};

const fn reference(stops: [f32; 3], black: [f32; 3], white: [f32; 3]) -> StockReference {
    StockReference { stops, black, white }
}

/// Each stock with the reference formats 66 to 68 derived, its negative read through its dyes by the camera.
const SCANNED: [(VirtualStock, StockReference); 75] = {
    [
        (VirtualStock::of("kodak-portra-800"), reference([-0.8467025, 0.0, 1.1942314], [0.2664324, 0.26636896, 0.266185], [0.57385015, 0.65379053, 0.72411275])),
        (VirtualStock::of("kodak-portra-800-push-1"), reference([-0.8467025, 0.0, 1.1942314], [0.2664324, 0.26636896, 0.266185], [0.5951463, 0.66220385, 0.720267])),
        (VirtualStock::of("kodak-portra-800-push-2"), reference([-0.8467025, 0.0, 1.1942314], [0.2664324, 0.26636896, 0.266185], [0.53791285, 0.60454583, 0.6328216])),
        (VirtualStock::of("kodak-portra-160"), reference([-1.0291739, 0.0, 0.7729276], [0.27216747, 0.2720913, 0.27198547], [0.58495903, 0.6544115, 0.7818025])),
        (VirtualStock::of("kodak-portra-400"), reference([-0.98764074, 0.0, 0.75364494], [0.2759493, 0.27587304, 0.27576756], [0.6090294, 0.67259055, 0.8307925])),
        (VirtualStock::of("kodak-ektar-100"), reference([-0.8861552, 0.0, 0.77332544], [0.27716777, 0.27709648, 0.27698642], [0.63018304, 0.69412404, 0.8497187])),
        (VirtualStock::of("kodak-gold-200"), reference([-0.830452, 0.0, 1.0133026], [0.2786753, 0.27860653, 0.27844664], [0.6040813, 0.6759063, 0.73893756])),
        (VirtualStock::of("kodak-ultramax-400"), reference([-0.82510906, 0.0, 1.1587664], [0.26436394, 0.26430267, 0.26412937], [0.57112956, 0.648374, 0.7212143])),
        (VirtualStock::of("kodak-verita-200d"), reference([-1.0199594, 0.0, 0.9399972], [0.29461864, 0.29452857, 0.29436558], [0.65531194, 0.6866728, 0.75721455])),
        (VirtualStock::of("kodak-vision3-50d"), reference([-0.91636825, 0.0, 0.94096464], [0.25927153, 0.2592079, 0.25908357], [0.5892993, 0.6541842, 0.6831387])),
        (VirtualStock::of("kodak-vision3-200t"), reference([-0.82578504, 0.0, 0.89388233], [0.26431274, 0.2642514, 0.26413083], [0.5918622, 0.66147536, 0.69205135])),
        (VirtualStock::of("kodak-vision3-250d"), reference([-0.7535622, 0.0, 1.0177397], [0.2626649, 0.26260838, 0.2624662], [0.5803882, 0.64968544, 0.67921406])),
        (VirtualStock::of("kodak-vision3-500t"), reference([-0.77878654, 0.0, 0.9240054], [0.2656662, 0.2656069, 0.26547948], [0.5724703, 0.64309555, 0.671298])),
        (VirtualStock::of("fujifilm-c200"), reference([-0.83061093, 0.0, 0.9496308], [0.27427864, 0.27421215, 0.2740708], [0.64410913, 0.70046955, 0.7577055])),
        (VirtualStock::of("fujifilm-pro-400h"), reference([-1.1394027, 0.0, 1.0356495], [0.305082, 0.30497682, 0.30477485], [0.6734493, 0.7103099, 0.7457083])),
        (VirtualStock::of("fujifilm-xtra-400"), reference([-0.782707, 0.0, 0.9280978], [0.2685544, 0.26849353, 0.26836243], [0.67764014, 0.73257005, 0.766886])),
        (VirtualStock::of("fujifilm-160nc"), reference([-0.9830299, 0.0, 0.9446567], [0.27338138, 0.27330685, 0.2731675], [0.64644915, 0.7188178, 0.9559797])),
        (VirtualStock::of("fujifilm-160nl"), reference([-1.0258617, 0.0, 0.88072264], [0.2809332, 0.28085187, 0.28071746], [0.61696494, 0.6893342, 0.8398999])),
        (VirtualStock::of("fujifilm-160npc"), reference([-0.69356275, 0.0, 0.88195366], [0.25589237, 0.25584206, 0.25573105], [0.6553372, 0.69170797, 0.7153741])),
        (VirtualStock::of("fujifilm-160ns"), reference([-1.1013058, 0.0, 0.6771016], [0.2900523, 0.28996074, 0.2898581], [0.6394005, 0.707842, 0.8684795])),
        (VirtualStock::of("fujifilm-200"), reference([-0.98764145, 0.0, 1.3207269], [0.26841062, 0.26833865, 0.2681215], [0.5365795, 0.6401868, 0.6929141])),
        (VirtualStock::of("fujifilm-400"), reference([-0.98744607, 0.0, 1.3159344], [0.2693434, 0.2692709, 0.26905343], [0.56718695, 0.67985857, 0.71471524])),
        (VirtualStock::of("fujifilm-fujicolor-100"), reference([-0.8553253, 0.0, 0.5461921], [0.26282758, 0.2627654, 0.26270142], [0.69335103, 0.7611767, 0.8521906])),
        (VirtualStock::of("fujifilm-natura-1600"), reference([-0.89495873, 0.0, 0.9772441], [0.2745942, 0.2745238, 0.27437645], [0.6612302, 0.7591614, 0.7650651])),
        (VirtualStock::of("fujifilm-new-pro-400"), reference([-0.9659846, 0.0, 0.82500136], [0.2955389, 0.29545158, 0.29531366], [0.6801367, 0.7054779, 0.77767396])),
        (VirtualStock::of("fujifilm-nexia-400"), reference([-1.1186444, 0.0, 1.0489243], [0.35295036, 0.35280007, 0.35250288], [0.73805636, 0.80672616, 0.8723476])),
        (VirtualStock::of("fujifilm-nexia-a200"), reference([-1.0643239, 0.0, 1.2049339], [0.36077076, 0.36061642, 0.36023104], [0.7564968, 0.80627173, 0.9123062])),
        (VirtualStock::of("fujifilm-nexia-zoom-master-800"), reference([-1.060208, 0.0, 1.2241347], [0.3226104, 0.3224957, 0.3222016], [0.7004337, 0.74900055, 0.80266106])),
        (VirtualStock::of("fujifilm-pro-160c"), reference([-1.3470296, 0.0, 0.6656624], [0.3497056, 0.3495408, 0.34938172], [0.7077327, 0.7545884, 0.84157795])),
        (VirtualStock::of("fujifilm-pro-160nc"), reference([-1.2484103, 0.0, 0.5905031], [0.33505836, 0.33491787, 0.33479527], [0.7006667, 0.7412388, 0.8387661])),
        (VirtualStock::of("fujifilm-pro-160nh"), reference([-1.3468778, 0.0, 0.6698033], [0.35126054, 0.3510937, 0.3509315], [0.72994417, 0.7379483, 0.8283898])),
        (VirtualStock::of("fujifilm-pro-160ns"), reference([-1.2484103, 0.0, 0.5905031], [0.33505836, 0.33491787, 0.33479527], [0.67940396, 0.6918014, 0.78970337])),
        (VirtualStock::of("fujifilm-pro-160s"), reference([-1.2484833, 0.0, 0.59052044], [0.33505875, 0.33491823, 0.33479562], [0.6692225, 0.70564353, 0.75640184])),
        (VirtualStock::of("fujifilm-pro-800"), reference([-0.99509996, 0.0, 1.2042607], [0.29084924, 0.2907633, 0.2905385], [0.68136615, 0.73732394, 0.7561798])),
        (VirtualStock::of("fujifilm-pro-800z"), reference([-1.0227036, 0.0, 1.1428204], [0.29216453, 0.292076, 0.29186568], [0.671126, 0.7200098, 0.73973036])),
        (VirtualStock::of("fujifilm-reala-ace"), reference([-0.8249654, 0.0, 0.74971867], [0.27130067, 0.271236, 0.27113494], [0.67152333, 0.7096726, 0.77254534])),
        (VirtualStock::of("fujifilm-superia-100"), reference([-0.87347025, 0.0, 0.5669083], [0.28469506, 0.28462034, 0.28454116], [0.63838637, 0.6988484, 0.7464876])),
        (VirtualStock::of("fujifilm-superia-200"), reference([-0.84955186, 0.0, 0.61150295], [0.2812986, 0.2812272, 0.28114265], [0.69175124, 0.744949, 0.7652797])),
        (VirtualStock::of("fujifilm-superia-1600"), reference([-0.89496434, 0.0, 0.9771375], [0.2746378, 0.27456737, 0.27442], [0.6612944, 0.75925225, 0.7651675])),
        (VirtualStock::of("fujifilm-superia-premium-400"), reference([-0.7838326, 0.0, 0.9296579], [0.26892984, 0.26886877, 0.268737], [0.70088303, 0.74364537, 0.78759575])),
        (VirtualStock::of("fujifilm-superia-reala"), reference([-0.72140634, 0.0, 0.90098035], [0.28289422, 0.2828304, 0.28268972], [0.6181936, 0.700884, 0.66580355])),
        (VirtualStock::of("fujifilm-superia-venus-400"), reference([-0.7705156, 0.0, 0.86905223], [0.26798925, 0.2679294, 0.2678099], [0.6571573, 0.7389691, 0.77597964])),
        (VirtualStock::of("fujifilm-superia-venus-800"), reference([-0.8487747, 0.0, 0.8897301], [0.27266905, 0.27260232, 0.2724745], [0.6658727, 0.7519495, 0.7893201])),
        (VirtualStock::of("fujifilm-xtra-800"), reference([-1.1181284, 0.0, 1.1705339], [0.3584864, 0.35832962, 0.35796675], [0.7595167, 0.8454484, 0.92107224])),
        (VirtualStock::of("kodak-portra-160nc"), reference([-0.55807227, 0.0, 0.43169335], [0.31164914, 0.31158417, 0.3115135], [0.74749005, 0.78080827, 0.87925315])),
        (VirtualStock::of("kodak-portra-160vc"), reference([-0.565028, 0.0, 0.5502797], [0.30715853, 0.30709514, 0.30700427], [0.7589101, 0.7894718, 0.8927779])),
        (VirtualStock::of("kodak-portra-400nc"), reference([-0.5557344, 0.0, 0.5149134], [0.30773997, 0.3076771, 0.3075928], [0.7414715, 0.77481437, 0.87066925])),
        (VirtualStock::of("kodak-portra-400vc"), reference([-0.5713503, 0.0, 0.5229958], [0.30710933, 0.30704534, 0.3069599], [0.75755215, 0.79070425, 0.8924931])),
        (VirtualStock::of("foma-fomapan-100"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.641245, 0.641245, 0.641245])),
        (VirtualStock::of("foma-fomapan-200"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.6508648, 0.6508648, 0.6508648])),
        (VirtualStock::of("foma-fomapan-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.6400596, 0.6400596, 0.6400596])),
        (VirtualStock::of("foma-fomapan-cine-100"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.64884645, 0.64884645, 0.64884645])),
        (VirtualStock::of("foma-fomapan-cine-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.6525161, 0.6525161, 0.6525161])),
        (VirtualStock::of("foma-ortho-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.6923136, 0.6923136, 0.6923136])),
        (VirtualStock::of("foma-cine-ortho-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.70480514, 0.70480514, 0.70480514])),
        (VirtualStock::of("foma-retropan-320"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4840337, 0.4840337, 0.4840337])),
        (VirtualStock::of("fujifilm-acros-ii"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.55640346, 0.55640346, 0.55640346])),
        (VirtualStock::of("fujifilm-neopan-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5011924, 0.5011924, 0.5011924])),
        (VirtualStock::of("fujifilm-neopan-1600"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5704813, 0.5704813, 0.5704813])),
        (VirtualStock::of("fujifilm-neopan-ortho"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5318679, 0.5318679, 0.5318679])),
        (VirtualStock::of("ilford-delta-100"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5453998, 0.5453998, 0.5453998])),
        (VirtualStock::of("ilford-delta-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5495109, 0.5495109, 0.5495109])),
        (VirtualStock::of("ilford-delta-3200"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.50518507, 0.50518507, 0.50518507])),
        (VirtualStock::of("ilford-fp4-plus"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5620778, 0.5620778, 0.5620778])),
        (VirtualStock::of("ilford-hp5-plus"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.529545, 0.529545, 0.529545])),
        (VirtualStock::of("ilford-ortho-plus"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5154966, 0.5154966, 0.5154966])),
        (VirtualStock::of("ilford-pan-f-plus"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.59475046, 0.59475046, 0.59475046])),
        (VirtualStock::of("ilford-sfx-200"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5630885, 0.5630885, 0.5630885])),
        (VirtualStock::of("ilford-xp2-super"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.58853966, 0.58853966, 0.58853966])),
        (VirtualStock::of("kodak-double-x"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5464296, 0.5464296, 0.5464296])),
        (VirtualStock::of("kodak-tmax-100"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5124685, 0.5124685, 0.5124685])),
        (VirtualStock::of("kodak-tmax-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.47859788, 0.47859788, 0.47859788])),
        (VirtualStock::of("kodak-tmax-p3200"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.65690064, 0.65690064, 0.65690064])),
        (VirtualStock::of("kodak-tri-x-320"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.49478894, 0.49478894, 0.49478894])),
        (VirtualStock::of("kodak-tri-x-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5753634, 0.5753634, 0.5753634])),
    ]
};

/// The reference a document of formats 66 to 68 laid for `stock`.
pub fn scanned(stock: VirtualStock) -> StockReference {
    SCANNED.iter().find(|(s, _)| *s == stock).map_or_else(|| super::digital_scan::layered(stock), |(_, r)| *r)
}

/// `settings` of `version` on its stock's reference of the layers' own densities.
pub fn caught_up(version: i64, settings: PipelineSettings) -> PipelineSettings {
    match settings.film() {
        Some(film) if version < LAYER_DENSITIES_VERSION => onto(settings, scanned(film.stock), super::digital_scan::layered(film.stock)),
        _ => settings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConversionMode;

    #[test]
    fn a_digital_photo_on_the_scanned_reference_lands_on_the_layers_own_its_hand_kept() {
        let stock = VirtualStock::of("kodak-portra-800");
        let mut photo = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(stock);
        let was = scanned(stock);
        photo.gains.stops = was.stops.map(|v| v + 0.25);
        photo.levels = was.levels();
        photo.levels.red.white += 0.05;
        let landed = caught_up(LAYER_DENSITIES_VERSION - 1, photo.clone());
        let now = super::super::digital_scan::layered(stock);
        assert!((0..3).all(|c| (landed.gains.stops[c] - (now.stops[c] + 0.25)).abs() < 1e-6), "{:?}", landed.gains.stops);
        assert!((landed.levels.red.white - (now.levels().red.white + 0.05)).abs() < 1e-5, "{:?}", landed.levels.red);
        // Twin: a photo of this format stays where it stands.
        assert_eq!(caught_up(LAYER_DENSITIES_VERSION, photo.clone()).gains, photo.gains);
        // A scan moves nothing.
        let scan = PipelineSettings::rest(ConversionMode::Negative, true);
        assert_eq!(caught_up(LAYER_DENSITIES_VERSION - 1, scan.clone()).levels, scan.levels);
    }
}
