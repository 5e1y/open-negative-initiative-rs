//! The grain's arguments: a texture in stops laid on the scene before the print, anchored on the
//! scan's full-resolution grid so a pan, a zoom, a crop or a turn leaves it on the photo.

use crate::{conversion, geometry};
use oni_graph::Rect;
use oni_render::{GRAIN_NODES, Grain, LUMA, Stage47, cpu, grain_node_light};
use oni_settings::PipelineSettings;
use oni_settings::blocks::GrainCurve;
use oni_settings::tone::Print;

/// The spread at full resolution of Amount's top, in stops.
pub const GRAIN_STOPS: f32 = 0.5;
/// The finest cell, a share of the scan's full width, the one length every reduction keeps exactly:
/// a pixel on a scan 6000 pixels wide.
pub const GRAIN_FINEST: f64 = 1.0 / 6000.0;
/// How many times Size's top coarsens the finest cell.
pub const GRAIN_COARSEST: f64 = 4.0;

/// The grain of `s` on a scan of extent `scan` at `scale`; `None` at rest or with no share anywhere, so
/// the chain holds no grain stage.
pub fn grain(s: &PipelineSettings, scan: Rect, scale: f64) -> Option<Grain> {
    if s.grain.amount == 0.0 {
        return None;
    }
    // A Scanner face's keys are its print, in the picture's gains: they leave the tones, so the grain's pass.
    let share = shares(&s.grain.curve, &Stage47 { gain: [1.0; 3], ..conversion::stage47(s) });
    if share.iter().all(|&v| v == 0.0) {
        return None;
    }
    let cell = f64::from(scan.w) / scale * GRAIN_FINEST * GRAIN_COARSEST.powf(f64::from(s.grain.size));
    let m = geometry::scan_from(&s.geometry, scan, scale).map(|r| r.map(|v| v as f32));
    Some(Grain::on(m, cell as f32, scale as f32, GRAIN_STOPS * s.grain.amount, share))
}

/// The tone each node's light shows as the picture opens, before Lighting: its luminance through the
/// picture's own pass, encoded as the curves read it. No print slider reaches it, so a print notch keeps
/// the grain's pass.
pub fn tones(picture: &Stage47) -> [f32; GRAIN_NODES] {
    std::array::from_fn(|i| {
        let y = grain_node_light(i);
        let shown = cpu::stage47([y, y, y, 1.0], picture);
        let luma = shown[0] * LUMA[0] + shown[1] * LUMA[1] + shown[2] * LUMA[2];
        luma.max(0.0).powf(1.0 / Print::DISPLAY_GAMMA)
    })
}

/// The curve's share at each node's tone, held to 0…1.
pub fn shares(curve: &GrainCurve, picture: &Stage47) -> [f32; GRAIN_NODES] {
    let strength = curve.strength();
    tones(picture).map(|t| strength.at(f64::from(t)) as f32)
}
