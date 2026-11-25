//! Before format 75 the halation moved the light past a threshold, its Amount and Size each a share of a track and its
//! sensitivity that threshold's stops. It is a dose of each layer's exposure now, its width on the film: each keeps its
//! place on its track, the sensitivity gone with the threshold. A digital photo's effect still at its former rest takes
//! the rest its film gives, as a new rest reaches every frame left on it.

use super::chain::HALATION_ON_LAYERS_VERSION;
use crate::PipelineSettings;
use crate::blocks::{Grain, Halation};
use crate::tolerant;
use serde_json::Value;

/// The former law: the narrowest glow a share of the scan's width, Size's top four times it, its rest halfway.
const FORMER_NARROWEST: f64 = 0.001;
const FORMER_WIDEST: f64 = 4.0;
const FORMER_REST: f64 = 0.5;

/// The deviation on the film a former `share` laid on a scan as wide as its long side.
pub fn microns(share: f64) -> f32 {
    (FORMER_NARROWEST * libm::pow(FORMER_WIDEST, share) * Grain::FRAME_LONG) as f32
}

/// The settings with the halation as a dose on the film's layers; `stored` is the document's own settings object as
/// written.
pub fn caught_up(version: i64, mut settings: PipelineSettings, stored: Option<&Value>) -> PipelineSettings {
    if version >= HALATION_ON_LAYERS_VERSION {
        return settings;
    }
    let rests = settings.film().map(|f| f.stock.effects()).unwrap_or_default();
    if settings.halation.amount == 0.0 {
        settings.halation = rests.halation;
    } else {
        let share = tolerant::ns_number(stored.and_then(|s| s.get("halation")).and_then(|h| h.get("size"))).filter(|v| v.is_finite()).unwrap_or(FORMER_REST);
        settings.halation.size = microns(share.clamp(0.0, 1.0));
        settings.halation.amount = settings.halation.amount.clamp(0.0, 1.0) * Halation::AMOUNT_RANGE.1;
    }
    settings.bloom = rests.bloom;
    settings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConversionMode;
    use serde_json::json;

    #[test]
    fn a_former_halation_keeps_its_place_on_each_track() {
        let older = HALATION_ON_LAYERS_VERSION - 1;
        let mut s = PipelineSettings::rest(ConversionMode::Digital, true);
        s.halation.amount = 0.6;
        let stored = json!({ "halation": { "amount": 0.6, "size": 1.0, "sensitivity": 1.5 } });
        let moved = caught_up(older, s.clone(), Some(&stored));
        assert_eq!((moved.halation.amount, moved.halation.size), (0.6 * Halation::AMOUNT_RANGE.1, 144.0));
        // A size never written rested halfway: twice the narrowest glow.
        assert_eq!(caught_up(older, s.clone(), Some(&json!({}))).halation.size, 72.0);
        // Twin: a document of this format is read as it is.
        assert_eq!(caught_up(HALATION_ON_LAYERS_VERSION, s.clone(), Some(&stored)), s);
    }

    #[test]
    fn an_effect_at_its_former_rest_takes_its_films_and_a_hands_stays() {
        let older = HALATION_ON_LAYERS_VERSION - 1;
        let stock = crate::VirtualStock::of("kodak-portra-800");
        let rests = stock.effects();
        assert!(rests.halation.amount > 0.0 && rests.bloom.size > 0.0, "the film gives its own: {rests:?}");
        let on_film = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(stock);
        let read = caught_up(older, on_film.clone(), Some(&json!({})));
        assert_eq!((read.halation, read.bloom), (rests.halation, rests.bloom));
        // Twin: a halation the hand laid keeps its place.
        let mut held = on_film.clone();
        held.halation.amount = 0.2;
        let read = caught_up(older, held, Some(&json!({ "halation": { "amount": 0.2, "size": 0.5 } })));
        assert_eq!((read.halation.amount, read.halation.size), (0.2 * Halation::AMOUNT_RANGE.1, 72.0));
        // The neutral film gives no effect: a new photo's rest.
        let neutral = caught_up(older, PipelineSettings::rest(ConversionMode::Digital, true), Some(&json!({})));
        assert_eq!((neutral.halation, neutral.bloom), (Halation::default(), crate::blocks::Bloom::default()));
    }
}
