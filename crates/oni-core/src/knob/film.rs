//! A digital photo's virtual film as its knobs read and write it: a photo on no film has none to move.

use super::Knob;
use oni_settings::PipelineSettings;

/// The narrowest share of the window a virtual negative's ends may leave between them.
pub const FILM_MINIMUM_SPAN: f32 = 0.02;

pub(super) fn get(knob: Knob, s: &PipelineSettings) -> f32 {
    let Some(f) = s.virtual_film.as_ref() else { return if matches!(knob, Knob::FilmWhite(_)) { 1.0 } else { 0.0 } };
    match knob {
        Knob::FilmGain(c) => f.stops[c.index()],
        Knob::FilmBlack(c) => f.black[c.index()],
        Knob::FilmWhite(c) => f.white[c.index()],
        _ => f.exposure,
    }
}

/// `v` already held to the knob's range; each end keeps the black under the white.
pub(super) fn set(knob: Knob, s: &mut PipelineSettings, v: f32) {
    let Some(f) = s.virtual_film.as_mut() else { return };
    match knob {
        Knob::FilmGain(c) => f.stops[c.index()] = v,
        Knob::FilmBlack(c) => f.black[c.index()] = v.min(f.white[c.index()] - FILM_MINIMUM_SPAN),
        Knob::FilmWhite(c) => f.white[c.index()] = v.max(f.black[c.index()] + FILM_MINIMUM_SPAN),
        _ => f.exposure = v,
    }
}
