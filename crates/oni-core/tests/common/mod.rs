//! What the store's checks share.

#![allow(dead_code)]

use oni_core::EditorStore;
use oni_settings::Date;
use std::path::Path;

/// A mark laid as the app lays it: begun on the store, run as off its thread, landed; `date` none restores.
pub fn marked(store: &mut EditorStore, paths: &[String], date: Option<Date>) -> usize {
    store.mark_begun(paths, date);
    let mosaic = store.mosaic_rule();
    let outcome = oni_core::store::mark_run(paths, date, &|p: &Path| mosaic(p));
    store.mark_landed(paths, &outcome, date)
}

/// `marked` into the ONI Trash, now.
pub fn trash(store: &mut EditorStore, paths: &[String]) -> usize {
    marked(store, paths, Some(Date::now()))
}

/// `marked` back out of the ONI Trash.
pub fn restore(store: &mut EditorStore, paths: &[String]) -> usize {
    marked(store, paths, None)
}
