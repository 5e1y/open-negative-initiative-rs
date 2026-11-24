//! A digital photo before format 71 stood on its stock's reference with each layer's density coded on its own channel:
//! it moves onto the reference of its negative scanned through its dyes and base, every hand's move kept; its render
//! moves on purpose (owner's model).

use super::chain::DIGITAL_SCAN_VERSION;
use super::datasheet::onto;
use crate::PipelineSettings;
use crate::virtual_film::{StockReference, VirtualStock};

const fn reference(stops: [f32; 3], black: [f32; 3], white: [f32; 3]) -> StockReference {
    StockReference { stops, black, white }
}

/// Each colour negative with the reference formats 69 and 70 derived, each layer's density on its own channel.
const LAYERED: [(VirtualStock, StockReference); 48] = {
    [
        (VirtualStock::of("fujifilm-160nc"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5164758, 0.50035465, 0.8006314])),
        (VirtualStock::of("fujifilm-160nl"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4846192, 0.46831, 0.67295873])),
        (VirtualStock::of("fujifilm-160npc"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5737342, 0.48222595, 0.5326451])),
        (VirtualStock::of("fujifilm-160ns"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.49376518, 0.46587342, 0.6951522])),
        (VirtualStock::of("fujifilm-200"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.45958436, 0.47943157, 0.454723])),
        (VirtualStock::of("fujifilm-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5002437, 0.5266648, 0.4657405])),
        (VirtualStock::of("fujifilm-c200"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4784751, 0.4726876, 0.5667638])),
        (VirtualStock::of("fujifilm-fujicolor-100"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5695875, 0.5483339, 0.6555249])),
        (VirtualStock::of("fujifilm-natura-1600"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5137712, 0.5729879, 0.5571788])),
        (VirtualStock::of("fujifilm-new-pro-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5714649, 0.463539, 0.5471708])),
        (VirtualStock::of("fujifilm-nexia-a200"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5619146, 0.510015, 0.64140844])),
        (VirtualStock::of("fujifilm-nexia-zoom-master-800"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.54714316, 0.5016417, 0.5281989])),
        (VirtualStock::of("fujifilm-nexia-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.56121457, 0.5763065, 0.46141773])),
        (VirtualStock::of("fujifilm-pro-160c"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.51192236, 0.46926993, 0.6025597])),
        (VirtualStock::of("fujifilm-pro-160nc"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5337195, 0.47426224, 0.6095693])),
        (VirtualStock::of("fujifilm-pro-160nh"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5604824, 0.4246403, 0.5866013])),
        (VirtualStock::of("fujifilm-pro-160ns"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5238152, 0.41176808, 0.55908084])),
        (VirtualStock::of("fujifilm-pro-160s"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.49538058, 0.44551528, 0.52102417])),
        (VirtualStock::of("fujifilm-pro-400h"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5207033, 0.4615963, 0.50880706])),
        (VirtualStock::of("fujifilm-pro-800"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5550977, 0.5138419, 0.5210686])),
        (VirtualStock::of("fujifilm-pro-800z"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5555611, 0.49344462, 0.5074712])),
        (VirtualStock::of("fujifilm-reala-ace"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.58284837, 0.4900629, 0.576234])),
        (VirtualStock::of("fujifilm-superia-premium-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5715005, 0.4965564, 0.6147678])),
        (VirtualStock::of("fujifilm-superia-reala"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.44121253, 0.4952845, 0.44928432])),
        (VirtualStock::of("fujifilm-superia-venus-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4989028, 0.5245314, 0.6009981])),
        (VirtualStock::of("fujifilm-superia-venus-800"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5581566, 0.5567312, 0.5972242])),
        (VirtualStock::of("fujifilm-xtra-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5355894, 0.4963566, 0.59353447])),
        (VirtualStock::of("fujifilm-xtra-800"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.55661094, 0.58114266, 0.5639145])),
        (VirtualStock::of("fujifilm-superia-100"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.45165586, 0.45594406, 0.551365])),
        (VirtualStock::of("fujifilm-superia-1600"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5137918, 0.57305247, 0.5572549])),
        (VirtualStock::of("fujifilm-superia-200"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.51369345, 0.5113918, 0.52141744])),
        (VirtualStock::of("kodak-ektar-100"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5317709, 0.46712714, 0.64566344])),
        (VirtualStock::of("kodak-gold-200"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.49530482, 0.46685815, 0.5301817])),
        (VirtualStock::of("kodak-portra-160"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.48944205, 0.46455348, 0.5802063])),
        (VirtualStock::of("kodak-portra-160nc"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4546789, 0.35771674, 0.6692456])),
        (VirtualStock::of("kodak-portra-160vc"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4553665, 0.36813986, 0.68444026])),
        (VirtualStock::of("kodak-portra-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.49785548, 0.45614183, 0.62589794])),
        (VirtualStock::of("kodak-portra-400nc"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.45378935, 0.35945702, 0.66195303])),
        (VirtualStock::of("kodak-portra-400vc"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.46913254, 0.3675424, 0.6836019])),
        (VirtualStock::of("kodak-portra-800"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.47139537, 0.46853244, 0.52487904])),
        (VirtualStock::of("kodak-portra-800-push-1"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.50858635, 0.46855438, 0.5184489])),
        (VirtualStock::of("kodak-portra-800-push-2"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.43024963, 0.42104852, 0.4342835])),
        (VirtualStock::of("kodak-ultramax-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.47177684, 0.4588763, 0.527811])),
        (VirtualStock::of("kodak-verita-200d"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.52952355, 0.45191997, 0.4524775])),
        (VirtualStock::of("kodak-vision3-200t"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.47857493, 0.47625405, 0.47741538])),
        (VirtualStock::of("kodak-vision3-250d"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.45321292, 0.46328843, 0.4858058])),
        (VirtualStock::of("kodak-vision3-500t"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4464389, 0.42824948, 0.4972486])),
        (VirtualStock::of("kodak-vision3-50d"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4631214, 0.46999073, 0.49043834])),
    ]
};

