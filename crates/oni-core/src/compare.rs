//! Compare's before: the bar's reset frame with its dust bare, so the after shows the dust each patch healed, and a
//! legacy print's look as part of the after.

use crate::parameters::imported;
use oni_settings::PipelineSettings;

pub fn reference(live: &PipelineSettings) -> PipelineSettings {
    PipelineSettings { corrections: Vec::new(), ..imported(live) }
}
