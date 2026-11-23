//! Every virtual stock's name, catalogue place, dyes, profile, light and reference, in `VirtualStock::all()`'s order.

use super::{Balance, StockReference, VirtualStock};
use crate::dyes::DyeSet;

/// What a stock is: its name, its place in the film catalogue, its dyes, its profile and its light.
pub(super) struct Entry {
    pub(super) name: &'static str,
    pub(super) catalogue: &'static str,
    pub(super) dyes: DyeSet,
    pub(super) profile: &'static str,
    pub(super) balance: Balance,
    pub(super) reference: StockReference,
}

const fn entry(name: &'static str, catalogue: &'static str, dyes: DyeSet, profile: &'static str, balance: Balance, reference: StockReference) -> Entry {
    Entry { name, catalogue, dyes, profile, balance, reference }
}

const fn reference(stops: [f32; 3], black: [f32; 3], white: [f32; 3]) -> StockReference {
    StockReference { stops, black, white }
}

/// Every stock in `VirtualStock::all()`'s order, which is the enum's. A push develops Portra 800's own dyes on its curves; a
/// black and white film rests on the generic set, as a black and white roll does: its silver unmixes nothing.
pub(super) const ENTRIES: [Entry; VirtualStock::all().len()] = {
    use Balance::*;
    [
        entry("Kodak Portra 800", "kodak-portra800", DyeSet::of("kodak-portra-800"), "kodak_portra_800", Daylight, reference(
            [-0.8467025, 0.0, 1.1942314], [0.2664324, 0.26636896, 0.266185], [0.57385015, 0.65379053, 0.72411275])),
        entry("Kodak Portra 800 (Push 1)", "kodak-portra800-push1", DyeSet::of("kodak-portra-800"), "kodak_portra_800_push1", Daylight, reference(
            [-0.8467025, 0.0, 1.1942314], [0.2664324, 0.26636896, 0.266185], [0.5951463, 0.66220385, 0.720267])),
        entry("Kodak Portra 800 (Push 2)", "kodak-portra800-push2", DyeSet::of("kodak-portra-800"), "kodak_portra_800_push2", Daylight, reference(
            [-0.8467025, 0.0, 1.1942314], [0.2664324, 0.26636896, 0.266185], [0.53791285, 0.60454583, 0.6328216])),
        entry("Kodak Portra 160", "kodak-portra160", DyeSet::of("kodak-portra-160"), "kodak_portra_160", Daylight, reference(
            [-1.0291739, 0.0, 0.7729276], [0.27216747, 0.2720913, 0.27198547], [0.58495903, 0.6544115, 0.7818025])),
        entry("Kodak Portra 400", "kodak-portra400", DyeSet::of("kodak-portra-400"), "kodak_portra_400", Daylight, reference(
            [-0.98764074, 0.0, 0.75364494], [0.2759493, 0.27587304, 0.27576756], [0.6090294, 0.67259055, 0.8307925])),
        entry("Kodak Ektar 100", "kodak-ektar100", DyeSet::of("kodak-ektar-100"), "kodak_ektar_100", Daylight, reference(
            [-0.8861552, 0.0, 0.77332544], [0.27716777, 0.27709648, 0.27698642], [0.63018304, 0.69412404, 0.8497187])),
        entry("Kodak Gold 200", "kodak-gold200", DyeSet::of("kodak-gold-200"), "kodak_gold_200", Daylight, reference(
            [-0.830452, 0.0, 1.0133026], [0.2786753, 0.27860653, 0.27844664], [0.6040813, 0.6759063, 0.73893756])),
        entry("Kodak UltraMax 400", "kodak-ultramax400", DyeSet::of("kodak-ultramax-400"), "kodak_ultra_max_400", Daylight, reference(
            [-0.82510906, 0.0, 1.1587664], [0.26436394, 0.26430267, 0.26412937], [0.57112956, 0.648374, 0.7212143])),
        entry("Kodak Verita 200D", "kodak-verita200d", DyeSet::of("kodak-verita-200d"), "kodak_verita_200d", Daylight, reference(
            [-1.0199594, 0.0, 0.9399972], [0.29461864, 0.29452857, 0.29436558], [0.65531194, 0.6866728, 0.75721455])),
        entry("Kodak Vision3 50D", "kodak-vision3-50d", DyeSet::of("kodak-vision3-50d"), "kodak_vision3_50d", Daylight, reference(
            [-0.91636825, 0.0, 0.94096464], [0.25927153, 0.2592079, 0.25908357], [0.5892993, 0.6541842, 0.6831387])),
        entry("Kodak Vision3 200T", "kodak-vision3-200t", DyeSet::of("kodak-vision3-200t"), "kodak_vision3_200t", Tungsten, reference(
            [-0.82578504, 0.0, 0.89388233], [0.26431274, 0.2642514, 0.26413083], [0.5918622, 0.66147536, 0.69205135])),
        entry("Kodak Vision3 250D", "kodak-vision3-250d", DyeSet::of("kodak-vision3-250d"), "kodak_vision3_250d", Daylight, reference(
            [-0.7535622, 0.0, 1.0177397], [0.2626649, 0.26260838, 0.2624662], [0.5803882, 0.64968544, 0.67921406])),
        entry("Kodak Vision3 500T", "kodak-vision3-500t", DyeSet::of("kodak-vision3-500t"), "kodak_vision3_500t", Tungsten, reference(
            [-0.77878654, 0.0, 0.9240054], [0.2656662, 0.2656069, 0.26547948], [0.5724703, 0.64309555, 0.671298])),
        entry("Fujifilm C200", "fujifilm-c200", DyeSet::of("fujifilm-c200"), "fujifilm_c200", Daylight, reference(
            [-0.83061093, 0.0, 0.9496308], [0.27427864, 0.27421215, 0.2740708], [0.64410913, 0.70046955, 0.7577055])),
        entry("Fujifilm Pro 400H", "fujifilm-pro400h", DyeSet::of("fujifilm-pro-400h"), "fujifilm_pro_400h", Daylight, reference(
            [-1.1394027, 0.0, 1.0356495], [0.305082, 0.30497682, 0.30477485], [0.6734493, 0.7103099, 0.7457083])),
        entry("Fujifilm Superia X-TRA 400", "fujifilm-superia-xtra400", DyeSet::of("fujifilm-xtra-400"), "fujifilm_superia_x_tra_400", Daylight, reference(
            [-0.782707, 0.0, 0.9280978], [0.2685544, 0.26849353, 0.26836243], [0.67764014, 0.73257005, 0.766886])),
        entry("Fujifilm 160NC", "fujifilm-160nc", DyeSet::of("fujifilm-160nc"), "fujifilm_160nc", Daylight, reference(
            [-0.9830299, 0.0, 0.9446567], [0.27338138, 0.27330685, 0.2731675], [0.64644915, 0.7188178, 0.9559797])),
        entry("Fujifilm 160NL", "fujifilm-160nl", DyeSet::of("fujifilm-160nl"), "fujifilm_160nl", Daylight, reference(
            [-1.0258617, 0.0, 0.88072264], [0.2809332, 0.28085187, 0.28071746], [0.61696494, 0.6893342, 0.8398999])),
        entry("Fujifilm 160NPC", "fujifilm-160npc", DyeSet::of("fujifilm-160npc"), "fujifilm_160npc", Daylight, reference(
            [-0.69356275, 0.0, 0.88195366], [0.25589237, 0.25584206, 0.25573105], [0.6553372, 0.69170797, 0.7153741])),
        entry("Fujifilm 160NS", "fujifilm-160ns", DyeSet::of("fujifilm-160ns"), "fujifilm_160ns", Daylight, reference(
            [-1.1013058, 0.0, 0.6771016], [0.2900523, 0.28996074, 0.2898581], [0.6394005, 0.707842, 0.8684795])),
        entry("Fujifilm 200", "fujifilm-200", DyeSet::of("fujifilm-200"), "fujifilm_200", Daylight, reference(
            [-0.98764145, 0.0, 1.3207269], [0.26841062, 0.26833865, 0.2681215], [0.5365795, 0.6401868, 0.6929141])),
        entry("Fujifilm 400", "fujifilm-400", DyeSet::of("fujifilm-400"), "fujifilm_400", Daylight, reference(
            [-0.98744607, 0.0, 1.3159344], [0.2693434, 0.2692709, 0.26905343], [0.56718695, 0.67985857, 0.71471524])),
        entry("Fujifilm Fujicolor 100", "fujifilm-fujicolor100", DyeSet::of("fujifilm-fujicolor-100"), "fujifilm_fujicolor_100", Daylight, reference(
            [-0.8553253, 0.0, 0.5461921], [0.26282758, 0.2627654, 0.26270142], [0.69335103, 0.7611767, 0.8521906])),
        entry("Fujifilm Natura 1600", "fujifilm-natura1600", DyeSet::of("fujifilm-natura-1600"), "fujifilm_natura_1600", Daylight, reference(
            [-0.89495873, 0.0, 0.9772441], [0.2745942, 0.2745238, 0.27437645], [0.6612302, 0.7591614, 0.7650651])),
        entry("Fujifilm New Pro 400", "fujifilm-new-pro400", DyeSet::of("fujifilm-new-pro-400"), "fujifilm_new_pro_400", Daylight, reference(
            [-0.9659846, 0.0, 0.82500136], [0.2955389, 0.29545158, 0.29531366], [0.6801367, 0.7054779, 0.77767396])),
        entry("Fujifilm Nexia 400", "fujifilm-nexia400", DyeSet::of("fujifilm-nexia-400"), "fujifilm_nexia_400", Daylight, reference(
            [-1.1186444, 0.0, 1.0489243], [0.35295036, 0.35280007, 0.35250288], [0.73805636, 0.80672616, 0.8723476])),
        entry("Fujifilm Nexia A200", "fujifilm-nexia-a200", DyeSet::of("fujifilm-nexia-a200"), "fujifilm_nexia_a200", Daylight, reference(
            [-1.0643239, 0.0, 1.2049339], [0.36077076, 0.36061642, 0.36023104], [0.7564968, 0.80627173, 0.9123062])),
        entry("Fujifilm Nexia Zoom Master 800", "fujifilm-nexia-zoom-master800", DyeSet::of("fujifilm-nexia-zoom-master-800"), "fujifilm_nexia_zoom_master_800", Daylight, reference(
            [-1.060208, 0.0, 1.2241347], [0.3226104, 0.3224957, 0.3222016], [0.7004337, 0.74900055, 0.80266106])),
        entry("Fujifilm Pro 160C", "fujifilm-pro160c", DyeSet::of("fujifilm-pro-160c"), "fujifilm_pro_160c", Daylight, reference(
            [-1.3470296, 0.0, 0.6656624], [0.3497056, 0.3495408, 0.34938172], [0.7077327, 0.7545884, 0.84157795])),
        entry("Fujifilm Pro 160NC", "fujifilm-pro160nc", DyeSet::of("fujifilm-pro-160nc"), "fujifilm_pro_160nc", Daylight, reference(
            [-1.2484103, 0.0, 0.5905031], [0.33505836, 0.33491787, 0.33479527], [0.7006667, 0.7412388, 0.8387661])),
        entry("Fujifilm Pro 160NH", "fujifilm-pro160nh", DyeSet::of("fujifilm-pro-160nh"), "fujifilm_pro_160nh", Daylight, reference(
            [-1.3468778, 0.0, 0.6698033], [0.35126054, 0.3510937, 0.3509315], [0.72994417, 0.7379483, 0.8283898])),
        entry("Fujifilm Pro 160NS", "fujifilm-pro160ns", DyeSet::of("fujifilm-pro-160ns"), "fujifilm_pro_160ns", Daylight, reference(
            [-1.2484103, 0.0, 0.5905031], [0.33505836, 0.33491787, 0.33479527], [0.67940396, 0.6918014, 0.78970337])),
        entry("Fujifilm Pro 160S", "fujifilm-pro160s", DyeSet::of("fujifilm-pro-160s"), "fujifilm_pro_160s", Daylight, reference(
            [-1.2484833, 0.0, 0.59052044], [0.33505875, 0.33491823, 0.33479562], [0.6692225, 0.70564353, 0.75640184])),
        entry("Fujifilm Pro 800", "fujifilm-pro800", DyeSet::of("fujifilm-pro-800"), "fujifilm_pro_800", Daylight, reference(
            [-0.99509996, 0.0, 1.2042607], [0.29084924, 0.2907633, 0.2905385], [0.68136615, 0.73732394, 0.7561798])),
        entry("Fujifilm Pro 800Z", "fujifilm-pro800z", DyeSet::of("fujifilm-pro-800z"), "fujifilm_pro_800z", Daylight, reference(
            [-1.0227036, 0.0, 1.1428204], [0.29216453, 0.292076, 0.29186568], [0.671126, 0.7200098, 0.73973036])),
        entry("Fujifilm Reala Ace", "fujifilm-reala-ace", DyeSet::of("fujifilm-reala-ace"), "fujifilm_reala_ace", Daylight, reference(
            [-0.8249654, 0.0, 0.74971867], [0.27130067, 0.271236, 0.27113494], [0.67152333, 0.7096726, 0.77254534])),
        entry("Fujifilm Superia 100", "fujifilm-superia100", DyeSet::of("fujifilm-superia-100"), "fujifilm_superia_100", Daylight, reference(
            [-0.87347025, 0.0, 0.5669083], [0.28469506, 0.28462034, 0.28454116], [0.63838637, 0.6988484, 0.7464876])),
        entry("Fujifilm Superia 200", "fujifilm-superia200", DyeSet::of("fujifilm-superia-200"), "fujifilm_superia_200", Daylight, reference(
            [-0.84955186, 0.0, 0.61150295], [0.2812986, 0.2812272, 0.28114265], [0.69175124, 0.744949, 0.7652797])),
        entry("Fujifilm Superia 1600", "fujifilm-superia1600", DyeSet::of("fujifilm-superia-1600"), "fujifilm_superia_1600", Daylight, reference(
            [-0.89496434, 0.0, 0.9771375], [0.2746378, 0.27456737, 0.27442], [0.6612944, 0.75925225, 0.7651675])),
        entry("Fujifilm Superia Premium 400", "fujifilm-superia-premium400", DyeSet::of("fujifilm-superia-premium-400"), "fujifilm_superia_premium_400", Daylight, reference(
            [-0.7838326, 0.0, 0.9296579], [0.26892984, 0.26886877, 0.268737], [0.70088303, 0.74364537, 0.78759575])),
        entry("Fujifilm Superia Reala", "fujifilm-superia-reala", DyeSet::of("fujifilm-superia-reala"), "fujifilm_superia_reala", Daylight, reference(
            [-0.72140634, 0.0, 0.90098035], [0.28289422, 0.2828304, 0.28268972], [0.6181936, 0.700884, 0.66580355])),
        entry("Fujifilm Superia Venus 400", "fujifilm-superia-venus400", DyeSet::of("fujifilm-superia-venus-400"), "fujifilm_superia_venus_400", Daylight, reference(
            [-0.7705156, 0.0, 0.86905223], [0.26798925, 0.2679294, 0.2678099], [0.6571573, 0.7389691, 0.77597964])),
        entry("Fujifilm Superia Venus 800", "fujifilm-superia-venus800", DyeSet::of("fujifilm-superia-venus-800"), "fujifilm_superia_venus_800", Daylight, reference(
            [-0.8487747, 0.0, 0.8897301], [0.27266905, 0.27260232, 0.2724745], [0.6658727, 0.7519495, 0.7893201])),
        entry("Fujifilm Superia X-TRA 800", "fujifilm-superia-xtra800", DyeSet::of("fujifilm-xtra-800"), "fujifilm_superia_x_tra_800", Daylight, reference(
            [-1.1181284, 0.0, 1.1705339], [0.3584864, 0.35832962, 0.35796675], [0.7595167, 0.8454484, 0.92107224])),
        entry("Kodak Portra 160NC", "kodak-portra160nc", DyeSet::of("kodak-portra-160nc"), "kodak_portra_160nc", Daylight, reference(
            [-0.55807227, 0.0, 0.43169335], [0.31164914, 0.31158417, 0.3115135], [0.74749005, 0.78080827, 0.87925315])),
        entry("Kodak Portra 160VC", "kodak-portra160vc", DyeSet::of("kodak-portra-160vc"), "kodak_portra_160vc", Daylight, reference(
            [-0.565028, 0.0, 0.5502797], [0.30715853, 0.30709514, 0.30700427], [0.7589101, 0.7894718, 0.8927779])),
        entry("Kodak Portra 400NC", "kodak-portra400nc", DyeSet::of("kodak-portra-400nc"), "kodak_portra_400nc", Daylight, reference(
            [-0.5557344, 0.0, 0.5149134], [0.30773997, 0.3076771, 0.3075928], [0.7414715, 0.77481437, 0.87066925])),
        entry("Kodak Portra 400VC", "kodak-portra400vc", DyeSet::of("kodak-portra-400vc"), "kodak_portra_400vc", Daylight, reference(
            [-0.5713503, 0.0, 0.5229958], [0.30710933, 0.30704534, 0.3069599], [0.75755215, 0.79070425, 0.8924931])),
        entry("Foma Fomapan 100", "foma-fomapan100", DyeSet::of("generic-c41"), "foma_fomapan_100", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.641245, 0.641245, 0.641245])),
        entry("Foma Fomapan 200", "foma-fomapan200", DyeSet::of("generic-c41"), "foma_fomapan_200", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.6508648, 0.6508648, 0.6508648])),
        entry("Foma Fomapan 400", "foma-fomapan400", DyeSet::of("generic-c41"), "foma_fomapan_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.6400596, 0.6400596, 0.6400596])),
        entry("Foma Fomapan Cine 100", "foma-fomapan-cine100", DyeSet::of("generic-c41"), "foma_fomapan_cine_100", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.64884645, 0.64884645, 0.64884645])),
        entry("Foma Fomapan Cine 400", "foma-fomapan-cine400", DyeSet::of("generic-c41"), "foma_fomapan_cine_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.6525161, 0.6525161, 0.6525161])),
        entry("Foma Ortho 400", "foma-ortho400", DyeSet::of("generic-c41"), "foma_ortho_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.6923136, 0.6923136, 0.6923136])),
        entry("Foma Cine Ortho 400", "foma-cine-ortho400", DyeSet::of("generic-c41"), "foma_cine_ortho_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.70480514, 0.70480514, 0.70480514])),
        entry("Foma Retropan 320", "foma-retropan320", DyeSet::of("generic-c41"), "foma_retropan_320", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4840337, 0.4840337, 0.4840337])),
        entry("Fujifilm Acros", "fujifilm-acros", DyeSet::of("generic-c41"), "fujifilm_acros_ii", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.55640346, 0.55640346, 0.55640346])),
        entry("Fujifilm Neopan 400", "fujifilm-neopan400", DyeSet::of("generic-c41"), "fujifilm_neopan_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5011924, 0.5011924, 0.5011924])),
        entry("Fujifilm Neopan 1600", "fujifilm-neopan1600", DyeSet::of("generic-c41"), "fujifilm_neopan_1600", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5704813, 0.5704813, 0.5704813])),
        entry("Fujifilm Neopan Ortho", "fujifilm-neopan-ortho", DyeSet::of("generic-c41"), "fujifilm_neopan_ortho", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5318679, 0.5318679, 0.5318679])),
        entry("Ilford Delta 100", "ilford-delta100", DyeSet::of("generic-c41"), "ilford_delta_100", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5453998, 0.5453998, 0.5453998])),
        entry("Ilford Delta 400", "ilford-delta400", DyeSet::of("generic-c41"), "ilford_delta_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5495109, 0.5495109, 0.5495109])),
        entry("Ilford Delta 3200", "ilford-delta3200", DyeSet::of("generic-c41"), "ilford_delta_3200", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.50518507, 0.50518507, 0.50518507])),
        entry("Ilford FP4", "ilford-fp4", DyeSet::of("generic-c41"), "ilford_fp4_plus", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5620778, 0.5620778, 0.5620778])),
        entry("Ilford HP5", "ilford-hp5", DyeSet::of("generic-c41"), "ilford_hp5_plus", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.529545, 0.529545, 0.529545])),
        entry("Ilford Ortho", "ilford-ortho", DyeSet::of("generic-c41"), "ilford_ortho_plus", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5154966, 0.5154966, 0.5154966])),
        entry("Ilford Pan F", "ilford-panf", DyeSet::of("generic-c41"), "ilford_pan_f_plus", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.59475046, 0.59475046, 0.59475046])),
        entry("Ilford SFX", "ilford-sfx", DyeSet::of("generic-c41"), "ilford_sfx_200", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5630885, 0.5630885, 0.5630885])),
        entry("Ilford XP2", "ilford-xp2", DyeSet::of("generic-c41"), "ilford_xp2_super", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.58853966, 0.58853966, 0.58853966])),
        entry("Kodak Double-X", "kodak-double-x", DyeSet::of("generic-c41"), "kodak_double_x", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5464296, 0.5464296, 0.5464296])),
        entry("Kodak T-Max 100", "kodak-tmax100", DyeSet::of("generic-c41"), "kodak_t_max_100", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5124685, 0.5124685, 0.5124685])),
        entry("Kodak T-Max 400", "kodak-tmax400", DyeSet::of("generic-c41"), "kodak_t_max_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.47859788, 0.47859788, 0.47859788])),
        entry("Kodak T-Max P3200", "kodak-tmax-p3200", DyeSet::of("generic-c41"), "kodak_t_max_p3200", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.65690064, 0.65690064, 0.65690064])),
        entry("Kodak Tri-X 320", "kodak-320txp", DyeSet::of("generic-c41"), "kodak_tri_x_320", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.49478894, 0.49478894, 0.49478894])),
        entry("Kodak Tri-X 400", "kodak-400tx", DyeSet::of("generic-c41"), "kodak_tri_x_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5753634, 0.5753634, 0.5753634])),
    ]
};
