//! The scan a former sidecar's dust strokes move onto, sized from each original's metadata: a lead's
//! three passes are laid on their green one, so its size is theirs.

use oni_settings::PipelineSettings;
use std::path::Path;
use std::sync::Arc;

/// Every later sidecar read sizes an original through `oni_decode::full_size`, no pixel decoded.
pub fn install() {
    oni_settings::strokes_on_scan::install(Some(Arc::new(|path: &Path, s: &PipelineSettings| {
        let green = s.rgb_passes.as_ref().map(|p| path.parent().map_or_else(|| Path::new(&p.green).to_path_buf(), |f| f.join(&p.green)));
        oni_decode::full_size(green.as_deref().unwrap_or(path)).map(|(w, h)| (w as f64, h as f64))
    })));
}
