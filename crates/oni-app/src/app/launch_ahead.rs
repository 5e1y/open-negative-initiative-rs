//! What the starter prepares while the version's picture shows: the film a new photo is read on and the photo opened
//! last, its film baked, its preview decoded, laid on the GPU and scanned, so its opening shows it at once.

use super::*;
use oni_library::journal::Kind;

impl Ctl {
    /// The photo opened last that still stands on disk, as the year's journal holds it.
    pub(in crate::app) fn last_opened(&self) -> Option<String> {
        let (records, subjects) = self.journal().read();
        latest_opened(&records, &subjects, |p| Path::new(p).is_file())
    }

    /// The work begun off this thread, behind any decode the hand asks: none of it holds the starter, and what is
    /// left when the app opens goes on as a background decode does.
    pub(in crate::app) fn prepare_ahead(&self) {
        let last = self.last_opened();
        let (engine, previews) = (self.engine.clone(), self.state.borrow().previews.clone());
        let _ = std::thread::Builder::new().name("ahead".into()).spawn(move || {
            crate::decode_gate::in_background();
            let _ = oni_pipeline::film::stock(oni_settings::VirtualStock::default());
            if let Some(path) = last {
                ahead_of(&engine, &previews, &path);
            }
        });
    }
}

/// The newest photo `records` note as opened that `stands`.
fn latest_opened(records: &[oni_library::journal::Record], subjects: &[String], stands: impl Fn(&str) -> bool) -> Option<String> {
    records.iter().rev().filter(|r| r.kind == Kind::Opened).filter_map(|r| subjects.get(r.subject as usize)).find(|p| stands(p)).cloned()
}

/// `path`'s film baked, and its preview, as its sidecar reads it, decoded among the kept previews and scanned.
fn ahead_of(engine: &render::Shared, previews: &Arc<Mutex<render::Previews>>, path: &str) {
    let mosaic = oni_decode::decodes_via_mosaic(Path::new(path));
    let Ok(read) = oni_settings::sidecar::read(Path::new(path), mosaic) else { return };
    let settings = read.settings;
    // The neutral film and a look bake no profile of their own.
    if let Some(film) = settings.film().filter(|f| { let e = f.stock.entry(); !e.neutral && e.look.is_none() }) {
        let _ = oni_pipeline::film::stock(film.stock);
    }
    let reading = render::Reading::of(&settings);
    if previews.lock().is_ok_and(|mut p| p.get(path, &reading).is_some()) {
        return;
    }
    let Ok((negative, _)) = super::workers::prepared_keeping(engine, path, reading.clone()) else { return };
    if let Ok(mut p) = previews.lock() {
        p.insert(path.to_string(), reading, negative.clone());
    }
    if let Ok(mut e) = engine.lock() {
        let _ = e.preload(&negative, &settings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_library::journal::Record;

    #[test]
    fn the_photo_prepared_is_the_newest_opened_that_still_stands() {
        let record = |kind, subject| Record { kind, detail: 0, time: 0, subject, value: 0 };
        let subjects = ["/a.RW2", "/b.RW2", "/gone.RW2"].map(String::from).to_vec();
        let records = [record(Kind::Opened, 0), record(Kind::Opened, 1), record(Kind::Edited, 0), record(Kind::Opened, 2)];
        assert_eq!(latest_opened(&records, &subjects, |p| p != "/gone.RW2"), Some("/b.RW2".into()), "the newest standing, an edit no opening");
        // Twin: every photo gone, none is prepared.
        assert_eq!(latest_opened(&records, &subjects, |_| false), None);
        assert_eq!(latest_opened(&records[..1], &subjects, |_| true), Some("/a.RW2".into()));
    }
}
