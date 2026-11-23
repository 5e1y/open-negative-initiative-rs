//! A paper's print before format 53 laid the frame's pick where the paper's rest prints it, unless True blacks laid it on
//! pure black: a photo then on a paper keeps that black as Print lift; any other takes the rest, pure black and pure white.
//! Before format 67 the switch was stored as Paper blacks, the white always the paper's: on, it is Print lift; off, the
//! paper's white comes to lie on pure white.

use super::chain::{PAPER_BLACKS_VERSION, PRINT_LIFT_VERSION};
use crate::PipelineSettings;
use crate::document::Failure;
use crate::tone::Print;
use serde::Deserialize;
use serde_json::Value;

/// The retired True blacks and Paper blacks switches a document carries, off where it names none.
#[derive(Debug, Default, Deserialize)]
pub struct Former {
    print: Option<FormerPrint>,
}

#[derive(Debug, Default, Deserialize)]
struct FormerPrint {
    #[serde(rename = "trueBlacks")]
    true_blacks: Option<bool>,
    #[serde(rename = "paperBlacks")]
    paper_blacks: Option<bool>,
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

    fn paper_blacks(&self) -> bool {
        self.print.as_ref().and_then(|p| p.paper_blacks).unwrap_or(false)
    }
}

/// `settings` of `version` with the ends its paper printed, every step before this one done.
pub fn caught_up(version: i64, settings: PipelineSettings, former: &Former) -> PipelineSettings {
    let print_lift = match version {
        v if v >= PRINT_LIFT_VERSION => return settings,
        v if v >= PAPER_BLACKS_VERSION => former.paper_blacks(),
        _ => super::chain::papered(&settings) && !former.true_blacks(),
    };
    PipelineSettings { print: Print { print_lift, ..settings.print }, ..settings }
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
        assert!(caught_up(older, on_paper(), &former(json!({ "print": {} }))).print.print_lift, "the paper's own black kept");
        assert!(!caught_up(older, on_paper(), &former(json!({ "print": { "trueBlacks": true } }))).print.print_lift, "True blacks kept");
        // Twin: a photo on ONI's print, or a slide whose paper never printed, takes the rest.
        let onis = PipelineSettings { print_paper: None, ..on_paper() };
        assert!(!caught_up(older, onis, &former(json!({}))).print.print_lift);
        let slide = super::super::slide_paper::caught_up(older, PipelineSettings { mode: ConversionMode::Positive, ..on_paper() }, None);
        assert!(!caught_up(older, slide, &former(json!({}))).print.print_lift);
        // Twin: a document of this format keeps what it says.
        assert!(!caught_up(PRINT_LIFT_VERSION, on_paper(), &former(json!({ "print": { "paperBlacks": true } }))).print.print_lift);
        let rest = on_paper();
        assert_eq!(caught_up(older, rest.clone(), &former(json!({ "print": { "trueBlacks": true } }))), rest, "nothing else moves");
    }

    #[test]
    fn paper_blacks_comes_to_print_lift_on_where_it_was_on() {
        for version in [PAPER_BLACKS_VERSION, PRINT_LIFT_VERSION - 1] {
            assert!(caught_up(version, on_paper(), &former(json!({ "print": { "paperBlacks": true } }))).print.print_lift, "{version}");
            // Twin: off, or absent, the paper's white comes to lie on pure white with its black.
            assert!(!caught_up(version, on_paper(), &former(json!({ "print": { "paperBlacks": false } }))).print.print_lift);
            assert!(!caught_up(version, on_paper(), &former(json!({ "print": {} }))).print.print_lift);
        }
        // Twin: a document of this format reads the switch at its own key, the former one ignored.
        let typed = PipelineSettings { print: Print { print_lift: true, ..on_paper().print }, ..on_paper() };
        assert!(caught_up(PRINT_LIFT_VERSION, typed, &former(json!({ "print": { "paperBlacks": false } }))).print.print_lift);
    }

    #[test]
    fn a_switch_of_the_wrong_shape_refuses_the_document() {
        assert!(Former::read(Some(&json!({ "print": { "paperBlacks": "on" } }))).is_err());
        assert!(Former::read(Some(&json!({ "print": { "trueBlacks": 1 } }))).is_err());
        assert!(!Former::read(Some(&json!({ "print": { "trueBlacks": null } }))).unwrap().true_blacks());
        assert!(!Former::read(None).unwrap().true_blacks());
    }
}
