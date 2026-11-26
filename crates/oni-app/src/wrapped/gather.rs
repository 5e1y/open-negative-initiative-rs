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

/// `work` over `items` on every core, each core a run of them, the results in the items' order.
fn spread<T: Sync, R: Send>(items: &[T], work: impl Fn(&T) -> Option<R> + Sync) -> Vec<R> {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let run = items.len().div_ceil(cores).max(1);
    let work = &work;
    std::thread::scope(|scope| {
        let hands: Vec<_> = items.chunks(run).map(|chunk| scope.spawn(move || chunk.iter().filter_map(work).collect::<Vec<R>>())).collect();
        hands.into_iter().flat_map(|hand| hand.join().unwrap_or_default()).collect()
    })
}

/// `year`'s frames among the gallery's `entries`, every one of them for no year, each with what its sidecar
/// holds; `markers` are their folders' rolls. A sidecar that cannot be read leaves its frame as the index describes it.
pub fn frames(entries: &[Entry], markers: &HashMap<String, Film>, year: Option<i32>) -> Vec<Frame> {
    spread(entries, |entry| {
        let roll = oni_library::info::roll_of(entry, markers);
        let unix = shot(entry, roll);
        if year.is_some_and(|y| Moment::local(unix).year != y) {
            return None;
        }
        let path = Path::new(&entry.path);
        // The original is never opened, which a disk away would make wait: its name says how it decodes.
        let settings = entry.sidecar_format.and_then(|_| oni_settings::sidecar::read(path, oni_decode::is_raw(path)).ok()).map(|l| l.settings);
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
        Some(frame)
    })
}

/// Each photo's own colour off its thumbnail, for those that have one; `paths` holds each photo's
/// path and fingerprint by date.
pub fn tones(support: &Path, paths: &[(String, String)]) -> Vec<Lab> {
    let stride = paths.len().div_ceil(TONES).max(1);
    let read: Vec<&(String, String)> = paths.iter().step_by(stride).collect();
    spread(&read, |(_, fingerprint)| {
        let (rgba, width, height) = crate::thumbnails::load(support, fingerprint, Screen::Srgb)?;
        dominant(&rgba, width, height)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_spread_over_the_cores_keeps_the_items_order_and_drops_what_gives_nothing() {
        let items: Vec<usize> = (0..1_000).collect();
        let odd = spread(&items, |n| (n % 2 == 1).then_some(n * 10));
        assert_eq!(odd.len(), 500);
        assert!(odd.windows(2).all(|w| w[0] < w[1]) && odd[0] == 10 && odd[499] == 9_990);
        assert_eq!(spread(&[] as &[usize], |n| Some(*n)), Vec::<usize>::new());
        // Twin: work that gives nothing gives no result, whatever the items.
        assert!(spread(&items, |_| None::<usize>).is_empty());
    }

    #[test]
    fn a_generic_stock_names_no_film_and_a_roll_s_typed_date_dates_its_frames() {
        assert_eq!(named("generic-color-negative"), None);
        assert_eq!(named("kodak-portra400").as_deref(), Some("kodak-portra400"));
        assert_eq!(named("a-film-no-file-defines").as_deref(), Some("a-film-no-file-defines"), "kept by its key until its file is back");
        let imported = oni_settings::Date::from_unix_seconds(1_800_000_000.0);
        let mut entry = Entry::new("/roll/a.tif", imported, "fp");
        assert_eq!(shot(&entry, None), 1_800_000_000.0);
        entry.captured_at = Some(oni_settings::Date::from_unix_seconds(1_700_000_000.0));
        assert_eq!(shot(&entry, None), 1_700_000_000.0);
        let roll = Film { shot_date: Some(oni_settings::Date::from_unix_seconds(1_600_000_000.0)), ..Film::new("Lisbon") };
        assert_eq!(shot(&entry, Some(&roll)), 1_600_000_000.0);
        // Twin: a roll with no date typed leaves the file's.
        assert_eq!(shot(&entry, Some(&Film::new("Lisbon"))), 1_700_000_000.0);
    }
}
