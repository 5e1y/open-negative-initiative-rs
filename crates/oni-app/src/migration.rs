//! What a launch brings up to date before anything reads the support folder: its files whose shape
//! changed moved under `State/`, the thumbnails' names, the former app's preferences. Each step once.

use crate::timing;
use std::path::Path;

pub mod defaults;
pub mod ledger;
pub mod tiles;

pub use ledger::Ledger;

/// The support folder's layout: 2 keeps the places and the projects under `State/` and each tile
/// under `thumbnails::file_name`.
pub const SUPPORT: i64 = 2;

/// Synchronous, milliseconds; a step that fails is tried again at the next launch.
pub fn support(support: &Path) -> Ledger {
    brought(support, defaults::plist().as_deref())
}

/// `support` with the former app's preferences read from `plist`, none when `None`.
pub fn brought(support: &Path, plist: Option<&Path>) -> Ledger {
    let Some(mut ledger) = ledger::read(support) else {
        timing::line("support folder: its ledger comes from a newer build, left as it is");
        return Ledger::default();
    };
    if ledger.support >= SUPPORT {
        return ledger;
    }
    let places = oni_library::migrate::places(support);
    let projects = oni_library::migrate::projects(support);
    let tiles = tiles::renamed(&crate::thumbnails::directory(support));
    let preferences = plist.map_or(0, |plist| defaults::import(support, plist));
    timing::line(&format!(
        "support folder: places {}, projects {}, {tiles} tiles renamed, {preferences} preferences taken",
        said(&places),
        said(&projects)
    ));
    if places.is_ok() && projects.is_ok() {
        ledger.support = SUPPORT;
        if let Err(e) = ledger::write(support, &ledger) {
            timing::line(&format!("support folder: the ledger could not be written ({e})"));
        }
    }
    ledger
}

fn said(imported: &std::io::Result<bool>) -> String {
    match imported {
        Ok(true) => "imported".into(),
        Ok(false) => "as they were".into(),
        Err(e) => format!("not imported ({e})"),
    }
}

#[cfg(test)]
mod tests;
