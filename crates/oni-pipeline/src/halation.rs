//! The halation's arguments: each film layer's whole light, every light feeding it, spread over the film with a long
//! tail and added to the layers before their development, the red layer alone.

use crate::stages;
use oni_graph::{Image, Rect, Source};
use oni_render::{HALATION_TAIL, Halation};
use oni_settings::PipelineSettings;

/// The glow's deviation in this render's pixels, on a scan of extent `scan`: its width on the film whatever the
/// scale.
pub fn deviation(s: &PipelineSettings, scan: Rect) -> f64 {
    s.halation.pixels(f64::from(scan.w.max(scan.h)))
}

/// The dose of the law per unit of Halation's amount: the amount is what the person sets and each film's file holds; a law
/// sending back every light, none under a threshold, asks this share of it (owner's call, Portra 800's 0.31 laying 0.05).
pub const DOSE_PER_AMOUNT: f32 = 0.05 / 0.31;

/// The dose each layer of the film of `s` takes, a silver film's one layer the same on its three channels.
pub fn kernel(s: &PipelineSettings) -> Halation {
    Halation::on(s.halation.amount * DOSE_PER_AMOUNT, s.film().is_some_and(|f| f.stock.is_silver()))
}

/// Whether `s` lays a halation: none at rest.
pub fn lays(s: &PipelineSettings) -> bool {
    s.halation.amount != 0.0
}

/// The long side of the film's plate a glow is drawn on, the preview's.
pub const PLATE_SIDE: f64 = 2000.0;

/// The glow of `s`'s halation on `exposed`, the film's plate of its layers' exposures or of the scene a film of no
/// layer reads: their whole light spread as `exp(-r / λ)`, λ the glow's width there, by `HALATION_TAIL`'s Gaussians.
pub fn glow(exposed: Image, s: &PipelineSettings) -> Image {
    let scan = exposed.extent();
    spread(exposed, s, scan)
}

/// `exposed`'s whole light spread by `HALATION_TAIL`'s Gaussians, λ the glow's width on a scan of extent `scan`.
fn spread(exposed: Image, s: &PipelineSettings, scan: Rect) -> Image {
    let width = deviation(s, scan);
    let [near, mid, far] = HALATION_TAIL.map(|t| stages::blurred(exposed.clone(), width * f64::from(t[0]), false));
    near.then_reading(stages::halation_tail(), &[mid, far])
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
    let glow = spread(exposed.clone(), s, scan);
    exposed.then_reading(stages::halation(&kernel(s)), &[glow])
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
