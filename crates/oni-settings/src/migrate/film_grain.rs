//! Before format 78 every film's grain rested at a new photo's, Amount 0.25 and 30 µm. A film's file now gives its own:
//! a digital photo's grain still at that rest takes its film's, as a new rest reaches every frame left on it.

use super::chain::FILM_GRAIN_VERSION;
use crate::PipelineSettings;
use crate::blocks::Grain;

/// The settings with the grain of their film where theirs rests; a scan's are left, its mode rendering none.
pub fn caught_up(version: i64, mut settings: PipelineSettings) -> PipelineSettings {
    let Some(film) = settings.film().filter(|_| version < FILM_GRAIN_VERSION) else { return settings };
    if settings.grain == (Grain { amount: Grain::DIGITAL_AMOUNT, size: Grain::SIZE_REST }) {
        settings.grain = film.stock.effects().grain;
    }
    settings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConversionMode, VirtualStock};

    #[test]
    fn a_grain_at_the_common_rest_takes_its_films_and_a_hands_stays() {
        let older = FILM_GRAIN_VERSION - 1;
        let stock = VirtualStock::of("kodak-portra-400");
        let mut s = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(stock);
        s.grain = Grain { amount: Grain::DIGITAL_AMOUNT, size: Grain::SIZE_REST };
        assert_eq!(caught_up(older, s.clone()).grain, stock.effects().grain);
        // Twins: a hand's grain stays, and a document of this format is read as it is.
        s.grain.size = 41.0;
        assert_eq!(caught_up(older, s.clone()).grain, s.grain);
        assert_eq!(caught_up(FILM_GRAIN_VERSION, s.clone()), s);
    }
}
