//! Dust strokes a sidecar before format 21 kept on the cropped, oriented frame, moved once onto the
//! scan they now live on, through the geometry they were read with.

use crate::codec::STROKES_ON_SCAN_VERSION;
use crate::geometry::Size;
use crate::PipelineSettings;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// The whole decode's raw size for an original read with these settings; `None` when unknown.
pub type Lookup = dyn Fn(&Path, &PipelineSettings) -> Option<Size> + Send + Sync;

static LOOKUP: RwLock<Option<Arc<Lookup>>> = RwLock::new(None);

/// Sets what every later sidecar read asks; `None` asks nothing.
pub fn install(lookup: Option<Arc<Lookup>>) {
    if let Ok(mut l) = LOOKUP.write() {
        *l = lookup;
    }
}

/// The settings with their strokes on the scan, and whether they may be written: strokes whose scan
/// size is unknown stay as read, never written under the new format.
pub(crate) fn caught_up(original: &Path, version: i64, settings: PipelineSettings) -> (PipelineSettings, bool) {
    caught_up_by(original, version, settings, |p, s| LOOKUP.read().ok().and_then(|l| l.clone()).and_then(|f| f(p, s)))
}

/// The size is asked last, as asking reads the file.
pub fn caught_up_by(original: &Path, version: i64, mut settings: PipelineSettings, size: impl FnOnce(&Path, &PipelineSettings) -> Option<Size>) -> (PipelineSettings, bool) {
    let painted = settings.corrections.iter().any(|c| !c.stroke.is_empty());
    if version >= STROKES_ON_SCAN_VERSION || !painted || settings.geometry.is_neutral() {
        return (settings, true);
    }
    let Some(image) = size(original, &settings) else { return (settings, false) };
    let placing = settings.geometry.placing(image);
    for c in &mut settings.corrections {
        for p in &mut c.stroke {
            *p = placing.on_scan(*p);
        }
    }
    (settings, true)
}
