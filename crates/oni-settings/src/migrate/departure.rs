//! A photo a sidecar before format 64 kept held Lighting's Exposure on its mode's former rest, a negative's 1.5 and a
//! slide's 0: Exposure came to be the departure from its pipe's rest, so each value moves by that rest, its render kept.

use super::chain::{DIGITAL_ON_FILM_VERSION, EXPOSURE_DEPARTURE_VERSION};
use crate::{ConversionMode, PipelineSettings};
use crate::tone::Print;

/// A negative's Exposure at rest before format 64.
pub const FORMER_REST: f32 = 1.5;

/// Exposure's rest in `mode` before format 64: a negative's `FORMER_REST`, a slide's 0.
pub fn former_rest(mode: ConversionMode) -> f32 {
    if mode.reads_positive() { 0.0 } else { FORMER_REST }
}

/// `settings` of `version` with Exposure as its departure from its pipe's rest, every step before this one done.
pub fn caught_up(version: i64, settings: PipelineSettings) -> PipelineSettings {
    if version >= EXPOSURE_DEPARTURE_VERSION {
        return settings;
    }
    let exposure = settings.print.exposure - former_rest(settings.mode);
    PipelineSettings { print: Print { exposure, ..settings.print }, ..settings }
}

/// `settings` as a format before 64 stored them, Exposure on its mode's former rest: what `caught_up` reads back.
pub fn former(settings: PipelineSettings) -> PipelineSettings {
    let exposure = settings.print.exposure + former_rest(settings.mode);
    PipelineSettings { print: Print { exposure, ..settings.print }, ..settings }
}

/// `document` as a build of `version` wrote it, Exposure in that format's units: a digital photo before format 38
/// rested at 0 on a pipe of its own.
pub fn stored_at(document: &crate::Document, version: i64) -> crate::Document {
    let own_pipe = document.settings.mode.is_digital() && version < DIGITAL_ON_FILM_VERSION;
    if version >= EXPOSURE_DEPARTURE_VERSION || own_pipe {
        return document.clone();
    }
    crate::Document { settings: former(document.settings.clone()), ..document.clone() }
}

/// What the Exposure steps read of a scan's `settings` a build of `version` wrote: `former`, gear and rest undone.
pub fn read_back(version: i64, settings: PipelineSettings) -> PipelineSettings {
    if version >= EXPOSURE_DEPARTURE_VERSION {
        return settings;
    }
    caught_up(version, super::exposure::caught_up(version, former(settings)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_former_exposure_moves_by_its_modes_rest_and_a_document_of_this_format_stays() {
        let older = EXPOSURE_DEPARTURE_VERSION - 1;
        for mode in ConversionMode::ALL {
            let mut stored = PipelineSettings::rest(mode, true);
            stored.print.exposure = former_rest(mode) + 0.5;
            let read = caught_up(older, stored.clone());
            assert_eq!(read.print.exposure, 0.5, "{mode:?}");
            assert_eq!(PipelineSettings { print: stored.print, ..read.clone() }, stored, "{mode:?}: nothing else moves");
            assert_eq!(caught_up(older, former(read.clone())), read, "{mode:?}: `former` undone");
            // Twin: a document of this format keeps the value it holds.
            assert_eq!(caught_up(EXPOSURE_DEPARTURE_VERSION, stored.clone()), stored);
        }
        assert_eq!((former_rest(ConversionMode::Negative), former_rest(ConversionMode::Positive)), (1.5, 0.0));
    }
}
