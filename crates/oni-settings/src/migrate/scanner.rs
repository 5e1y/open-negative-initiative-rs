//! Before format 79 a still colour negative's virtual scan was its paper's gaze and a cine negative's a camera's, the
//! 1931 observer under a 5500 K lamp. ONI's scanner reads them all now: a digital photo moves onto its stock's new
//! reference, every hand's move kept.

use super::chain::UNIVERSAL_SCANNER_VERSION;
use super::datasheet::onto;
use crate::PipelineSettings;
use crate::virtual_film::{StockReference, VirtualStock};

const fn reference(stops: [f32; 3], black: [f32; 3], white: [f32; 3]) -> StockReference {
    StockReference { stops, black, white }
}

/// Each colour negative the scanner came to read, its reference as format 78 laid it.
const FORMERLY_READ: [(VirtualStock, StockReference); 48] = {
    [
        (VirtualStock::of("fujifilm-160nc"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5183451, 0.4695056, 0.48685804])),
        (VirtualStock::of("fujifilm-160nl"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.49459338, 0.45696637, 0.46693733])),
        (VirtualStock::of("fujifilm-160npc"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5115253, 0.4600402, 0.41927812])),
        (VirtualStock::of("fujifilm-160ns"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.524941, 0.44916874, 0.4476556])),
        (VirtualStock::of("fujifilm-200"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.46511993, 0.42682335, 0.45707515])),
        (VirtualStock::of("fujifilm-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.49420443, 0.44835538, 0.48240706])),
        (VirtualStock::of("fujifilm-c200"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5143732, 0.47364444, 0.45144385])),
        (VirtualStock::of("fujifilm-fujicolor-100"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.60725284, 0.541034, 0.5500801])),
        (VirtualStock::of("fujifilm-natura-1600"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5790791, 0.5321547, 0.54269034])),
        (VirtualStock::of("fujifilm-new-pro-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5402905, 0.4532553, 0.4424183])),
        (VirtualStock::of("fujifilm-nexia-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.502962, 0.49393192, 0.5430495])),
        (VirtualStock::of("fujifilm-nexia-a200"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.49941188, 0.47630435, 0.4798122])),
        (VirtualStock::of("fujifilm-nexia-zoom-master-800"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.54152304, 0.48787445, 0.5132035])),
        (VirtualStock::of("fujifilm-pro-160c"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5326091, 0.46069703, 0.43217593])),
        (VirtualStock::of("fujifilm-pro-160nc"), reference([-8.599133e-08, 0.0, -8.599133e-08], [0.104436636, 0.104436636, 0.104436636], [0.5418063, 0.46084177, 0.43516004])),
        (VirtualStock::of("fujifilm-pro-160nh"), reference([0.0, 0.0, 1.7198263e-07], [0.10443662, 0.10443662, 0.10443662], [0.5328125, 0.43734676, 0.39350298])),
        (VirtualStock::of("fujifilm-pro-160ns"), reference([-8.599133e-08, 0.0, -8.599133e-08], [0.104436636, 0.104436636, 0.104436636], [0.49325106, 0.41096348, 0.38065532])),
        (VirtualStock::of("fujifilm-pro-160s"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.48403403, 0.4205459, 0.39825422])),
        (VirtualStock::of("fujifilm-pro-400h"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.52111685, 0.4399502, 0.41831246])),
        (VirtualStock::of("fujifilm-pro-800"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.57702804, 0.4794521, 0.46035284])),
        (VirtualStock::of("fujifilm-pro-800z"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.57091844, 0.47145084, 0.44939524])),
        (VirtualStock::of("fujifilm-reala-ace"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5233105, 0.48266184, 0.498997])),
        (VirtualStock::of("fujifilm-superia-100"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5336138, 0.51304364, 0.48751506])),
        (VirtualStock::of("fujifilm-superia-1600"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5791241, 0.53219134, 0.5427335])),
        (VirtualStock::of("fujifilm-superia-200"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5438273, 0.52572316, 0.49151066])),
        (VirtualStock::of("fujifilm-superia-premium-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.58208406, 0.51284045, 0.4884329])),
        (VirtualStock::of("fujifilm-superia-reala"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.53245807, 0.48433602, 0.44874656])),
        (VirtualStock::of("fujifilm-superia-venus-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5453657, 0.5124852, 0.49855927])),
        (VirtualStock::of("fujifilm-superia-venus-800"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.57803005, 0.5201994, 0.5287955])),
        (VirtualStock::of("fujifilm-xtra-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.51560014, 0.49265444, 0.47441983])),
        (VirtualStock::of("fujifilm-xtra-800"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5498349, 0.52563393, 0.54786503])),
        (VirtualStock::of("kodak-ektar-100"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.50933033, 0.484749, 0.5114777])),
        (VirtualStock::of("kodak-gold-200"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.50299317, 0.48177767, 0.46005815])),
        (VirtualStock::of("kodak-portra-160"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.49976432, 0.47865433, 0.48564404])),
        (VirtualStock::of("kodak-portra-160nc"), reference([1.7198263e-07, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5033095, 0.4650072, 0.5362317])),
        (VirtualStock::of("kodak-portra-160vc"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5243281, 0.4736032, 0.5488911])),
        (VirtualStock::of("kodak-portra-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.50708425, 0.4775318, 0.48832908])),
        (VirtualStock::of("kodak-portra-400nc"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5040446, 0.46450332, 0.5357539])),
        (VirtualStock::of("kodak-portra-400vc"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5248003, 0.4791457, 0.5557933])),
        (VirtualStock::of("kodak-portra-800"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4944812, 0.4784341, 0.4846847])),
        (VirtualStock::of("kodak-portra-800-push-1"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.5138217, 0.5108578, 0.51248485])),
        (VirtualStock::of("kodak-portra-800-push-2"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.49694368, 0.49893385, 0.47323388])),
        (VirtualStock::of("kodak-ultramax-400"), reference([0.0, 0.0, 0.0], [0.10443662, 0.10443662, 0.10443662], [0.4889207, 0.47498095, 0.47217107])),
        (VirtualStock::of("kodak-verita-200d"), reference([-1.0046501, 0.0, 0.9290177], [0.2943861, 0.29429713, 0.294137], [0.5563389, 0.5832267, 0.66381735])),
        (VirtualStock::of("kodak-vision3-200t"), reference([-0.8056435, 0.0, 0.88720316], [0.26404598, 0.26398596, 0.26386678], [0.5184133, 0.5712007, 0.59905165])),
        (VirtualStock::of("kodak-vision3-250d"), reference([-0.73414564, 0.0, 1.0144322], [0.2623733, 0.26231802, 0.2621768], [0.5130873, 0.5661374, 0.59351265])),
        (VirtualStock::of("kodak-vision3-500t"), reference([-0.7581185, 0.0, 0.91422534], [0.265473, 0.265415, 0.2652896], [0.50690967, 0.559963, 0.58556056])),
        (VirtualStock::of("kodak-vision3-50d"), reference([-0.89372385, 0.0, 0.9421484], [0.2588959, 0.2588336, 0.2587094], [0.5155934, 0.56569976, 0.5944729])),
    ]
};

