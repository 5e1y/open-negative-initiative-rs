//! Before format 82 a digital photo's film effects and its shooting exposure took any value; they take named levels now
//! (owner's call), so a stored value off them lands on the nearest.

use super::chain::EFFECT_LEVELS_VERSION;
use crate::PipelineSettings;

/// `settings` of `version` with its film's effects and shooting exposure on their nearest levels.
pub fn caught_up(version: i64, settings: PipelineSettings) -> PipelineSettings {
    match version < EFFECT_LEVELS_VERSION {
        true => crate::effect_levels::on_levels(&settings),
        false => settings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::virtual_film::VirtualStock;
    use crate::{ConversionMode, effect_levels};

    fn stored(s: &PipelineSettings, version: i64) -> Vec<u8> {
        serde_json::to_vec(&super::super::reshape::as_stored(&crate::Document::of(s.clone()), version)).unwrap()
    }

    fn read(bytes: &[u8]) -> PipelineSettings {
        super::super::document(bytes).unwrap().document.settings
    }

    #[test]
    fn a_stored_photos_effects_land_on_their_nearest_levels_once() {
        let mut s = PipelineSettings::rest(ConversionMode::Digital, true).picking(VirtualStock::of("kodak-portra-800"));
        (s.grain.amount, s.grain.size, s.halation.amount, s.halation.size, s.bloom.size) = (0.3, 28.0, 0.5, 120.0, 4.5);
        s.virtual_film.as_mut().unwrap().exposure = -0.9;
        let landed = read(&stored(&s, EFFECT_LEVELS_VERSION - 1));
        let sheet = VirtualStock::of("kodak-portra-800").effects().bloom.size;
        assert_eq!((landed.grain.amount, landed.grain.size, landed.halation.amount, landed.halation.size), (0.2545, 30.0, 0.31, 100.0));
        assert_eq!(landed.bloom.size, effect_levels::snapped(&effect_levels::bloom(VirtualStock::of("kodak-portra-800")), 4.5));
        assert!((landed.bloom.size - 1.5 * sheet).abs() < 1e-6, "{} on a sheet of {sheet}", landed.bloom.size);
        assert_eq!(landed.film().unwrap().exposure, -1.0);
        // Twins: a photo of this format keeps its values; a scanned negative's grain, none of a virtual film's, stays.
        assert_eq!(caught_up(EFFECT_LEVELS_VERSION, s.clone()), s);
        let mut scan = PipelineSettings::rest(ConversionMode::Negative, true);
        scan.grain.amount = 0.3;
        assert_eq!(caught_up(EFFECT_LEVELS_VERSION - 1, scan.clone()), scan);
    }
}
