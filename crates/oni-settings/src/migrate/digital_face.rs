//! A digital photo a sidecar before format 30 kept on the Scanner face, where it rendered as Pro: it
//! opens on Pro, so its render does not move now that a digital photo prints on the machine.

use super::chain::DIGITAL_SCANNER_VERSION;
use crate::PipelineSettings;
use crate::modes::Face;

/// The settings on the face they rendered on when stored.
pub fn caught_up(version: i64, mut settings: PipelineSettings) -> PipelineSettings {
    if version < DIGITAL_SCANNER_VERSION && settings.mode.is_digital() {
        settings.face = Face::Pro;
    }
    settings
}
