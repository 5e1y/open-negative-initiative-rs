//! The grain's arguments: grains on a lattice anchored on the scan's full-resolution grid, laid on a digital photo's
//! developed film in its windows' shares, so a pan, a zoom, a crop or a turn leaves them on the photo.

use crate::{film, stages};
use oni_graph::{Image, Rect};
use oni_render::{Grain, GrainedNegative};
use oni_settings::axis::density_axis;
use oni_settings::{PipelineSettings, VirtualFilm, VirtualStock};

/// A grain's deviation at Amount's top, in shares of its layer's window: about four times Portra 800's rest, 0.0398, which
/// lays a real Portra 800 scan's grain through the scanner, 0.014 of green's window in 32-pixel tiles of 47 Mpx, at 30 µm.
pub const GRAIN_TOP: f32 = 0.1563;
/// Each layer's grain against green's: red's emulsion is the finer.
pub const LAYER_GRAIN: [f32; 3] = [0.85, 1.0, 1.0];
/// Each channel's mix of `stock`'s layers' deviations, as its own reading sees the dyes, each row summing to one so a
/// grey's grain stays grey: a colour negative's scanner, a slide's projector; each channel its own layer on silver, on
/// the neutral film's pure dyes and on a look, which hands a positive of no layers.
pub fn gaze(stock: VirtualStock) -> [[f32; 3]; 3] {
    let entry = stock.entry();
    match () {
        _ if entry.silver || entry.neutral || stock.look().is_some() => film::OWN_LAYERS,
        _ if entry.reversal => film::projection::projected(stock).gaze,
        _ => film::stock(stock).gaze,
    }
}

/// Each layer's window on `stock`'s film, in density, its bare film to the sensor's white.
fn windows(stock: VirtualStock) -> [f32; 3] {
    stock.reference().levels().windows().map(|w| w.white - w.black)
}

/// The negative `film` hands the chain, as its grain reads it: its bare film under its reading's pedestal and its
/// layers' windows; none on a slide or a look, which hand it a positive.
pub fn negative(film: &VirtualFilm) -> Option<GrainedNegative> {
    if film.stock.entry().reversal || film.stock.look().is_some() {
        return None;
    }
    let veil = density_axis::pedestal(film::CEILING);
    let bare = film::shot(film.stock, [0.0, 0.0, 0.0, 1.0], 0.0);
    Some(GrainedNegative { bare: [bare[0] + veil, bare[1] + veil, bare[2] + veil], veil, window: windows(film.stock) })
}

/// The grain of `s` on its film, a scan of extent `scan` at `scale`; `None` on a frame shot on no virtual film or at
/// rest, so the chain holds no grain stage.
pub fn grain(s: &PipelineSettings, scan: Rect, scale: f64) -> Option<Grain> {
    let f = s.film()?;
    if s.grain.amount == 0.0 {
        return None;
    }
    let long = f64::from(scan.w.max(scan.h)) / scale;
    let radius = 0.5 * oni_settings::blocks::Grain::pixels(s.grain.size, long);
    // On the scan's own grid, before any crop or turn: a render's pixel to the full frame's.
    let m = [[(1.0 / scale) as f32, 0.0, 0.0], [0.0, (1.0 / scale) as f32, 0.0]];
    let deviation = GRAIN_TOP * s.grain.amount;
    let silver = f.stock.is_silver();
    let layers = if silver { [deviation; 3] } else { LAYER_GRAIN.map(|l| l * deviation) };
    let k = Grain::on(m, radius as f32, (1.0 / scale) as f32, layers, gaze(f.stock));
    let k = if silver { k.on_silver() } else { k };
    Some(match negative(&f) {
        Some(n) => k.on_negative(n),
        None => k,
    })
}

/// `image`, a developed film, grained by `k`, its layers' field drawn over `base`, an image of its extent that its
/// film's and its grain's sliders leave as it is, so their notches draw no grain again.
pub fn grained_over(image: Image, base: &Image, k: &Grain) -> Image {
    assert_eq!(image.extent(), base.extent(), "a grain's field lies on its film's extent");
    let field = base.clone().then(stages::grain_field(k));
    image.then_reading(stages::grain(k), &[field])
}

/// `image`, a developed film, grained by `k`.
pub fn grained(image: Image, k: &Grain) -> Image {
    let base = image.clone();
    grained_over(image, &base, k)
}
