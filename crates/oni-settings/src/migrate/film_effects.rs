//! Before format 76 a film's halation was a dose of each layer's whole exposure over one and the dose, a hundredth on
//! most films, and a film's grain width came from its sheet's granularity. The glow now adds a dose of each layer's clipped
//! light and every film's grain rests at 30 µm: an effect still at its former rest takes its film's new one, a halation
//! the hand laid keeps its place on its track.

use super::chain::{FILM_EFFECTS_VERSION, HALATION_ON_LAYERS_VERSION};
use crate::PipelineSettings;
use crate::blocks::{Grain, Halation};

/// The grain's rest before format 76, every film's but those whose sheet gave their own.
const FORMER_GRAIN_REST: f32 = 27.6;
/// Halation's top at format 75.
const FORMER_HALATION_TOP: f32 = 0.25;
/// The halation's rests at format 75, by its base's family: none, an anti-halation layer; their widths.
const FORMER_DOSES: [f32; 2] = [0.0, 0.01];
const FORMER_WIDTHS: [f32; 2] = [122.0, 125.0];
/// Each film's grain width at format 75, its sheet's granularity read through the former grain.
const FORMER_SIZES: [(&str, f32); 30] = [
    ("foma-cine-ortho-400", 60.0), ("foma-fomapan-cine-100", 60.0), ("foma-fomapan-cine-400", 60.0),
    ("foma-fomapan-100", 60.0), ("foma-fomapan-200", 60.0), ("foma-fomapan-400", 60.0), ("foma-ortho-400", 60.0),
    ("fujifilm-160nc", 14.3), ("fujifilm-160nl", 14.3), ("fujifilm-160npc", 14.3), ("fujifilm-160ns", 14.3),
    ("fujifilm-fujicolor-100", 14.3), ("fujifilm-natura-1600", 28.6), ("fujifilm-new-pro-400", 14.3),
    ("fujifilm-nexia-a200", 14.3), ("fujifilm-nexia-zoom-master-800", 18.7), ("fujifilm-nexia-400", 14.3),
    ("fujifilm-pro-160nc", 10.6), ("fujifilm-pro-160nh", 10.6), ("fujifilm-pro-160ns", 10.6),
    ("fujifilm-pro-800", 18.7), ("fujifilm-reala-ace", 14.3), ("fujifilm-superia-premium-400", 14.3),
    ("fujifilm-superia-reala", 14.3), ("fujifilm-superia-venus-400", 14.3), ("fujifilm-superia-venus-800", 18.7),
    ("fujifilm-xtra-800", 18.7), ("fujifilm-superia-100", 14.3), ("fujifilm-superia-1600", 28.6),
    ("fujifilm-superia-200", 14.3),
];

/// The settings with the film effects of this format; a scan's are left, its mode rendering none.
pub fn caught_up(version: i64, mut settings: PipelineSettings) -> PipelineSettings {
    let Some(film) = settings.film().filter(|_| version < FILM_EFFECTS_VERSION) else { return settings };
    if version >= HALATION_ON_LAYERS_VERSION {
        let h = settings.halation;
        if FORMER_DOSES.contains(&h.amount) && FORMER_WIDTHS.contains(&h.size) {
            settings.halation = film.stock.effects().halation;
        } else {
            settings.halation.amount = (h.amount / FORMER_HALATION_TOP * Halation::AMOUNT_RANGE.1).min(Halation::AMOUNT_RANGE.1);
        }
    }
    let former = FORMER_SIZES.iter().find(|(key, _)| *key == film.stock.key()).map(|(_, size)| *size);
    if settings.grain.size == FORMER_GRAIN_REST || Some(settings.grain.size) == former {
        settings.grain.size = Grain::SIZE_REST;
    }
    settings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConversionMode, VirtualStock};

    fn on(key: &'static str) -> PipelineSettings {
        PipelineSettings::rest(ConversionMode::Digital, true).on_stock(VirtualStock::of(key))
    }

    #[test]
    fn an_effect_at_its_former_rest_takes_its_films_new_one() {
        let older = FILM_EFFECTS_VERSION - 1;
        let mut s = on("fujifilm-natura-1600");
        (s.halation.amount, s.halation.size, s.grain.size) = (0.01, 125.0, 28.6);
        let read = caught_up(older, s.clone());
        assert_eq!((read.halation, read.grain.size), (VirtualStock::of("fujifilm-natura-1600").effects().halation, Grain::SIZE_REST));
        // Twin: a hand's halation keeps its place on its track, a hand's grain width its value.
        (s.halation.amount, s.grain.size) = (0.1, 40.0);
        let read = caught_up(older, s.clone());
        assert_eq!((read.halation.amount, read.halation.size, read.grain.size), (0.4, 125.0, 40.0));
        // A document of this format is read as it is.
        assert_eq!(caught_up(FILM_EFFECTS_VERSION, s.clone()), s);
    }

    #[test]
    fn a_document_before_75_keeps_the_halation_its_own_step_laid() {
        let mut s = on("kodak-portra-800");
        (s.halation.amount, s.halation.size, s.grain.size) = (0.3, 144.0, FORMER_GRAIN_REST);
        let read = caught_up(HALATION_ON_LAYERS_VERSION - 1, s.clone());
        assert_eq!((read.halation, read.grain.size), (s.halation, Grain::SIZE_REST));
        // Twin: a scan's settings stay, its mode rendering no effect.
        let mut scan = PipelineSettings::rest(ConversionMode::Negative, true);
        scan.grain.size = FORMER_GRAIN_REST;
        assert_eq!(caught_up(HALATION_ON_LAYERS_VERSION - 1, scan.clone()), scan);
    }
}
