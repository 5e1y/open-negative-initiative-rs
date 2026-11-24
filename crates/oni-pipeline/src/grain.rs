//! The grain's arguments: a texture uniform in the film's density, laid on a digital photo's developed film before it
//! is read, anchored on the scan's full-resolution grid so a pan, a zoom, a crop or a turn leaves it on the photo.

use crate::{conversion, film, geometry, stages};
use oni_graph::{Image, Rect};
use oni_settings::axis::pipeline::BLUR_FLOOR;
use oni_render::{GRAIN_NODES, Grain, GrainedNegative, LUMA, Stage47, cpu, grain_node_light};
use oni_settings::axis::density_axis;
use oni_settings::blocks::{GrainColour, GrainCurve};
use oni_settings::tone::{Print, color_density};
use oni_settings::{PipelineSettings, VirtualFilm, VirtualStock};

/// The spread at full resolution of Amount's top, a share of the film's window, its bare film to the sensor's white:
/// a sixteenth, so the Grain a digital photo opens with spreads 1.6 % of it.
pub const GRAIN_WINDOW: f32 = 1.0 / 16.0;
/// The length a clump's blur spares, a share of the scan's full width, the one length every reduction keeps
/// exactly: a pixel on a scan 6000 pixels wide.
pub const GRAIN_FINEST: f64 = 1.0 / 6000.0;
/// The blur a clump takes in, as a share of how much its cell outgrows the finest: none at a grain that fine,
/// so a fine grain blurs nothing, and half a cell's side less a pixel at a coarse one.
pub const GRAIN_DISSOLVE: f64 = 0.5;
/// The share of that blur laid between the clumps too, so a coarse grain lets no detail finer than itself
/// read sharp through it; its blur growing with Size, none at the finest grain.
pub const GRAIN_HAZE: f32 = 0.9;

/// The shares a grey's is sought between, from far over the bare film's light to past any highlight a film holds.
const GREY_SHARES: (f32, f32) = (-2.0, 6.0);

/// Each layer's window on `stock`'s film, in density, its bare film to the sensor's white.
fn windows(stock: VirtualStock) -> [f32; 3] {
    stock.reference().levels().windows().map(|w| w.white - w.black)
}

/// The density Amount's top spreads on `stock`'s film at full resolution: `GRAIN_WINDOW` of its reference's window,
/// its layers' mean, so one Amount grains every stock's picture alike.
pub fn spread(stock: VirtualStock) -> f32 {
    GRAIN_WINDOW * windows(stock).iter().sum::<f32>() / 3.0
}

/// Each layer's share of the spread on `stock`'s negative: its own window over their mean, so the grain moves one
/// share of every layer's window and a grey stays grey.
pub fn layers(stock: VirtualStock) -> [f32; 3] {
    let windows = windows(stock);
    windows.map(|w| 3.0 * w / windows.iter().sum::<f32>())
}

/// The negative `film` hands the chain, as its grain reads it: its bare film under its reading's pedestal and its
/// layers' windows; none on a slide or a look, which hand it a positive.
pub fn negative(film: &VirtualFilm) -> Option<GrainedNegative> {
    if film.stock.entry().reversal || film.stock.look().is_some() {
        return None;
    }
    let veil = density_axis::pedestal(film::CEILING);
    let bare = film::shot(film.stock, [0.0, 0.0, 0.0, 1.0], 0.0);
    Some(GrainedNegative { bare: [bare[0] + veil, bare[1] + veil, bare[2] + veil], veil, layers: layers(film.stock) })
}

/// The grain of `s` on its film, a scan of extent `scan` at `scale`; `None` on a frame shot on no virtual film, at
/// rest or with no share anywhere, so the chain holds no grain stage.
pub fn grain(s: &PipelineSettings, scan: Rect, scale: f64) -> Option<Grain> {
    let f = s.film()?;
    if s.grain.amount == 0.0 {
        return None;
    }
    let tones = tones(s);
    let share = shares(&s.grain.curve, &tones);
    if share.iter().all(|&v| v == 0.0) {
        return None;
    }
    let cell = s.grain.cell(f64::from(scan.w.max(scan.h)) / scale);
    let m = geometry::scan_from(&s.geometry, scan, scale).map(|r| r.map(|v| v as f32));
    let stops = spread(f.stock) * s.grain.amount * std::f32::consts::LOG2_10;
    let laid = Grain::on(m, cell as f32, scale as f32, stops, share).coloured(colours(&s.grain.colour, &tones));
    let laid = match negative(&f) {
        Some(n) => laid.on_negative(n),
        None => laid,
    };
    // Its clumps take in the film whole at any Amount, so a faint grain is still grains, never a texture laid
    // over the photo: Amount moves their contrast alone (owner's call).
    Some(laid.dissolving(1.0).hazed(GRAIN_HAZE))
}

/// The deviation of the blur a clump of `s` takes in, in this render's pixels on a scan of extent `scan`:
/// a share of its picture whatever its scale, so fit, a zoom and an export dissolve the same detail.
pub fn dissolve(s: &PipelineSettings, scan: Rect) -> f64 {
    GRAIN_DISSOLVE * (s.grain.cell(f64::from(scan.w.max(scan.h))) - f64::from(scan.w) * GRAIN_FINEST).max(0.0)
}

/// `image`, a developed film, grained by `k`, its clumps taking in its blur of deviation `sigma`; under the blur's
/// floor the pixel itself, so a fine grain renders as it did with no blur.
pub fn grained(image: Image, k: &Grain, sigma: f64) -> Image {
    let blurred = if sigma > BLUR_FLOOR { stages::blurred(image.clone(), sigma, false) } else { image.clone() };
    image.then_reading(stages::grain(k), &[blurred])
}

/// A light's tone as a display codes it.
fn coded(luma: f32) -> f32 {
    luma.max(0.0).powf(1.0 / Print::DISPLAY_GAMMA)
}

/// The tone the film's light at each node shows as the picture opens, before Lighting: on a negative, the print of
/// the grey holding that light, one share of every layer's window; a slide's or a look's light its own tone.
pub fn tones(s: &PipelineSettings) -> [f32; GRAIN_NODES] {
    let Some((f, n)) = s.film().and_then(|f| negative(&f).map(|n| (f, n))) else { return std::array::from_fn(|i| coded(grain_node_light(i)).min(1.0)) };
    // Read on the print at rest and the film's own gains: no print slider, filtration or Scanner key reaches the
    // tones, so a notch on one keeps the grain's pass.
    let rest = PipelineSettings { print: Print::default(), density: color_density::BASE, ..s.clone() };
    let developed = Stage47 { gain: f.stock.reference().stops.map(f32::exp2), ..conversion::stage47(&rest) };
    let window = spread(f.stock) / GRAIN_WINDOW;
    let grey = |share: f32| -> [f32; 4] {
        let layer = |c: usize| n.bare[c] * 10f32.powf(-window * n.layers[c] * share) - n.veil;
        [layer(0), layer(1), layer(2), 1.0]
    };
    let luma = |p: [f32; 4]| p[0] * LUMA[0] + p[1] * LUMA[1] + p[2] * LUMA[2];
    std::array::from_fn(|i| {
        // The film thins as its share falls: the share whose grey holds the node's light, bisected.
        let (mut thin, mut dense) = GREY_SHARES;
        for _ in 0..40 {
            let mid = 0.5 * (thin + dense);
            if luma(grey(mid)) > grain_node_light(i) { thin = mid } else { dense = mid }
        }
        coded(luma(cpu::stage47(grey(0.5 * (thin + dense)), &developed)))
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
