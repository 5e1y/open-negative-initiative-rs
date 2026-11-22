//! The grain's arguments: a texture in stops laid on the scene before the print, anchored on the
//! scan's full-resolution grid so a pan, a zoom, a crop or a turn leaves it on the photo.

use crate::{conversion, film, geometry, stages};
use oni_graph::{Image, Rect};
use oni_settings::axis::pipeline::BLUR_FLOOR;
use oni_render::{GRAIN_NODES, Grain, LUMA, Stage47, cpu, grain_node_light};
use oni_settings::PipelineSettings;
use oni_settings::blocks::{GrainColour, GrainCurve};
use oni_settings::tone::{Print, color_density};

/// The spread at full resolution of Amount's top, in stops.
pub const GRAIN_STOPS: f32 = 0.5;
/// The length a clump's blur spares, a share of the scan's full width, the one length every reduction keeps
/// exactly: a pixel on a scan 6000 pixels wide.
pub const GRAIN_FINEST: f64 = 1.0 / 6000.0;
/// The blur a clump takes in, as a share of how much its cell outgrows the finest: none at a grain that fine,
/// so a fine grain blurs nothing, and half a cell's side less a pixel at a coarse one.
pub const GRAIN_DISSOLVE: f64 = 0.5;
/// The share of that blur laid between the clumps too, so a coarse grain lets no detail finer than itself
/// read sharp through it; its blur growing with Size, none at the finest grain.
pub const GRAIN_HAZE: f32 = 0.9;

/// The grain of `s` on a scan of extent `scan` at `scale`; `None` at rest or with no share anywhere, so
/// the chain holds no grain stage.
pub fn grain(s: &PipelineSettings, scan: Rect, scale: f64) -> Option<Grain> {
    if s.grain.amount == 0.0 {
        return None;
    }
    let tones = scene_tones(s);
    let share = shares(&s.grain.curve, &tones);
    if share.iter().all(|&v| v == 0.0) {
        return None;
    }
    let cell = s.grain.cell(f64::from(scan.w.max(scan.h)) / scale);
    let m = geometry::scan_from(&s.geometry, scan, scale).map(|r| r.map(|v| v as f32));
    let laid = Grain::on(m, cell as f32, scale as f32, GRAIN_STOPS * s.grain.amount, share).coloured(colours(&s.grain.colour, &tones));
    // Its clumps take in the scene whole at any Amount, so a faint grain is still grains, never a texture laid
    // over the photo: Amount moves their contrast alone (owner's call).
    Some(laid.dissolving(1.0).hazed(GRAIN_HAZE))
}

/// The deviation of the blur a clump of `s` takes in, in this render's pixels on a scan of extent `scan`:
/// a share of its picture whatever its scale, so fit, a zoom and an export dissolve the same detail.
pub fn dissolve(s: &PipelineSettings, scan: Rect) -> f64 {
    GRAIN_DISSOLVE * (s.grain.cell(f64::from(scan.w.max(scan.h))) - f64::from(scan.w) * GRAIN_FINEST).max(0.0)
}

/// `image` grained by `k`, its clumps taking in its blur of deviation `sigma`; under the blur's floor
/// the pixel itself, so a fine grain renders as it did with no blur.
pub fn grained(image: Image, k: &Grain, sigma: f64) -> Image {
    let blurred = if sigma > BLUR_FLOOR { stages::blurred(image.clone(), sigma, false) } else { image.clone() };
    image.then_reading(stages::grain(k), &[blurred])
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

/// The tone each node's light of the scene shows: a digital photo's light crosses its virtual negative first,
/// as its grain lies on the scene before the film, so its shadows keep their grain.
fn scene_tones(s: &PipelineSettings) -> [f32; GRAIN_NODES] {
    // Read on the print at rest and the film's own gains: no print slider, filtration or Scanner key reaches the
    // tones, so a notch on one keeps the grain's pass.
    let rest = PipelineSettings { print: Print::default(), density: color_density::BASE, ..s.clone() };
    let picture = Stage47 { gain: [1.0; 3], ..conversion::stage47(&rest) };
    let Some(f) = s.film() else { return tones(&picture) };
    let stock = film::stock(f.stock);
    let kernel = stock.kernel_at(f.exposure);
    let developed = Stage47 { gain: f.stock.reference().stops.map(f32::exp2), ..picture };
    std::array::from_fn(|i| {
        let y = grain_node_light(i);
        let shown = cpu::stage47(cpu::virtual_negative([y, y, y, 1.0], &kernel, &stock.table), &developed);
        let luma = shown[0] * LUMA[0] + shown[1] * LUMA[1] + shown[2] * LUMA[2];
        luma.max(0.0).powf(1.0 / Print::DISPLAY_GAMMA)
    })
}

/// The curve's share at each node's tone, held to 0…1.
pub fn shares(curve: &GrainCurve, tones: &[f32; GRAIN_NODES]) -> [f32; GRAIN_NODES] {
    let strength = curve.strength();
    tones.map(|t| strength.at(f64::from(t)) as f32)
}

/// The colour line at each node's tone, held to 0…1: the strength curve's tone, the axis its editor shows,
/// so one tone reads both and a place in the editor names the colour laid there too.
pub fn colours(colour: &GrainColour, tones: &[f32; GRAIN_NODES]) -> [f32; GRAIN_NODES] {
    tones.map(|t| colour.black + t.clamp(0.0, 1.0) * (colour.white - colour.black))
}
