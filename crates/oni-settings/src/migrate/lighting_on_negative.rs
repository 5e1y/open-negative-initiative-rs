//! A photo on a laid legacy print before format 59 kept its machine's entry in Lighting's Exposure, which ONI's print
//! curve read: the legacy print came to read it itself, so its Exposure moves to Lighting's rest by that entry.

use super::chain::LIGHTING_ON_NEGATIVE_VERSION;
use super::departure::former_rest;
use super::one_face::look_entry_exposure;
use crate::PipelineSettings;
use crate::tone::Print;

/// `settings` of `version` with Exposure in this format's units, every step before this one done.
pub fn caught_up(version: i64, settings: PipelineSettings) -> PipelineSettings {
    if version >= LIGHTING_ON_NEGATIVE_VERSION || settings.legacy().is_none() || super::chain::papered(&settings) {
        return settings;
    }
    let exposure = settings.print.exposure - look_entry_exposure(settings.mode) + former_rest(settings.mode);
    PipelineSettings { print: Print { exposure, ..settings.print }, ..settings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConversionMode, LegacyMachine, LegacyPrint, PaperStock};

    fn on_legacy(mode: ConversionMode) -> PipelineSettings {
        let mut s = PipelineSettings { print_paper: None, ..PipelineSettings::rest(mode, true) };
        s.print.exposure = look_entry_exposure(mode);
        s.legacy_print = Some(LegacyPrint { machine: LegacyMachine::Frontier, ..LegacyPrint::default() });
        s
    }

    #[test]
    fn a_legacy_prints_entry_leaves_lightings_exposure_and_a_hand_s_move_stays() {
        let older = LIGHTING_ON_NEGATIVE_VERSION - 1;
        for mode in ConversionMode::ALL {
            let rest = former_rest(mode);
            assert_eq!(caught_up(older, on_legacy(mode)).print.exposure, rest, "{mode:?}: the machine's entry rests");
            let mut lit = on_legacy(mode);
            lit.print.exposure += 0.4;
            assert!((caught_up(older, lit).print.exposure - (rest + 0.4)).abs() < 1e-6, "{mode:?}");
            // Twins: a document of this format, a photo on no legacy print or on a paper keeps its Exposure.
            assert_eq!(caught_up(LIGHTING_ON_NEGATIVE_VERSION, on_legacy(mode)), on_legacy(mode));
            let plain = PipelineSettings { legacy_print: None, ..on_legacy(mode) };
            assert_eq!(caught_up(older, plain.clone()), plain);
            let papered = PipelineSettings { print_paper: Some(PaperStock::of("kodak-endura-premier")), ..on_legacy(mode) };
            assert_eq!(caught_up(older, papered.clone()), papered);
        }
    }
}
