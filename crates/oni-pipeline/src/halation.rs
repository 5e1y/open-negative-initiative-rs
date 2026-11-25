//! The halation's arguments: a dose of each film layer's exposure, blurred over its width on the film and laid back
//! on the layers before their development, the red layer's most.

use crate::stages;
use oni_graph::{Image, Rect};
use oni_render::Halation;
use oni_settings::PipelineSettings;

/// The glow's deviation in this render's pixels, on a scan of extent `scan`: its width on the film whatever the
/// scale.
pub fn deviation(s: &PipelineSettings, scan: Rect) -> f64 {
    s.halation.pixels(f64::from(scan.w.max(scan.h)))
}

/// The dose each layer of the film of `s` takes: a silver film's one layer the same on its three channels.
pub fn kernel(s: &PipelineSettings) -> Halation {
    Halation::on(s.halation.amount, s.film().is_some_and(|f| f.stock.is_silver()))
}

/// Whether `s` lays a halation: none at rest.
pub fn lays(s: &PipelineSettings) -> bool {
    s.halation.amount != 0.0
}

/// `exposed`, a film's layers' exposures or the scene a film of no layer reads, with the halation of `s` on a scan
/// of extent `scan`; itself at rest, with no stage.
pub fn halated(exposed: Image, s: &PipelineSettings, scan: Rect) -> Image {
    if !lays(s) {
        return exposed;
    }
    let glow = stages::blurred(exposed.clone(), deviation(s, scan), false);
    exposed.then_reading(stages::halation(&kernel(s)), &[glow])
}
