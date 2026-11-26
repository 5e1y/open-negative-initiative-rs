//! The halation's arguments: each film layer's light weighted by how near the sensor's saturation it stands, blurred
//! over its width on the film and added to the layers before their development, the red layer's most.

use crate::{film, stages};
use oni_graph::{Image, Rect, Source};
use oni_render::{Halation, cpu};
use oni_settings::PipelineSettings;

/// The glow's deviation in this render's pixels, on a scan of extent `scan`: its width on the film whatever the
/// scale.
pub fn deviation(s: &PipelineSettings, scan: Rect) -> f64 {
    s.halation.pixels(f64::from(scan.w.max(scan.h)))
}

/// Each layer's exposure at the sensor's saturation on the film of `s`, at its own exposure, so a light's weight is its
/// share of what the sensor held; the scene's own on the neutral film and a look, whose layers are its channels.
pub fn saturation(s: &PipelineSettings) -> [f32; 3] {
    let white = film::saturation();
    let Some(f) = s.film().filter(|f| film::neutral::kernel(f.stock).is_none() && f.stock.look().is_none()) else { return [white; 3] };
    let stock = film::stock(f.stock);
    let e = cpu::virtual_exposure([white, white, white, 1.0], &stock.kernel_at(f.exposure), &stock.table);
    [e[0], e[1], e[2]]
}

/// The dose each layer of the film of `s` takes, a silver film's one layer the same on its three channels, and their
/// exposures at the saturation.
pub fn kernel(s: &PipelineSettings) -> Halation {
    Halation::on(s.halation.amount, s.film().is_some_and(|f| f.stock.is_silver()), saturation(s))
}

/// Whether `s` lays a halation: none at rest.
pub fn lays(s: &PipelineSettings) -> bool {
    s.halation.amount != 0.0
}

/// The long side of the film's plate a glow is drawn on, the preview's.
pub const PLATE_SIDE: f64 = 2000.0;

/// The glow of `s`'s halation on `exposed`, the film's plate of its layers' exposures or of the scene a film of no
/// layer reads: their light weighted by its share of the saturation, blurred over the glow's width there.
pub fn glow(exposed: Image, s: &PipelineSettings) -> Image {
    let scan = exposed.extent();
    stages::blurred(exposed.then(stages::halation_source(&kernel(s))), deviation(s, scan), false)
}

/// What a render reads its glow off: the film's plate, drawn once a photo and its exposure, and where a render
/// pixel's centre lies on it.
#[derive(Clone)]
pub struct Glow {
    pub plate: Source,
    pub map: [[f32; 3]; 2],
}

/// `exposed`, a film's layers' exposures or the scene a film of no layer reads on a scan of extent `scan`, with the
/// halation of `s` drawn on itself, its own plate; itself at rest, with no stage.
pub fn halated(exposed: Image, s: &PipelineSettings, scan: Rect) -> Image {
    if !lays(s) {
        return exposed;
    }
    let k = kernel(s);
    let glow = stages::blurred(exposed.clone().then(stages::halation_source(&k)), deviation(s, scan), false);
    exposed.then_reading(stages::halation(&k), &[glow])
}

/// `exposed`, a film's layers' exposures or the scene a film of no layer reads, with the halation of `s`, its glow read
/// off `glow`; itself at rest, with no stage.
pub fn halated_from(exposed: Image, s: &PipelineSettings, glow: &Glow) -> Image {
    if !lays(s) {
        return exposed;
    }
    let plate = glow.plate.extent();
    exposed.then_reading(stages::halation_laid(&kernel(s), glow.map, plate), &[Image::new(&glow.plate)])
}
