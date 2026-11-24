//! Before format 72 a print could be made without a paper: ONI's own print where a frame named none, or a neutral paper
//! with Print on paper off. Every print is made on a paper now (owner's call): a frame naming none prints on the default,
//! ONI Standard, and one printed with Print on paper off names ONI Standard, the neutral paper; a legacy print laid on
//! no paper keeps its own.

use super::chain::EVERY_PAPER_VERSION;
use crate::{PaperStock, PipelineSettings};
use serde_json::Value;

/// `settings` of `version`, read from the stored `raw` settings, on the paper its print stands for.
pub fn caught_up(version: i64, settings: PipelineSettings, raw: Option<&Value>) -> PipelineSettings {
    let paperless = raw.and_then(|r| r.get("paperless")).and_then(Value::as_bool) == Some(true);
    if version >= EVERY_PAPER_VERSION || !paperless {
        return settings;
    }
    PipelineSettings { print_paper: Some(PaperStock::default()), ..settings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConversionMode;
    use crate::document::VERSION;
    use serde_json::json;

    fn stored(version: i64, settings: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(&json!({ "format": crate::document::FORMAT, "version": version, "settings": settings })).unwrap()
    }

    fn read(bytes: &[u8]) -> PipelineSettings {
        super::super::document(bytes).unwrap().document.settings
    }

    #[test]
    fn print_on_paper_off_lands_on_oni_standard_and_a_paper_print_keeps_its_paper() {
        let on_edge = PipelineSettings { print_paper: Some(PaperStock::of("kodak-ektacolor-edge")), ..PipelineSettings::rest(ConversionMode::Negative, true) };
        let mut text = serde_json::to_value(&on_edge).unwrap();
        text["paperless"] = json!(true);
        let paperless = read(&stored(VERSION - 1, text.clone()));
        assert_eq!(paperless.print_paper, Some(PaperStock::of("oni-standard")));
        assert_eq!(paperless.paper(), Some(PaperStock::default()));
        // Twins: the paper kept where the switch was on, and a newer document naming the switch read as it stands.
        text["paperless"] = json!(false);
        assert_eq!(read(&stored(VERSION - 1, text.clone())).print_paper, Some(PaperStock::of("kodak-ektacolor-edge")));
        text["paperless"] = json!(true);
        assert_eq!(read(&stored(VERSION, text)).print_paper, Some(PaperStock::of("kodak-ektacolor-edge")));
    }

    #[test]
    fn a_frame_on_onis_print_prints_on_the_default_paper_and_a_legacy_print_keeps_its_own() {
        let rest = PipelineSettings::rest(ConversionMode::Negative, true);
        let read_back = read(&stored(VERSION - 1, serde_json::to_value(&rest).unwrap()));
        assert_eq!((read_back.print_paper, read_back.paper()), (None, Some(PaperStock::default())));
        // Twin: a legacy print laid on no paper keeps its own print.
        let legacy = PipelineSettings { legacy_print: Some(crate::LegacyPrint::default()), ..rest };
        assert!(legacy.legacy().is_some() && legacy.paper().is_none());
        assert!(PipelineSettings { legacy_print: None, ..legacy }.paper().is_some());
    }
}
