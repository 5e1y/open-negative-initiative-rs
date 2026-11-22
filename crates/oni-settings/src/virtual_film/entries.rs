//! Every virtual stock's name, catalogue place, dyes, profile, light and reference, in `VirtualStock::ALL`'s order.

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

/// Every stock in `VirtualStock::ALL`'s order, which is the enum's. A push develops Portra 800's own dyes on its curves; a
/// black and white film rests on the generic set, as a black and white roll does: its silver unmixes nothing.
pub(super) const ENTRIES: [Entry; VirtualStock::ALL.len()] = {
    use Balance::*;
    [
        entry("Kodak Portra 800", "kodak-portra800", DyeSet::KodakPortra800, "kodak_portra_800", Daylight, reference(
            [-0.84658265, 0.0, 1.1941923], [0.26643735, 0.2663739, 0.26618993], [0.57385314, 0.653798, 0.7241174])),
        entry("Kodak Portra 800 (Push 1)", "kodak-portra800-push1", DyeSet::KodakPortra800, "kodak_portra_800_push1", Daylight, reference(
            [-0.8467025, 0.0, 1.1942314], [0.2664324, 0.26636896, 0.266185], [0.5951463, 0.66220385, 0.720267])),
        entry("Kodak Portra 800 (Push 2)", "kodak-portra800-push2", DyeSet::KodakPortra800, "kodak_portra_800_push2", Daylight, reference(
            [-0.8467025, 0.0, 1.1942314], [0.2664324, 0.26636896, 0.266185], [0.53791285, 0.60454583, 0.6328216])),
        entry("Kodak Portra 160", "kodak-portra160", DyeSet::KodakPortra160, "kodak_portra_160", Daylight, reference(
            [-1.0310265, 0.0, 0.7707151], [0.27240276, 0.27232632, 0.27222067], [0.585196, 0.65457016, 0.78202575])),
        entry("Kodak Portra 400", "kodak-portra400", DyeSet::KodakPortra400, "kodak_portra_400", Daylight, reference(
            [-0.98764074, 0.0, 0.75364494], [0.2759493, 0.27587304, 0.27576756], [0.6090294, 0.67259055, 0.8307925])),
        entry("Kodak Ektar 100", "kodak-ektar100", DyeSet::KodakEktar100, "kodak_ektar_100", Daylight, reference(
            [-0.8861552, 0.0, 0.77332544], [0.27716777, 0.27709648, 0.27698642], [0.63018304, 0.69412404, 0.8497187])),
        entry("Kodak Gold 200", "kodak-gold200", DyeSet::KodakGold200, "kodak_gold_200", Daylight, reference(
            [-0.83139825, 0.0, 1.0253669], [0.27878144, 0.27871257, 0.2785499], [0.60417104, 0.67596966, 0.73896456])),
        entry("Kodak UltraMax 400", "kodak-ultramax400", DyeSet::KodakUltraMax400, "kodak_ultra_max_400", Daylight, reference(
            [-0.82510906, 0.0, 1.1587664], [0.26436394, 0.26430267, 0.26412937], [0.57112956, 0.648374, 0.7212143])),
        entry("Kodak Verita 200D", "kodak-verita200d", DyeSet::KodakVerita200D, "kodak_verita_200d", Daylight, reference(
            [-1.0199594, 0.0, 0.9399972], [0.29461864, 0.29452857, 0.29436558], [0.65531194, 0.6866728, 0.75721455])),
        entry("Kodak Vision3 50D", "kodak-vision3-50d", DyeSet::KodakVision3_50D, "kodak_vision3_50d", Daylight, reference(
            [-0.91775477, 0.0, 0.93932354], [0.25944576, 0.25938198, 0.2592578], [0.58947694, 0.6543164, 0.6832948])),
        entry("Kodak Vision3 200T", "kodak-vision3-200t", DyeSet::KodakVision3_200T, "kodak_vision3_200t", Tungsten, reference(
            [-0.8270469, 0.0, 0.8934726], [0.26446837, 0.26440692, 0.26428625], [0.5920118, 0.66158617, 0.69219285])),
        entry("Kodak Vision3 250D", "kodak-vision3-250d", DyeSet::KodakVision3_250D, "kodak_vision3_250d", Daylight, reference(
            [-0.7536511, 0.0, 1.0187563], [0.26267508, 0.26261857, 0.26247615], [0.580397, 0.6496923, 0.6792257])),
        entry("Kodak Vision3 500T", "kodak-vision3-500t", DyeSet::KodakVision3_500T, "kodak_vision3_500t", Tungsten, reference(
            [-0.7810475, 0.0, 0.92161024], [0.26594657, 0.26588705, 0.2657598], [0.572731, 0.6433025, 0.67158437])),
        entry("Fujifilm C200", "fujifilm-c200", DyeSet::FujifilmC200, "fujifilm_c200", Daylight, reference(
            [-0.8324413, 0.0, 0.94690055], [0.27455822, 0.2744915, 0.27435043], [0.6443795, 0.7007058, 0.75799775])),
        entry("Fujifilm Pro 400H", "fujifilm-pro400h", DyeSet::FujifilmPro400H, "fujifilm_pro_400h", Daylight, reference(
            [-1.1439059, 0.0, 1.0300418], [0.3056799, 0.30557394, 0.3053726], [0.6740562, 0.7107914, 0.7463245])),
        entry("Fujifilm Superia X-TRA 400", "fujifilm-superia-xtra400", DyeSet::FujifilmXtra400, "fujifilm_superia_x_tra_400", Daylight, reference(
            [-0.782707, 0.0, 0.9280978], [0.2685544, 0.26849353, 0.26836243], [0.67764014, 0.73257005, 0.766886])),
        entry("Fujifilm 160NC", "fujifilm-160nc", DyeSet::Fujifilm160NC, "fujifilm_160nc", Daylight, reference(
            [-0.9831477, 0.0, 0.9439672], [0.2734567, 0.27338213, 0.27324286], [0.64651644, 0.71887934, 0.9560455])),
        entry("Fujifilm 160NL", "fujifilm-160nl", DyeSet::Fujifilm160NL, "fujifilm_160nl", Daylight, reference(
            [-1.0268108, 0.0, 0.87943566], [0.28105754, 0.28097606, 0.2808418], [0.6170878, 0.6894285, 0.84003425])),
        entry("Fujifilm 160NPC", "fujifilm-160npc", DyeSet::Fujifilm160NPC, "fujifilm_160npc", Daylight, reference(
            [-0.6942758, 0.0, 0.8810246], [0.25598925, 0.25593886, 0.25582793], [0.65543544, 0.6917867, 0.7154745])),
        entry("Fujifilm 160NS", "fujifilm-160ns", DyeSet::Fujifilm160NS, "fujifilm_160ns", Daylight, reference(
            [-1.1010375, 0.0, 0.67702633], [0.29006514, 0.2899736, 0.28987095], [0.63941145, 0.7078595, 0.8684883])),
        entry("Fujifilm 200", "fujifilm-200", DyeSet::Fujifilm200, "fujifilm_200", Daylight, reference(
            [-0.9948747, 0.0, 1.3113018], [0.26929316, 0.26922035, 0.26900405], [0.5374284, 0.64066726, 0.69382226])),
        entry("Fujifilm 400", "fujifilm-400", DyeSet::Fujifilm400, "fujifilm_400", Daylight, reference(
            [-0.99332553, 0.0, 1.3085915], [0.27005628, 0.2699831, 0.26976624], [0.5679498, 0.68008995, 0.7137311])),
        entry("Fujifilm Fujicolor 100", "fujifilm-fujicolor100", DyeSet::FujifilmFujicolor100, "fujifilm_fujicolor_100", Daylight, reference(
            [-0.8553253, 0.0, 0.5461921], [0.26282758, 0.2627654, 0.26270142], [0.69335103, 0.7611767, 0.8521906])),
        entry("Fujifilm Natura 1600", "fujifilm-natura1600", DyeSet::FujifilmNatura1600, "fujifilm_natura_1600", Daylight, reference(
            [-0.89495873, 0.0, 0.9772441], [0.2745942, 0.2745238, 0.27437645], [0.6612302, 0.7591614, 0.7650651])),
        entry("Fujifilm New Pro 400", "fujifilm-new-pro400", DyeSet::FujifilmNewPro400, "fujifilm_new_pro_400", Daylight, reference(
            [-0.9669731, 0.0, 0.8238012], [0.2956667, 0.29557922, 0.29544142], [0.68026924, 0.7055776, 0.7778082])),
        entry("Fujifilm Nexia 400", "fujifilm-nexia400", DyeSet::FujifilmNexia400, "fujifilm_nexia_400", Daylight, reference(
            [-1.1273885, 0.0, 1.0422893], [0.35418445, 0.35403195, 0.35373458], [0.73957896, 0.8080721, 0.8736243])),
        entry("Fujifilm Nexia A200", "fujifilm-nexia-a200", DyeSet::FujifilmNexiaA200, "fujifilm_nexia_a200", Daylight, reference(
            [-1.0738528, 0.0, 1.1921123], [0.36212105, 0.3619641, 0.36158082], [0.75783634, 0.8072892, 0.91372514])),
        entry("Fujifilm Nexia Zoom Master 800", "fujifilm-nexia-zoom-master800", DyeSet::FujifilmNexiaZoomMaster800, "fujifilm_nexia_zoom_master_800", Daylight, reference(
            [-1.060208, 0.0, 1.2241347], [0.3226104, 0.3224957, 0.3222016], [0.7004337, 0.74900055, 0.80266106])),
        entry("Fujifilm Pro 160C", "fujifilm-pro160c", DyeSet::FujifilmPro160C, "fujifilm_pro_160c", Daylight, reference(
            [-1.3489603, 0.0, 0.6635711], [0.34994933, 0.34978402, 0.34962532], [0.7079751, 0.7547523, 0.8418316])),
        entry("Fujifilm Pro 160NC", "fujifilm-pro160nc", DyeSet::FujifilmPro160NC, "fujifilm_pro_160nc", Daylight, reference(
            [-1.2522445, 0.0, 0.5854004], [0.33554104, 0.33539972, 0.33527794], [0.70115215, 0.7415657, 0.83928])),
        entry("Fujifilm Pro 160NH", "fujifilm-pro160nh", DyeSet::FujifilmPro160NH, "fujifilm_pro_160nh", Daylight, reference(
            [-1.3569378, 0.0, 0.6563389], [0.35253024, 0.352361, 0.35220125], [0.73125017, 0.7388877, 0.8297411])),
        entry("Fujifilm Pro 160NS", "fujifilm-pro160ns", DyeSet::FujifilmPro160NS, "fujifilm_pro_160ns", Daylight, reference(
            [-1.2569981, 0.0, 0.579417], [0.33613908, 0.33599678, 0.335876], [0.68050957, 0.6926089, 0.7908475])),
        entry("Fujifilm Pro 160S", "fujifilm-pro160s", DyeSet::FujifilmPro160S, "fujifilm_pro_160s", Daylight, reference(
            [-1.2497555, 0.0, 0.5904358], [0.33521792, 0.3350771, 0.3349544], [0.66937995, 0.7057606, 0.75655246])),
        entry("Fujifilm Pro 800", "fujifilm-pro800", DyeSet::FujifilmPro800, "fujifilm_pro_800", Daylight, reference(
            [-1.0017225, 0.0, 1.1957734], [0.29175043, 0.29166344, 0.29143947], [0.6822735, 0.738021, 0.7571088])),
        entry("Fujifilm Pro 800Z", "fujifilm-pro800z", DyeSet::FujifilmPro800Z, "fujifilm_pro_800z", Daylight, reference(
            [-1.0237049, 0.0, 1.1416192], [0.2922934, 0.2922047, 0.29199448], [0.6712575, 0.72010565, 0.73986113])),
        entry("Fujifilm Reala Ace", "fujifilm-reala-ace", DyeSet::FujifilmRealaAce, "fujifilm_reala_ace", Daylight, reference(
            [-0.82549745, 0.0, 0.7494385], [0.2713693, 0.2713046, 0.27120352], [0.6715933, 0.70972663, 0.77261454])),
        entry("Fujifilm Superia 100", "fujifilm-superia100", DyeSet::FujifilmSuperia100, "fujifilm_superia_100", Daylight, reference(
            [-0.8800615, 0.0, 0.5566057], [0.28569442, 0.2856187, 0.28554067], [0.6392721, 0.69965786, 0.74758047])),
        entry("Fujifilm Superia 200", "fujifilm-superia200", DyeSet::FujifilmSuperia200, "fujifilm_superia_200", Daylight, reference(
            [-0.8489469, 0.0, 0.6115581], [0.28131896, 0.28124765, 0.281163], [0.6917813, 0.7449813, 0.76527333])),
        entry("Fujifilm Superia 1600", "fujifilm-superia1600", DyeSet::FujifilmSuperia1600, "fujifilm_superia_1600", Daylight, reference(
            [-0.89496434, 0.0, 0.9771375], [0.2746378, 0.27456737, 0.27442], [0.6612944, 0.75925225, 0.7651675])),
        entry("Fujifilm Superia Premium 400", "fujifilm-superia-premium400", DyeSet::FujifilmSuperiaPremium400, "fujifilm_superia_premium_400", Daylight, reference(
            [-0.7838326, 0.0, 0.9296579], [0.26892984, 0.26886877, 0.268737], [0.70088303, 0.74364537, 0.78759575])),
        entry("Fujifilm Superia Reala", "fujifilm-superia-reala", DyeSet::FujifilmSuperiaReala, "fujifilm_superia_reala", Daylight, reference(
            [-0.7220987, 0.0, 0.9036558], [0.28298858, 0.28292465, 0.2827833], [0.6182754, 0.7009721, 0.6658694])),
        entry("Fujifilm Superia Venus 400", "fujifilm-superia-venus400", DyeSet::FujifilmSuperiaVenus400, "fujifilm_superia_venus_400", Daylight, reference(
            [-0.7705156, 0.0, 0.86905223], [0.26798925, 0.2679294, 0.2678099], [0.6571573, 0.7389691, 0.77597964])),
        entry("Fujifilm Superia Venus 800", "fujifilm-superia-venus800", DyeSet::FujifilmSuperiaVenus800, "fujifilm_superia_venus_800", Daylight, reference(
            [-0.8487747, 0.0, 0.8897301], [0.27266905, 0.27260232, 0.2724745], [0.6658727, 0.7519495, 0.7893201])),
        entry("Fujifilm Superia X-TRA 800", "fujifilm-superia-xtra800", DyeSet::FujifilmXtra800, "fujifilm_superia_x_tra_800", Daylight, reference(
            [-1.1167265, 0.0, 1.1680197], [0.35891587, 0.3587587, 0.35839576], [0.759881, 0.8458699, 0.9214567])),
        entry("Kodak Portra 160NC", "kodak-portra160nc", DyeSet::KodakPortra160NC, "kodak_portra_160nc", Daylight, reference(
            [-0.55850405, 0.0, 0.43036628], [0.3117739, 0.3117088, 0.31163836], [0.7476073, 0.78093415, 0.87936914])),
        entry("Kodak Portra 160VC", "kodak-portra160vc", DyeSet::KodakPortra160VC, "kodak_portra_160vc", Daylight, reference(
            [-0.5653085, 0.0, 0.54926115], [0.3072533, 0.3071898, 0.3070991], [0.75899804, 0.7895659, 0.8928661])),
        entry("Kodak Portra 400NC", "kodak-portra400nc", DyeSet::KodakPortra400NC, "kodak_portra_400nc", Daylight, reference(
            [-0.5557344, 0.0, 0.5149134], [0.30773997, 0.3076771, 0.3075928], [0.7414715, 0.77481437, 0.87066925])),
        entry("Kodak Portra 400VC", "kodak-portra400vc", DyeSet::KodakPortra400VC, "kodak_portra_400vc", Daylight, reference(
            [-0.5713503, 0.0, 0.5229958], [0.30710933, 0.30704534, 0.3069599], [0.75755215, 0.79070425, 0.8924931])),
        entry("Foma Fomapan 100", "foma-fomapan100", DyeSet::GenericC41, "foma_fomapan_100", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.641245, 0.641245, 0.641245])),
        entry("Foma Fomapan 200", "foma-fomapan200", DyeSet::GenericC41, "foma_fomapan_200", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.6508648, 0.6508648, 0.6508648])),
        entry("Foma Fomapan 400", "foma-fomapan400", DyeSet::GenericC41, "foma_fomapan_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.6400596, 0.6400596, 0.6400596])),
        entry("Foma Fomapan Cine 100", "foma-fomapan-cine100", DyeSet::GenericC41, "foma_fomapan_cine_100", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.64884645, 0.64884645, 0.64884645])),
        entry("Foma Fomapan Cine 400", "foma-fomapan-cine400", DyeSet::GenericC41, "foma_fomapan_cine_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.6525161, 0.6525161, 0.6525161])),
        entry("Foma Ortho 400", "foma-ortho400", DyeSet::GenericC41, "foma_ortho_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.6923136, 0.6923136, 0.6923136])),
        entry("Foma Cine Ortho 400", "foma-cine-ortho400", DyeSet::GenericC41, "foma_cine_ortho_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.70480514, 0.70480514, 0.70480514])),
        entry("Foma Retropan 320", "foma-retropan320", DyeSet::GenericC41, "foma_retropan_320", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4840337, 0.4840337, 0.4840337])),
        entry("Fujifilm Acros", "fujifilm-acros", DyeSet::GenericC41, "fujifilm_acros_ii", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.55640346, 0.55640346, 0.55640346])),
        entry("Fujifilm Neopan 400", "fujifilm-neopan400", DyeSet::GenericC41, "fujifilm_neopan_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5011924, 0.5011924, 0.5011924])),
        entry("Fujifilm Neopan 1600", "fujifilm-neopan1600", DyeSet::GenericC41, "fujifilm_neopan_1600", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5704813, 0.5704813, 0.5704813])),
        entry("Fujifilm Neopan Ortho", "fujifilm-neopan-ortho", DyeSet::GenericC41, "fujifilm_neopan_ortho", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5318679, 0.5318679, 0.5318679])),
        entry("Ilford Delta 100", "ilford-delta100", DyeSet::GenericC41, "ilford_delta_100", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5453998, 0.5453998, 0.5453998])),
        entry("Ilford Delta 400", "ilford-delta400", DyeSet::GenericC41, "ilford_delta_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5495109, 0.5495109, 0.5495109])),
        entry("Ilford Delta 3200", "ilford-delta3200", DyeSet::GenericC41, "ilford_delta_3200", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.50518507, 0.50518507, 0.50518507])),
        entry("Ilford FP4", "ilford-fp4", DyeSet::GenericC41, "ilford_fp4_plus", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5620778, 0.5620778, 0.5620778])),
        entry("Ilford HP5", "ilford-hp5", DyeSet::GenericC41, "ilford_hp5_plus", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.529545, 0.529545, 0.529545])),
        entry("Ilford Ortho", "ilford-ortho", DyeSet::GenericC41, "ilford_ortho_plus", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5154966, 0.5154966, 0.5154966])),
        entry("Ilford Pan F", "ilford-panf", DyeSet::GenericC41, "ilford_pan_f_plus", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.59475046, 0.59475046, 0.59475046])),
        entry("Ilford SFX", "ilford-sfx", DyeSet::GenericC41, "ilford_sfx_200", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5630885, 0.5630885, 0.5630885])),
        entry("Ilford XP2", "ilford-xp2", DyeSet::GenericC41, "ilford_xp2_super", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.58853966, 0.58853966, 0.58853966])),
        entry("Kodak Double-X", "kodak-double-x", DyeSet::GenericC41, "kodak_double_x", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5464296, 0.5464296, 0.5464296])),
        entry("Kodak T-Max 100", "kodak-tmax100", DyeSet::GenericC41, "kodak_t_max_100", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5124685, 0.5124685, 0.5124685])),
        entry("Kodak T-Max 400", "kodak-tmax400", DyeSet::GenericC41, "kodak_t_max_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.47859788, 0.47859788, 0.47859788])),
        entry("Kodak T-Max P3200", "kodak-tmax-p3200", DyeSet::GenericC41, "kodak_t_max_p3200", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.65690064, 0.65690064, 0.65690064])),
        entry("Kodak Tri-X 320", "kodak-320txp", DyeSet::GenericC41, "kodak_tri_x_320", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.49478894, 0.49478894, 0.49478894])),
        entry("Kodak Tri-X 400", "kodak-400tx", DyeSet::GenericC41, "kodak_tri_x_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5753634, 0.5753634, 0.5753634])),
    ]
};
