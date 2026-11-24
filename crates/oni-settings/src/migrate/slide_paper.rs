//! A slide before format 58 printed on Kodak Ektachrome Radiance III alone: one naming a negative paper rendered on ONI's
//! own print; one on the reversal paper, which left, came to name the default paper, kept since a slide is projected.

use super::chain::SLIDE_INTERNEGATIVE_VERSION;
use crate::{PaperStock, PipelineSettings};
use serde_json::Value;

/// `settings` of `version`, read from the stored `raw` settings, on the paper it printed on; run before any step that
/// asks which paper a frame prints on.
pub fn caught_up(version: i64, settings: PipelineSettings, raw: Option<&Value>) -> PipelineSettings {
    if version >= SLIDE_INTERNEGATIVE_VERSION {
        return settings;
    }
    let reversal = raw.and_then(|r| r.get("printPaper")).and_then(Value::as_str) == Some(PaperStock::RETIRED_REVERSAL_KEY);
    let print_paper = match (settings.mode.reads_positive(), reversal) {
        (true, true) => Some(PaperStock::default()),
        (true, false) | (false, true) => None,
        (false, false) => settings.print_paper,
    };
    PipelineSettings { print_paper, ..settings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConversionMode;
    use crate::document::VERSION;
    use serde_json::json;

    fn stored(s: &PipelineSettings, version: i64, key: Option<&str>) -> Vec<u8> {
        let mut stored = super::super::reshape::as_stored(&crate::Document::of(s.clone()), version);
        if let Some(key) = key {
            stored["settings"]["printPaper"] = json!(key);
        }
        serde_json::to_vec(&stored).unwrap()
    }

    fn read(bytes: &[u8]) -> PipelineSettings {
        super::super::document(bytes).unwrap().document.settings
    }

    #[test]
    fn a_slide_on_a_negative_paper_before_stays_on_onis_print_and_one_on_the_reversal_paper_takes_the_default() {
        let older = SLIDE_INTERNEGATIVE_VERSION - 1;
        let slide = PipelineSettings { print_paper: Some(PaperStock::of("kodak-ektacolor-edge")), ..PipelineSettings::rest(ConversionMode::Positive, true) };
        assert_eq!(read(&stored(&slide, older, None)), PipelineSettings { print_paper: None, ..slide.clone() }, "nothing else moves");
        let reversal = read(&stored(&slide, older, Some(PaperStock::RETIRED_REVERSAL_KEY)));
        assert_eq!(reversal.print_paper, Some(PaperStock::default()), "the default paper");
        // A negative keeps its paper; one naming the reversal paper printed on ONI's print and stays there.
        let negative = PipelineSettings { mode: ConversionMode::Negative, ..slide.clone() };
        assert_eq!(read(&stored(&negative, older, None)).print_paper, Some(PaperStock::of("kodak-ektacolor-edge")));
        assert_eq!(read(&stored(&negative, older, Some(PaperStock::RETIRED_REVERSAL_KEY))).print_paper, None);
        // Twin: a slide of this format keeps the negative paper it names.
        assert_eq!(read(&stored(&slide, VERSION, None)).print_paper, Some(PaperStock::of("kodak-ektacolor-edge")));
    }

    #[test]
    fn a_slide_on_a_paper_is_written_past_every_build_that_rendered_it_on_onis_print() {
        let s = PipelineSettings { print_paper: Some(PaperStock::default()), ..PipelineSettings::rest(ConversionMode::Positive, true) };
        let bytes = stored(&s, VERSION, None);
        assert!(VERSION >= SLIDE_INTERNEGATIVE_VERSION && read(&bytes) == s);
        // Twin: a reader refuses a version past its own as newer, as a build before the internegative refuses this one.
        let mut newer: Value = serde_json::from_slice(&bytes).unwrap();
        newer["version"] = json!(VERSION + 1);
        assert_eq!(super::super::document(&serde_json::to_vec(&newer).unwrap()).err(), Some(super::super::Refusal::Newer(VERSION + 1)));
    }
}
