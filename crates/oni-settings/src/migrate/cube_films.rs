//! A digital photo on a film a person's `.cube` file made, keyed `lut-`, before format 84: no cube is a film any more
//! (owner's call), so it lands on the neutral film, its gains and windows on that film's reference, every other edit kept.

use super::chain::PAPERS_REDONE_VERSION;
use crate::{PipelineSettings, VirtualStock};

/// The prefix every film a cube made was keyed under.
const CUBE_KEY: &str = "lut-";

/// `settings` of `version` on a film that still is one, every step before this one done.
pub fn caught_up(version: i64, settings: PipelineSettings) -> PipelineSettings {
    let cubed = settings.virtual_film.is_some_and(|f| f.stock.key().starts_with(CUBE_KEY));
    if version >= PAPERS_REDONE_VERSION || !cubed {
        return settings;
    }
    settings.on_stock(VirtualStock::NEUTRAL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConversionMode, VirtualFilm};

    #[test]
    fn a_photo_on_a_cubes_film_lands_on_the_neutral_film_with_its_exposure_and_its_effects() {
        let older = PAPERS_REDONE_VERSION - 1;
        let mut s = PipelineSettings::rest(ConversionMode::Digital, true);
        s.virtual_film = Some(VirtualFilm { stock: VirtualStock::of("lut-my-look"), exposure: 0.5 });
        (s.grain.amount, s.print.exposure) = (0.3, -0.2);
        let read = caught_up(older, s.clone());
        assert_eq!(read.virtual_film, Some(VirtualFilm { stock: VirtualStock::NEUTRAL, exposure: 0.5 }));
        assert_eq!(read.levels, VirtualStock::NEUTRAL.reference().levels());
        assert_eq!((read.grain.amount, read.print.exposure), (0.3, -0.2), "its edits kept");
        // Twins: a photo on a shipped film, and one of this format, move nothing.
        let portra = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(VirtualStock::of("kodak-portra-400"));
        assert_eq!(caught_up(older, portra.clone()), portra);
        assert_eq!(caught_up(PAPERS_REDONE_VERSION, s.clone()), s);
    }
}
