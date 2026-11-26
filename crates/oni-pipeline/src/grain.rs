//! The grain's arguments: grains on a lattice anchored on the scan's full-resolution grid, laid on a digital photo's
//! developed film in its windows' shares, so a pan, a zoom, a crop or a turn leaves them on the photo.

use crate::{film, geometry, stages};
use oni_graph::{Image, Rect, Source};
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
    let m = geometry::scan_from(&s.geometry, scan, scale).map(|r| r.map(|v| v as f32));
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

/// `image`, a developed film, grained by `k`, its layers' field drawn on itself.
pub fn grained(image: Image, k: &Grain) -> Image {
    let field = image.clone().then(stages::grain_field(k));
    let e = image.extent();
    image.then_reading(stages::grain(k, [e, e]), &[field.clone(), field])
}

/// The pitch of the field's finest level, in full-resolution pixels: four texels across a grain.
pub fn finest(radius: f32) -> f32 {
    0.5 * radius
}

/// The layers' field on the scan's grid, drawn once a photo and grain size: each level's source and pitch, the finest
/// drawn, each next the mean of its 2 × 2 texels, down to a texel across the frame.
#[derive(Clone)]
pub struct Plate {
    pub levels: Vec<(Source, f32)>,
    /// The gain at footprints half an octave apart from a full-resolution pixel's: the deviation a render drawing its
    /// field at that footprint lays, its spread included, over the plate's as such a render reads it.
    pub gains: Vec<(f32, f32)>,
}

impl Plate {
    /// The gain at `footprint`, between the two measured about it by its place in their half octave.
    pub fn gain(&self, footprint: f32) -> f32 {
        let at = self.gains.iter().rposition(|(f, _)| *f <= footprint).unwrap_or(0);
        let (f0, g0) = self.gains[at];
        let Some(&(f1, g1)) = self.gains.get(at + 1) else { return g0 };
        let share = ((footprint.max(f0)).log2() - f0.log2()) / (f1.log2() - f0.log2());
        g0 * (g1 / g0).powf(share.clamp(0.0, 1.0))
    }
}

/// The pitches of a plate for grains of `radius` on a scan of `full` full-resolution pixels.
pub fn pitches(radius: f32, full: (f64, f64)) -> Vec<f32> {
    let long = full.0.max(full.1) as f32;
    std::iter::successors(Some(finest(radius)), |p| Some(2.0 * p)).take_while(|p| long / p >= 1.0).collect()
}

/// The level of pitch `pitch` for grains of `radius` on a scan of `full` pixels, the photo's `seed`, over `base`.
pub fn level(base: &Image, radius: f32, pitch: f32, full: (f64, f64), seed: [u32; 2], silver: bool) -> Image {
    let k = Grain::level(radius, pitch).seeded(seed);
    let k = if silver { k.on_silver() } else { k };
    let extent = Rect::new(0, 0, (full.0 / f64::from(pitch)).ceil() as u32, (full.1 / f64::from(pitch)).ceil() as u32);
    base.clone().then(stages::GrainLevel(k, extent))
}

/// How a render of `footprint` full-resolution pixels reads `plate`: the level, and the box's half width in its texels,
/// zero for a smooth read. Under twice the finest pitch the finest level is read smooth, its texels holding the grain's
/// contrast at every phase; past it a box two to four texels wide on the level whose pitch is under half the footprint.
pub fn reading(plate_pitches: &[f32], footprint: f32) -> (usize, f32) {
    if footprint < 2.0 * plate_pitches[0] {
        return (0, 0.0);
    }
    let level = plate_pitches.iter().rposition(|p| *p <= 0.5 * footprint).unwrap_or(0);
    (level, 0.5 * footprint / plate_pitches[level])
}

/// `image`, a developed film, grained by `k`, its layers' field read off `plate` as a render of its footprint reads it.
pub fn grained_on(image: Image, k: &Grain, plate: &Plate) -> Image {
    let footprint = k.footprint();
    let pitches: Vec<f32> = plate.levels.iter().map(|l| l.1).collect();
    let (level, half) = reading(&pitches, footprint);
    let (source, pitch) = &plate.levels[level];
    let k = k.plated(*pitch, plate.gain(footprint), half);
    let e = source.extent();
    image.then_reading(stages::grain(&k, [e, e]), &[Image::new(source), Image::new(source)])
}
