//! A digital photo a sidecar before format 56 kept may read its negative on another film's gains and dyes, which its
//! folder's roll laid when a film was picked on another photo of it: it moves back onto its own film's.

use super::chain::OWN_FILM_VERSION;
use super::datasheet::{former, on_former};
use crate::PipelineSettings;
use crate::virtual_film::VirtualStock;

/// Within a float's round trip through the sidecar, so only a reference laid whole is recognised, never a hand's move.
const SAME: f32 = 1e-6;

/// The settings on their own film's gains and dyes, where they stood on another stock's whole.
pub fn caught_up(version: i64, settings: PipelineSettings) -> PipelineSettings {
    if version >= OWN_FILM_VERSION {
        return settings;
    }
    let Some(film) = settings.film() else { return settings };
    let held = |stock: VirtualStock| {
        let stops = former(stock).stops;
        settings.dye_set == stock.dye_set() && (0..3).all(|c| (settings.gains.stops[c] - stops[c]).abs() < SAME)
    };
    // A black and white film came after format 56: no photo then stood on its reference.
    if held(film.stock) || !VirtualStock::ALL.into_iter().filter(|s| !s.is_silver()).any(held) {
        return settings;
    }
    let own = on_former(&settings, film.stock);
    PipelineSettings { gains: own.gains, dye_set: own.dye_set, ..settings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConversionMode;

    fn on(film: VirtualStock, laid: VirtualStock) -> PipelineSettings {
        let mut s = on_former(&PipelineSettings::rest(ConversionMode::Digital, true), film);
        (s.gains.stops, s.dye_set) = (former(laid).stops, laid.dye_set());
        s
    }

    #[test]
    fn a_photo_read_on_another_films_gains_and_dyes_moves_onto_its_own() {
        let older = OWN_FILM_VERSION - 1;
        let moved = caught_up(older, on(VirtualStock::KodakVerita200D, VirtualStock::KodakVision3_200T));
        let own = on_former(&PipelineSettings::rest(ConversionMode::Digital, true), VirtualStock::KodakVerita200D);
        assert_eq!((moved.gains, moved.dye_set, moved.levels), (own.gains, own.dye_set, own.levels));
        // Twins: a hand's gains, a photo on its own film and a file of this format stay as they are.
        let mut hand = on(VirtualStock::KodakVerita200D, VirtualStock::KodakVision3_200T);
        hand.gains.stops[2] += 0.05;
        assert_eq!(caught_up(older, hand.clone()), hand);
        let push = on(VirtualStock::KodakPortra800Push1, VirtualStock::KodakPortra800);
        assert_eq!(caught_up(older, push.clone()), push, "a push stands on Portra 800's own gains and dyes");
        let laid = on(VirtualStock::KodakVerita200D, VirtualStock::KodakVision3_200T);
        assert_eq!(caught_up(OWN_FILM_VERSION, laid.clone()), laid);
        // Neutral gains on the generic dyes, a black and white film's reference, which no photo then stood on.
        let mut neutral = on(VirtualStock::KodakVerita200D, VirtualStock::IlfordHP5Plus);
        (neutral.gains.stops, neutral.dye_set) = ([0.0; 3], crate::DyeSet::GenericC41);
        assert_eq!(caught_up(older, neutral.clone()), neutral);
    }
}
