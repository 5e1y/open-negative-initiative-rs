//! What a film's layers take between their exposure and their development: the emulsion's own blur, then the halation
//! its base sends back.

use crate::{halation, stages};
use oni_graph::{Image, Rect};
use oni_settings::PipelineSettings;
use oni_settings::axis::pipeline::BLUR_FLOOR;

/// The deviation of the film's own blur in this render's pixels, on a scan of extent `scan`.
pub fn bloom(s: &PipelineSettings, scan: Rect) -> f64 {
    s.bloom.pixels(f64::from(scan.w.max(scan.h)))
}

/// `exposed`, a film's layers' exposures or the scene a film of no layer reads, with the effects of `s` on a scan of
/// extent `scan`; none where both rest, so the film stays one pass.
pub fn laid(exposed: Image, s: &PipelineSettings, scan: Rect) -> Option<Image> {
    let sigma = bloom(s, scan);
    let soft = sigma > BLUR_FLOOR;
    if !soft && !halation::lays(s) {
        return None;
    }
    let spread = if soft { stages::blurred(exposed, sigma, false) } else { exposed };
    Some(halation::halated(spread, s, scan))
}
