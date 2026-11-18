//! The grain's arguments: a texture in stops laid on the scene before the print, anchored on the
//! scan's full-resolution grid so a pan, a zoom, a crop or a turn leaves it on the photo.

use crate::geometry;
use oni_graph::Rect;
use oni_render::Grain;
use oni_settings::PipelineSettings;

/// The spread at full resolution of Amount's top, in stops.
pub const GRAIN_STOPS: f32 = 0.5;
/// The finest cell, a share of the scan's full width, the one length every reduction keeps exactly:
/// a pixel on a scan 6000 pixels wide.
pub const GRAIN_FINEST: f64 = 1.0 / 6000.0;
/// How many times Size's top coarsens the finest cell.
pub const GRAIN_COARSEST: f64 = 4.0;

/// The grain of `s` on a scan of extent `scan` at `scale`; `None` at rest, so the chain holds no
/// grain stage.
pub fn grain(s: &PipelineSettings, scan: Rect, scale: f64) -> Option<Grain> {
    if s.grain.amount == 0.0 {
        return None;
    }
    let cell = f64::from(scan.w) / scale * GRAIN_FINEST * GRAIN_COARSEST.powf(f64::from(s.grain.size));
    let m = geometry::scan_from(&s.geometry, scan, scale).map(|r| r.map(|v| v as f32));
    Some(Grain::on(m, cell as f32, scale as f32, GRAIN_STOPS * s.grain.amount))
}