/// The reference a document of format 78 laid for `stock`; a stock this step leaves unmoved, its own.
pub fn formerly_read(stock: VirtualStock) -> StockReference {
    FORMERLY_READ.iter().find(|(s, _)| *s == stock).map_or_else(|| stock.reference(), |(_, r)| *r)
}

/// `settings` of `version` on its stock's reference as the scanner reads it.
pub fn caught_up(version: i64, settings: PipelineSettings) -> PipelineSettings {
    match settings.film() {
        Some(film) if version < UNIVERSAL_SCANNER_VERSION => onto(settings, formerly_read(film.stock), film.stock.reference()),
        _ => settings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConversionMode;

    #[test]
    fn a_digital_photo_lands_on_the_scanners_reading_its_hand_kept() {
        let stock = VirtualStock::of("kodak-portra-800");
        let mut photo = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(stock);
        let (was, now) = (formerly_read(stock), stock.reference());
        assert!((was.stops[0] - now.stops[0]).abs() > 0.05, "the scanner reads the orange base the paper's gaze laid on one");
        photo.gains.stops = was.stops.map(|v| v + 0.25);
        photo.levels = was.levels();
        photo.levels.green.white += 0.03;
        let landed = caught_up(UNIVERSAL_SCANNER_VERSION - 1, photo.clone());
        assert!((0..3).all(|c| (landed.gains.stops[c] - (now.stops[c] + 0.25)).abs() < 1e-5), "{:?}", landed.gains.stops);
        assert!((landed.levels.green.white - (now.levels().green.white + 0.03)).abs() < 1e-5, "{:?}", landed.levels.green);
        // Twins: a photo of this format stays where it stands, a black and white film and a scan move nothing.
        assert_eq!(caught_up(UNIVERSAL_SCANNER_VERSION, photo.clone()), photo);
        let silver = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(VirtualStock::of("ilford-hp5-plus"));
        assert_eq!(caught_up(UNIVERSAL_SCANNER_VERSION - 1, silver.clone()), silver);
        let scan = PipelineSettings::rest(ConversionMode::Negative, true);
        assert_eq!(caught_up(UNIVERSAL_SCANNER_VERSION - 1, scan.clone()), scan);
    }
}
