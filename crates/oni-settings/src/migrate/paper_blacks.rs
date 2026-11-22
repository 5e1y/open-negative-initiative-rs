//! A paper's print before format 48 laid the frame's pick where the paper's rest prints it, unless True blacks laid it on
//! pure black: a photo then on a paper keeps that black as Paper blacks; any other takes the rest, pure black.

use super::chain::PAPER_BLACKS_VERSION;
use crate::PipelineSettings;
use crate::document::Failure;
use crate::tone::Print;
use serde::Deserialize;
use serde_json::Value;

/// The retired True blacks switch a document carries, off where it names none.
#[derive(Debug, Default, Deserialize)]
pub struct Former {
    print: Option<FormerPrint>,
}

#[derive(Debug, Default, Deserialize)]
struct FormerPrint {
    #[serde(rename = "trueBlacks")]
    true_blacks: Option<bool>,
}

impl Former {
    /// The switch `settings` carries; a value of the wrong shape refuses the document, as its typed read did.
    pub fn read(settings: Option<&Value>) -> Result<Former, Failure> {
        match settings {
            Some(v @ Value::Object(_)) => serde_json::from_value(v.clone()).map_err(|e| Failure::Malformed(e.to_string())),
            _ => Ok(Former::default()),
        }
    }

    fn true_blacks(&self) -> bool {
        self.print.as_ref().and_then(|p| p.true_blacks).unwrap_or(false)
    }
}

/// `settings` of `version` with the black its paper printed, every step before this one done.
pub fn caught_up(version: i64, settings: PipelineSettings, former: &Former) -> PipelineSettings {
    if version >= PAPER_BLACKS_VERSION {
        return settings;
    }
    let paper_blacks = settings.paper().is_some() && !former.true_blacks();
    PipelineSettings { print: Print { paper_blacks, ..settings.print }, ..settings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConversionMode, PaperStock};
    use serde_json::json;

    fn on_paper() -> PipelineSettings {
        PipelineSettings { print_paper: Some(PaperStock::default()), ..PipelineSettings::rest(ConversionMode::Negative, true) }
    }

    fn former(stored: Value) -> Former {
        Former::read(Some(&stored)).unwrap()
    }

    #[test]
    fn a_photo_on_a_paper_keeps_the_black_it_printed_and_any_other_takes_pure_black() {
        let older = PAPER_BLACKS_VERSION - 1;
        assert!(caught_up(older, on_paper(), &former(json!({ "print": {} }))).print.paper_blacks, "the paper's own black kept");
        assert!(!caught_up(older, on_paper(), &former(json!({ "print": { "trueBlacks": true } }))).print.paper_blacks, "True blacks kept");
        // Twin: a photo on ONI's print, or a slide whose paper never printed, takes the rest.
        let onis = PipelineSettings { print_paper: None, ..on_paper() };
        assert!(!caught_up(older, onis, &former(json!({}))).print.paper_blacks);
        let slide = super::super::slide_paper::caught_up(older, PipelineSettings { mode: ConversionMode::Positive, ..on_paper() }, None);
        assert!(!caught_up(older, slide, &former(json!({}))).print.paper_blacks);
        // Twin: a document of this format keeps what it says.
        assert!(!caught_up(PAPER_BLACKS_VERSION, on_paper(), &former(json!({}))).print.paper_blacks);
        let rest = on_paper();
        assert_eq!(caught_up(older, rest.clone(), &former(json!({ "print": { "trueBlacks": true } }))), rest, "nothing else moves");
    }

    #[test]
    fn a_switch_of_the_wrong_shape_refuses_the_document() {
        assert!(Former::read(Some(&json!({ "print": { "trueBlacks": 1 } }))).is_err());
        assert!(!Former::read(Some(&json!({ "print": { "trueBlacks": null } }))).unwrap().true_blacks());
        assert!(!Former::read(None).unwrap().true_blacks());
    }
}
