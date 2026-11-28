//! A photo stored at format 85 kept its colour compression as a strength of its paper's crosstalk, 1 the paper as baked:
//! from format 86 it is the share of chroma that paper takes, absent its own dose, so the same strength prints.

use super::chain::{ABSOLUTE_COMPRESSION_VERSION, COLOUR_COMPRESSION_VERSION};
use crate::PipelineSettings;
use crate::tolerant;
use serde_json::Value;

/// `settings` of `version` with its colour compression as its paper's share, read off the stored strength.
pub fn caught_up(version: i64, mut settings: PipelineSettings, stored: Option<&Value>) -> PipelineSettings {
    if !(COLOUR_COMPRESSION_VERSION..ABSOLUTE_COMPRESSION_VERSION).contains(&version) {
        return settings;
    }
    let strength = tolerant::ns_number(stored.and_then(|s| s.get("print")).and_then(|p| p.get("colourCompression"))).filter(|v| v.is_finite());
    let scale = settings.print_paper.and_then(|p| p.compression());
    settings.print.colour_compression = match (strength, scale) {
        (Some(k), Some(scale)) if k != 1.0 => Some(scale.removed_at(k as f32)),
        _ => None,
    };
    settings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConversionMode, PaperStock};
    use serde_json::json;

    #[test]
    fn a_strength_of_format_85_becomes_its_papers_share_and_its_rest_its_own_dose() {
        let mut s = PipelineSettings { print_paper: Some(PaperStock::of("kodak-endura-premier")), ..PipelineSettings::rest(ConversionMode::Negative, true) };
        s.print.colour_compression = Some(1.0);
        let scale = PaperStock::of("kodak-endura-premier").compression().unwrap();
        let at = |k: f64| caught_up(COLOUR_COMPRESSION_VERSION, s.clone(), Some(&json!({"print": {"colourCompression": k}}))).print.colour_compression;
        assert_eq!(at(0.5), Some(scale.removed_at(0.5)));
        assert!((scale.strength(at(0.5).unwrap()) - 0.5).abs() < 1e-5, "the same strength prints");
        assert_eq!((at(1.0), caught_up(COLOUR_COMPRESSION_VERSION, s.clone(), Some(&json!({}))).print.colour_compression), (None, None));
        // Twin: a document of format 86 keeps its share as stored.
        assert_eq!(caught_up(ABSOLUTE_COMPRESSION_VERSION, s.clone(), Some(&json!({"print": {"colourCompression": 0.5}}))), s);
    }
}
