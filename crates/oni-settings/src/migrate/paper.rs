//! A photo a sidecar before format 41 kept was printed on ONI's own print: it stays there, its render unmoved, the
//! paper a switch away (owner's call).

use super::chain::PAPER_VERSION;
use crate::PipelineSettings;

/// The settings of an earlier format on ONI's own print, whatever they name.
pub fn caught_up(version: i64, settings: PipelineSettings) -> PipelineSettings {
    if version >= PAPER_VERSION {
        return settings;
    }
    PipelineSettings { print_paper: None, ..settings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConversionMode, PaperStock};

    #[test]
    fn a_photo_edited_before_the_paper_stays_on_onis_print_and_a_new_one_keeps_its_paper() {
        let older = PAPER_VERSION - 1;
        let digital = PipelineSettings::rest(ConversionMode::Digital, true);
        assert_eq!(digital.print_paper, Some(PaperStock::default()), "a digital photo at rest prints on the paper");
        assert_eq!(caught_up(older, digital.clone()).print_paper, None);
        let negative = PipelineSettings { print_paper: Some(PaperStock::KodakSupraEndura), ..PipelineSettings::rest(ConversionMode::Negative, true) };
        assert_eq!(caught_up(older, negative.clone()), PipelineSettings { print_paper: None, ..negative.clone() }, "nothing else moves");
        // Twin: a file of this format keeps the paper it names.
        assert_eq!(caught_up(PAPER_VERSION, negative).print_paper, Some(PaperStock::KodakSupraEndura));
    }
}
