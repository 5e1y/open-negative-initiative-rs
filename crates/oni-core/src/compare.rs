//! `NegativeView.referenceSettings`: Compare's before, the bar's reset frame with its dust still healed.

use crate::parameters::imported;
use oni_settings::PipelineSettings;

pub fn reference(live: &PipelineSettings) -> PipelineSettings {
    PipelineSettings { corrections: live.corrections.clone(), ..imported(live) }
}