/// The reference a document of formats 69 and 70 laid for `stock`; a stock the scan leaves unmoved, its own.
pub fn layered(stock: VirtualStock) -> StockReference {
    LAYERED.iter().find(|(s, _)| *s == stock).map_or_else(|| stock.reference(), |(_, r)| *r)
}

/// `settings` of `version` on its stock's reference of the scanned negative.
pub fn caught_up(version: i64, settings: PipelineSettings) -> PipelineSettings {
    match settings.film() {
        Some(film) if version < DIGITAL_SCAN_VERSION => onto(settings, layered(film.stock), film.stock.reference()),
        _ => settings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConversionMode;

    #[test]
    fn a_digital_photo_on_the_layers_reference_lands_on_the_scans_its_hand_kept() {
        let stock = VirtualStock::of("kodak-portra-800");
        let mut photo = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(stock);
        let was = layered(stock);
        photo.gains.stops = was.stops.map(|v| v + 0.25);
        photo.levels = was.levels();
        photo.levels.blue.black -= 0.02;
        let landed = caught_up(DIGITAL_SCAN_VERSION - 1, photo.clone());
        let now = stock.reference();
        assert!(now.stops != was.stops, "the scan moves Portra 800's reference");
        assert!((0..3).all(|c| (landed.gains.stops[c] - (now.stops[c] + 0.25)).abs() < 1e-6), "{:?}", landed.gains.stops);
        assert!((landed.levels.blue.black - (now.levels().blue.black - 0.02)).abs() < 1e-5, "{:?}", landed.levels.blue);
        // Twin: a photo of this format stays where it stands.
        assert_eq!(caught_up(DIGITAL_SCAN_VERSION, photo.clone()).gains, photo.gains);
        // A black and white film's reference is unmoved, and a scan moves nothing.
        let silver = VirtualStock::of("ilford-hp5-plus");
        assert_eq!(layered(silver), silver.reference());
        let scan = PipelineSettings::rest(ConversionMode::Negative, true);
        assert_eq!(caught_up(DIGITAL_SCAN_VERSION - 1, scan.clone()).levels, scan.levels);
    }
}
