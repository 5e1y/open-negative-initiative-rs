//! Compare's before: the bar's reset frame with its dust bare, so the after shows the dust each patch healed.
//! On either face it is the Pro face's plain conversion, so the Scanner's table shows as part of the after.

use crate::parameters::imported;
use oni_settings::{Face, PipelineSettings};

pub fn reference(live: &PipelineSettings) -> PipelineSettings {
    let pro = PipelineSettings { face: Face::Pro, ..live.clone() };
    PipelineSettings { corrections: Vec::new(), ..imported(&pro) }
}
