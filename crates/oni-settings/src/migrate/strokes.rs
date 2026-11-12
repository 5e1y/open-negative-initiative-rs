//! Dust strokes a sidecar before format 21 kept on the cropped, oriented frame, moved once onto the
//! scan they now live on, through the geometry they were read with.

use super::Refusal;
use super::chain::STROKES_ON_SCAN_VERSION;
use crate::PipelineSettings;
use crate::geometry::Size;

/// The settings with their strokes on the scan; with no scan size they are refused. The size is asked
/// last, as asking reads the file.
pub fn caught_up(version: i64, mut settings: PipelineSettings, scan: &dyn Fn(&PipelineSettings) -> Result<Size, Refusal>) -> Result<PipelineSettings, Refusal> {
    let painted = settings.corrections.iter().any(|c| !c.stroke.is_empty());
    if version >= STROKES_ON_SCAN_VERSION || !painted || settings.geometry.is_neutral() {
        return Ok(settings);
    }
    let image = scan(&settings)?;
    let placing = settings.geometry.placing(image);
    for c in &mut settings.corrections {
        for p in &mut c.stroke {
            *p = placing.on_scan(*p);
        }
    }
    Ok(settings)
}
