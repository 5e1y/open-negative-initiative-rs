//! Gathering: the library's rows and their sidecars read into the year's frames, and their thumbnails
//! into its colours. Nothing is decoded or rendered: only what the disk already holds is read.

use crate::display::Screen;
use oni_library::index::Entry;
use oni_library::wrapped::palette::{Lab, dominant};
use oni_library::wrapped::{Frame, Moment};
use oni_settings::film::film_stock;
use oni_settings::cg::Rect;
use oni_settings::{ConversionMode, Film};
use std::collections::HashMap;
use std::path::Path;

/// The thumbnails read for the year's colours: past this many, one in every few, spread over the year.
pub const TONES: usize = 400;

/// A stock that names a film: none for a generic one, which says only that nobody picked one.
fn named(key: &str) -> Option<String> {
    film_stock(Some(key)).is_none_or(|k| !k.generic).then(|| key.to_string())
}

/// When the photo was shot: its roll's date typed by hand, else its file's, else the day it joined the library.
fn shot(entry: &Entry, roll: Option<&Film>) -> f64 {
    roll.and_then(|f| f.shot_date).or(entry.captured_at).unwrap_or(entry.imported_at).unix_seconds()
}

/// `year`'s frames among the gallery's `entries`, every one of them for no year, each with what its sidecar
/// holds; `markers` are their folders' rolls. A sidecar that cannot be read leaves its frame as the index describes it.
pub fn frames(entries: &[Entry], markers: &HashMap<String, Film>, year: Option<i32>) -> Vec<Frame> {
    let mut out = Vec::new();
    for entry in entries {
        let roll = oni_library::info::roll_of(entry, markers);
        let unix = shot(entry, roll);
        if year.is_some_and(|y| Moment::local(unix).year != y) {
            continue;
        }
        let path = Path::new(&entry.path);
        let settings = entry.sidecar_format.and_then(|_| oni_settings::sidecar::read(path, oni_decode::decodes_via_mosaic(path)).ok()).map(|l| l.settings);
        let mode = settings.as_ref().map(|s| s.mode).or(entry.mode).unwrap_or_default();
        let digital = mode == ConversionMode::Digital;
        let stock = match (&settings, digital) {
            (Some(s), true) => s.virtual_film.and_then(|v| named(v.stock.catalogue_id())),
            _ => roll.and_then(|f| f.stock.as_deref()).and_then(named),
        };
        let mut frame = Frame {
            path: entry.path.clone(),
            fingerprint: entry.fingerprint.clone(),
            unix,
            medal: entry.label,
            mode,
            roll: roll.map(|f| (f.id.to_string(), f.name.clone())),
            stock,
            body: entry.body().map(str::to_string),
            lens: entry.lens().map(str::to_string),
            ..Default::default()
        };
        if let Some(s) = &settings {
            frame.paper = s.paper().map(|p| p.key().to_string());
            frame.dust = s.corrections.len();
            frame.cropped = !s.geometry.crop_bypassed && s.geometry.crop != Rect::UNIT;
            frame.straightened = s.geometry.angle != 0.0;
            if digital {
                (frame.grain, frame.halation) = (s.grain.amount, s.halation.amount);
            }
        }
        out.push(frame);
    }
    out
}

/// Each photo's own colour off its thumbnail, for those that have one; `paths` holds each photo's
/// path and fingerprint by date.
pub fn tones(support: &Path, paths: &[(String, String)]) -> Vec<Lab> {
    let stride = paths.len().div_ceil(TONES).max(1);
    paths
        .iter()
        .step_by(stride)
        .filter_map(|(_, fingerprint)| crate::thumbnails::load(support, fingerprint, Screen::Srgb))
        .filter_map(|(rgba, width, height)| dominant(&rgba, width, height))
        .collect()
}
