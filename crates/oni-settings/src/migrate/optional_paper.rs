//! Before format 73 a frame naming no paper printed on its format's default, and at 72 every print was made on one. A
//! paper is a choice laid over the pure inversion now (owner's call): a frame on the default paper of its format, Kodak
//! Royal Digital before 72 and ONI Standard at 72, named or not, or printed with Print on paper off, names none; any
//! other paper stays, and a frame holding a legacy print keeps the paper it names.

use super::chain::{EVERY_PAPER_VERSION, OPTIONAL_PAPER_VERSION};
use crate::{PaperStock, PipelineSettings};
use serde_json::Value;

/// The default paper before format 72, and the one of format 72, which also stood for Print on paper off.
const ROYAL_DIGITAL: PaperStock = PaperStock::of("kodak-royal-digital");
const ONI_STANDARD: PaperStock = PaperStock::of("oni-standard");

/// `settings` of `version`, read from the stored `raw` settings, on the paper its person chose, none where it stood on
/// its format's default.
pub fn caught_up(version: i64, settings: PipelineSettings, raw: Option<&Value>) -> PipelineSettings {
    if version >= OPTIONAL_PAPER_VERSION {
        return settings;
    }
    let before_every_paper = version < EVERY_PAPER_VERSION;
    let neutral = before_every_paper && settings.print_paper.is_some() && raw.and_then(|r| r.get("paperless")).and_then(Value::as_bool) == Some(true);
    if settings.legacy_print.is_some() {
        // A legacy print laid on a paper with Print on paper off keeps the neutral paper that stood for it.
        return if neutral { PipelineSettings { print_paper: Some(ONI_STANDARD), ..settings } } else { settings };
    }
    let default = if before_every_paper { ROYAL_DIGITAL } else { ONI_STANDARD };
    if neutral || settings.print_paper == Some(default) { PipelineSettings { print_paper: None, ..settings } } else { settings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConversionMode, LegacyPrint};
    use serde_json::json;

    const EDGE: PaperStock = PaperStock::of("kodak-ektacolor-edge");
    /// Format 72, where ONI Standard was the default, the format before it, on Royal Digital, and the first of this step.
    const ON_STANDARD: i64 = EVERY_PAPER_VERSION;
    const ON_ROYAL: i64 = EVERY_PAPER_VERSION - 1;
    const OPTIONAL: i64 = OPTIONAL_PAPER_VERSION;

    fn stored(version: i64, settings: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(&json!({ "format": crate::document::FORMAT, "version": version, "settings": settings })).unwrap()
    }

    fn read(bytes: &[u8]) -> PipelineSettings {
        super::super::document(bytes).unwrap().document.settings
    }

    fn on(paper: Option<PaperStock>) -> PipelineSettings {
        PipelineSettings { print_paper: paper, ..PipelineSettings::rest(ConversionMode::Negative, true) }
    }

    fn paper_read(version: i64, settings: &PipelineSettings) -> Option<PaperStock> {
        read(&stored(version, serde_json::to_value(settings).unwrap())).print_paper
    }

    #[test]
    fn a_frame_on_its_formats_default_paper_names_none_and_a_chosen_paper_stays() {
        // ONI Standard, format 72's default, and Royal Digital, the one before it.
        assert_eq!(paper_read(ON_STANDARD, &on(Some(ONI_STANDARD))), None);
        assert_eq!(paper_read(ON_ROYAL, &on(Some(ROYAL_DIGITAL))), None);
        assert_eq!((paper_read(ON_STANDARD, &on(None)), paper_read(ON_ROYAL, &on(None))), (None, None));
        let migrated = read(&stored(ON_STANDARD, serde_json::to_value(on(Some(ONI_STANDARD))).unwrap()));
        assert!(migrated.shows_inversion() && migrated.paper().is_none());
        // Twins: a paper chosen stays, Royal Digital named once ONI Standard was the default included, and this format
        // keeps ONI Standard where its person names it.
        assert_eq!((paper_read(ON_STANDARD, &on(Some(EDGE))), paper_read(ON_ROYAL, &on(Some(EDGE)))), (Some(EDGE), Some(EDGE)));
        assert_eq!(paper_read(ON_STANDARD, &on(Some(ROYAL_DIGITAL))), Some(ROYAL_DIGITAL));
        assert_eq!(paper_read(OPTIONAL, &on(Some(ONI_STANDARD))), Some(ONI_STANDARD));
    }

    #[test]
    fn print_on_paper_off_names_no_paper_and_its_paper_stays_where_the_switch_was_on() {
        let mut text = serde_json::to_value(on(Some(EDGE))).unwrap();
        text["paperless"] = json!(true);
        assert_eq!(read(&stored(ON_ROYAL, text.clone())).print_paper, None);
        // Twins: the paper kept where the switch was on, and where a later format only carries the retired key.
        text["paperless"] = json!(false);
        assert_eq!(read(&stored(ON_ROYAL, text.clone())).print_paper, Some(EDGE));
        text["paperless"] = json!(true);
        assert_eq!(read(&stored(ON_STANDARD, text.clone())).print_paper, Some(EDGE));
        assert_eq!(read(&stored(OPTIONAL, text)).print_paper, Some(EDGE));
    }

    #[test]
    fn a_frame_holding_a_legacy_print_keeps_the_paper_it_names() {
        let legacy = |paper, laid| PipelineSettings { legacy_print: Some(LegacyPrint { laid, ..LegacyPrint::default() }), ..on(paper) };
        for laid in [true, false] {
            assert_eq!(paper_read(ON_ROYAL, &legacy(Some(ROYAL_DIGITAL), laid)), Some(ROYAL_DIGITAL), "laid {laid}");
            assert_eq!(paper_read(ON_STANDARD, &legacy(Some(ONI_STANDARD), laid)), Some(ONI_STANDARD), "laid {laid}");
            assert_eq!(paper_read(ON_ROYAL, &legacy(None, laid)), None, "laid {laid}");
        }
        // Laid on a paper with Print on paper off, it keeps the neutral paper; on its own curve, that curve.
        let mut neutral = serde_json::to_value(legacy(Some(EDGE), true)).unwrap();
        neutral["paperless"] = json!(true);
        assert_eq!(read(&stored(ON_ROYAL, neutral)).print_paper, Some(ONI_STANDARD));
        let mut own = serde_json::to_value(legacy(None, true)).unwrap();
        own["paperless"] = json!(true);
        let own = read(&stored(ON_ROYAL, own));
        assert!(own.print_paper.is_none() && own.legacy().is_some() && !own.shows_inversion());
        // Twin: the same frames with no legacy print leave their default paper.
        assert_eq!(paper_read(ON_ROYAL, &on(Some(ROYAL_DIGITAL))), None);
    }
}
