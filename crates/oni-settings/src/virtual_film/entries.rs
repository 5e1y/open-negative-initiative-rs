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
/// black and white film rests on the generic set, as a black and white roll does.
pub(super) const ENTRIES: [Entry; VirtualStock::ALL.len()] = {
    use Balance::*;
    [
        entry("Kodak Portra 800", "kodak-portra800", DyeSet::KodakPortra800, "kodak_portra_800", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4713956, 0.46853185, 0.52487874])),
        entry("Kodak Portra 800 (Push 1)", "kodak-portra800-push1", DyeSet::KodakPortra800, "kodak_portra_800_push1", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.50858885, 0.46855497, 0.5184835])),
        entry("Kodak Portra 800 (Push 2)", "kodak-portra800-push2", DyeSet::KodakPortra800, "kodak_portra_800_push2", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.43020713, 0.42108405, 0.43428195])),
        entry("Kodak Portra 160", "kodak-portra160", DyeSet::KodakPortra160, "kodak_portra_160", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.489442, 0.46455348, 0.5802147])),
        entry("Kodak Portra 400", "kodak-portra400", DyeSet::KodakPortra400, "kodak_portra_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4978547, 0.4561537, 0.6259074])),
        entry("Kodak Ektar 100", "kodak-ektar100", DyeSet::KodakEktar100, "kodak_ektar_100", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5317531, 0.46712613, 0.6456602])),
        entry("Kodak Gold 200", "kodak-gold200", DyeSet::KodakGold200, "kodak_gold_200", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4953245, 0.46684808, 0.5301168])),
        entry("Kodak UltraMax 400", "kodak-ultramax400", DyeSet::KodakUltraMax400, "kodak_ultra_max_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.47177672, 0.45887822, 0.52780765])),
        entry("Kodak Verita 200D", "kodak-verita200d", DyeSet::KodakVerita200D, "kodak_verita_200d", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.52952355, 0.45191997, 0.4524775])),
        entry("Kodak Vision3 50D", "kodak-vision3-50d", DyeSet::KodakVision3_50D, "kodak_vision3_50d", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4631214, 0.46999073, 0.49043834])),
        entry("Kodak Vision3 200T", "kodak-vision3-200t", DyeSet::KodakVision3_200T, "kodak_vision3_200t", Tungsten, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.47826314, 0.47624475, 0.47750306])),
        entry("Kodak Vision3 250D", "kodak-vision3-250d", DyeSet::KodakVision3_250D, "kodak_vision3_250d", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.45314205, 0.46329427, 0.48591965])),
        entry("Kodak Vision3 500T", "kodak-vision3-500t", DyeSet::KodakVision3_500T, "kodak_vision3_500t", Tungsten, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.446059, 0.42791712, 0.49711108])),
        entry("Fujifilm C200", "fujifilm-c200", DyeSet::FujifilmC200, "fujifilm_c200", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4785112, 0.47279185, 0.5669467])),
        entry("Fujifilm Pro 400H", "fujifilm-pro400h", DyeSet::FujifilmPro400H, "fujifilm_pro_400h", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5207028, 0.4615916, 0.508808])),
        entry("Fujifilm Superia X-TRA 400", "fujifilm-superia-xtra400", DyeSet::FujifilmXtra400, "fujifilm_superia_x_tra_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5355894, 0.4963566, 0.59353447])),
        entry("Fujifilm 160NC", "fujifilm-160nc", DyeSet::Fujifilm160NC, "fujifilm_160nc", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5164758, 0.50035465, 0.8006314])),
        entry("Fujifilm 160NL", "fujifilm-160nl", DyeSet::Fujifilm160NL, "fujifilm_160nl", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4846192, 0.46831, 0.67295873])),
        entry("Fujifilm 160NPC", "fujifilm-160npc", DyeSet::Fujifilm160NPC, "fujifilm_160npc", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5737342, 0.48222595, 0.5326451])),
        entry("Fujifilm 160NS", "fujifilm-160ns", DyeSet::Fujifilm160NS, "fujifilm_160ns", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.49376518, 0.46587342, 0.6951522])),
        entry("Fujifilm 200", "fujifilm-200", DyeSet::Fujifilm200, "fujifilm_200", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.45958436, 0.47943157, 0.454723])),
        entry("Fujifilm 400", "fujifilm-400", DyeSet::Fujifilm400, "fujifilm_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5002437, 0.5266648, 0.4657405])),
        entry("Fujifilm Fujicolor 100", "fujifilm-fujicolor100", DyeSet::FujifilmFujicolor100, "fujifilm_fujicolor_100", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5695875, 0.5483339, 0.6555249])),
        entry("Fujifilm Natura 1600", "fujifilm-natura1600", DyeSet::FujifilmNatura1600, "fujifilm_natura_1600", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5137712, 0.5729879, 0.5571788])),
        entry("Fujifilm New Pro 400", "fujifilm-new-pro400", DyeSet::FujifilmNewPro400, "fujifilm_new_pro_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5714649, 0.463539, 0.5471708])),
        entry("Fujifilm Nexia 400", "fujifilm-nexia400", DyeSet::FujifilmNexia400, "fujifilm_nexia_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.56121457, 0.5763065, 0.46141773])),
        entry("Fujifilm Nexia A200", "fujifilm-nexia-a200", DyeSet::FujifilmNexiaA200, "fujifilm_nexia_a200", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5619146, 0.510015, 0.64140844])),
        entry("Fujifilm Nexia Zoom Master 800", "fujifilm-nexia-zoom-master800", DyeSet::FujifilmNexiaZoomMaster800, "fujifilm_nexia_zoom_master_800", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.54714316, 0.5016417, 0.5281989])),
        entry("Fujifilm Pro 160C", "fujifilm-pro160c", DyeSet::FujifilmPro160C, "fujifilm_pro_160c", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.51192236, 0.46926993, 0.6025597])),
        entry("Fujifilm Pro 160NC", "fujifilm-pro160nc", DyeSet::FujifilmPro160NC, "fujifilm_pro_160nc", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5337195, 0.47426224, 0.6095693])),
        entry("Fujifilm Pro 160NH", "fujifilm-pro160nh", DyeSet::FujifilmPro160NH, "fujifilm_pro_160nh", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5604824, 0.4246403, 0.5866013])),
        entry("Fujifilm Pro 160NS", "fujifilm-pro160ns", DyeSet::FujifilmPro160NS, "fujifilm_pro_160ns", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5238152, 0.41176808, 0.55908084])),
        entry("Fujifilm Pro 160S", "fujifilm-pro160s", DyeSet::FujifilmPro160S, "fujifilm_pro_160s", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.49538058, 0.44551528, 0.52102417])),
        entry("Fujifilm Pro 800", "fujifilm-pro800", DyeSet::FujifilmPro800, "fujifilm_pro_800", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5550977, 0.5138419, 0.5210686])),
        entry("Fujifilm Pro 800Z", "fujifilm-pro800z", DyeSet::FujifilmPro800Z, "fujifilm_pro_800z", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5555611, 0.49344462, 0.5074712])),
        entry("Fujifilm Reala Ace", "fujifilm-reala-ace", DyeSet::FujifilmRealaAce, "fujifilm_reala_ace", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.58284837, 0.4900629, 0.576234])),
        entry("Fujifilm Superia 100", "fujifilm-superia100", DyeSet::FujifilmSuperia100, "fujifilm_superia_100", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.45165586, 0.45594406, 0.551365])),
        entry("Fujifilm Superia 200", "fujifilm-superia200", DyeSet::FujifilmSuperia200, "fujifilm_superia_200", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.51352775, 0.5114436, 0.5225248])),
        entry("Fujifilm Superia 1600", "fujifilm-superia1600", DyeSet::FujifilmSuperia1600, "fujifilm_superia_1600", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5137821, 0.573045, 0.5572378])),
        entry("Fujifilm Superia Premium 400", "fujifilm-superia-premium400", DyeSet::FujifilmSuperiaPremium400, "fujifilm_superia_premium_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5715005, 0.4965564, 0.6147678])),
        entry("Fujifilm Superia Reala", "fujifilm-superia-reala", DyeSet::FujifilmSuperiaReala, "fujifilm_superia_reala", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.44121253, 0.4952845, 0.44928432])),
        entry("Fujifilm Superia Venus 400", "fujifilm-superia-venus400", DyeSet::FujifilmSuperiaVenus400, "fujifilm_superia_venus_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4989028, 0.5245314, 0.6009981])),
        entry("Fujifilm Superia Venus 800", "fujifilm-superia-venus800", DyeSet::FujifilmSuperiaVenus800, "fujifilm_superia_venus_800", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5581566, 0.5567312, 0.5972242])),
        entry("Fujifilm Superia X-TRA 800", "fujifilm-superia-xtra800", DyeSet::FujifilmXtra800, "fujifilm_superia_x_tra_800", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.55661094, 0.58114266, 0.5639145])),
        entry("Kodak Portra 160NC", "kodak-portra160nc", DyeSet::KodakPortra160NC, "kodak_portra_160nc", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4546824, 0.35771585, 0.6692486])),
        entry("Kodak Portra 160VC", "kodak-portra160vc", DyeSet::KodakPortra160VC, "kodak_portra_160vc", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.45536554, 0.36813122, 0.68444836])),
        entry("Kodak Portra 400NC", "kodak-portra400nc", DyeSet::KodakPortra400NC, "kodak_portra_400nc", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.45378935, 0.35945702, 0.66195303])),
        entry("Kodak Portra 400VC", "kodak-portra400vc", DyeSet::KodakPortra400VC, "kodak_portra_400vc", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.46913254, 0.3675424, 0.6836019])),
        entry("Foma Fomapan 100", "foma-fomapan100", DyeSet::GenericC41, "foma_fomapan_100", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.64124495, 0.64124495, 0.64124495])),
        entry("Foma Fomapan 200", "foma-fomapan200", DyeSet::GenericC41, "foma_fomapan_200", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.6508648, 0.6508648, 0.6508648])),
        entry("Foma Fomapan 400", "foma-fomapan400", DyeSet::GenericC41, "foma_fomapan_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.6400596, 0.6400596, 0.6400596])),
        entry("Foma Fomapan Cine 100", "foma-fomapan-cine100", DyeSet::GenericC41, "foma_fomapan_cine_100", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.64884645, 0.64884645, 0.64884645])),
        entry("Foma Fomapan Cine 400", "foma-fomapan-cine400", DyeSet::GenericC41, "foma_fomapan_cine_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.65251607, 0.65251607, 0.65251607])),
        entry("Foma Ortho 400", "foma-ortho400", DyeSet::GenericC41, "foma_ortho_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.6923136, 0.6923136, 0.6923136])),
        entry("Foma Cine Ortho 400", "foma-cine-ortho400", DyeSet::GenericC41, "foma_cine_ortho_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.7048051, 0.7048051, 0.7048051])),
        entry("Foma Retropan 320", "foma-retropan320", DyeSet::GenericC41, "foma_retropan_320", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4840337, 0.4840337, 0.4840337])),
        entry("Fujifilm Acros", "fujifilm-acros", DyeSet::GenericC41, "fujifilm_acros_ii", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.55640346, 0.55640346, 0.55640346])),
        entry("Fujifilm Neopan 400", "fujifilm-neopan400", DyeSet::GenericC41, "fujifilm_neopan_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.50119233, 0.50119233, 0.50119233])),
        entry("Fujifilm Neopan 1600", "fujifilm-neopan1600", DyeSet::GenericC41, "fujifilm_neopan_1600", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5704813, 0.5704813, 0.5704813])),
        entry("Fujifilm Neopan Ortho", "fujifilm-neopan-ortho", DyeSet::GenericC41, "fujifilm_neopan_ortho", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.53186786, 0.53186786, 0.53186786])),
        entry("Ilford Delta 100", "ilford-delta100", DyeSet::GenericC41, "ilford_delta_100", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.54539967, 0.54539967, 0.54539967])),
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
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5947504, 0.5947504, 0.5947504])),
        entry("Ilford SFX", "ilford-sfx", DyeSet::GenericC41, "ilford_sfx_200", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5630885, 0.5630885, 0.5630885])),
        entry("Ilford XP2", "ilford-xp2", DyeSet::GenericC41, "ilford_xp2_super", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.58853966, 0.58853966, 0.58853966])),
        entry("Kodak Double-X", "kodak-double-x", DyeSet::GenericC41, "kodak_double_x", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5464296, 0.5464296, 0.5464296])),
        entry("Kodak T-Max 100", "kodak-tmax100", DyeSet::GenericC41, "kodak_t_max_100", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5124686, 0.5124686, 0.5124686])),
        entry("Kodak T-Max 400", "kodak-tmax400", DyeSet::GenericC41, "kodak_t_max_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.47859782, 0.47859782, 0.47859782])),
        entry("Kodak T-Max P3200", "kodak-tmax-p3200", DyeSet::GenericC41, "kodak_t_max_p3200", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.6569006, 0.6569006, 0.6569006])),
        entry("Kodak Tri-X 320", "kodak-320txp", DyeSet::GenericC41, "kodak_tri_x_320", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.49478894, 0.49478894, 0.49478894])),
        entry("Kodak Tri-X 400", "kodak-400tx", DyeSet::GenericC41, "kodak_tri_x_400", Daylight, reference(
            [0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5753634, 0.5753634, 0.5753634])),
    ]
};
