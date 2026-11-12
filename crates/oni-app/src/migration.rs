//! What a launch brings up to date from an earlier format: the support folder's files, the tiles' names,
//! the former app's preferences, the presets; the facts an old sidecar asks, and the sweep of every sidecar.

use crate::timing;
use oni_i18n::marked;
use oni_library::migrate::{self, Imported};
use std::path::Path;

pub mod defaults;
pub mod facts;
pub mod ledger;
pub mod sweep;
pub mod tiles;
pub mod walk;

pub use ledger::Ledger;
pub use sweep::{Place, Sweep, gated, start};

/// The support folder's layout: 2 keeps the places and the projects under `State/` and each tile
/// under `thumbnails::file_name`.
pub const SUPPORT: i64 = 2;

/// Said while an import is due, at the launch and at each change refused.
pub const NOT_BROUGHT: &str = marked(
    "The favourites or projects of an earlier version of ONI could not be brought over. They are shown as they were and brought at the next launch; until then, changes to them are not kept.",
);
pub const UNPARSED: &str = marked("A file of favourites or projects of an earlier version of ONI could not be read, so nothing was taken from it. It is left as it was.");
pub const OLDER_SINCE: &str = marked("An earlier version of ONI changed favourites or projects after this version brought them over. Those changes are not carried over.");

/// What a launch's bringing leaves the app: an import due is shown from its old file and never
/// written, so the next launch imports it; a notice to say once.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Brought {
    pub places_due: bool,
    pub projects_due: bool,
    pub notice: Option<&'static str>,
    /// The sidecars' new format is said once, where the support folder held something before.
    pub tell: bool,
}

/// What a support folder holds once any build of this line or the former app ran there.
const HELD: [&str; 9] = ["preferences.json", "roots.json", "favourites.json", "projects.json", "library.json", "Thumbnails", "Presets", "State/places.json", "State/projects.json"];

/// Synchronous, milliseconds; a step that fails is tried again at the next launch.
pub fn support(support: &Path) -> Brought {
    brought(support, defaults::plist().as_deref())
}

/// `support` with the former app's preferences read from `plist`, none when `None`; the sidecars'
/// new format said once, and the presets brought to it.
pub fn brought(support: &Path, plist: Option<&Path>) -> Brought {
    let Some(mut ledger) = ledger::read(support) else {
        timing::line("support folder: its ledger comes from a newer build, left as it is");
        return Brought::default();
    };
    let presets = oni_core::presets::brought(&oni_core::presets::directory(support));
    if presets > 0 {
        timing::line(&format!("support folder: {presets} presets brought to this format"));
    }
    let tell = ledger.told < oni_settings::document::VERSION && HELD.iter().any(|name| support.join(name).exists());
    // A support folder that held nothing has nothing to be told; one that did is told once the sheet shows.
    if ledger.told < oni_settings::document::VERSION && !tell {
        ledger.told = oni_settings::document::VERSION;
        kept(support, &ledger);
    }
    Brought { tell, ..imported(support, plist, ledger) }
}

/// The sheet saying the sidecars' new format shown: never again in this support folder.
pub fn told(support: &Path) {
    if let Err(e) = ledger::update(support, |l| l.told = oni_settings::document::VERSION) {
        timing::line(&format!("support folder: the ledger could not be written ({e})"));
    }
}

fn imported(support: &Path, plist: Option<&Path>, mut ledger: Ledger) -> Brought {
    let written = migrate::last_written(support);
    if ledger.support >= SUPPORT {
        if written <= ledger.former_written {
            return Brought::default();
        }
        timing::line("support folder: an older build wrote its favourites or projects since they were brought");
        ledger.former_written = written;
        kept(support, &ledger);
        return Brought { notice: Some(OLDER_SINCE), ..Brought::default() };
    }
    let preferences = plist.map_or(0, |plist| defaults::import(support, plist));
    let places = migrate::places(support);
    let projects = migrate::projects(support);
    let tiles = tiles::renamed(&crate::thumbnails::directory(support));
    timing::line(&format!(
        "support folder: places {}, projects {}, {tiles} tiles renamed, {preferences} preferences taken",
        said(&places),
        said(&projects)
    ));
    let (places_due, projects_due) = (places.is_err(), projects.is_err());
    if !places_due && !projects_due {
        (ledger.support, ledger.former_written) = (SUPPORT, written);
        kept(support, &ledger);
    }
    let unparsed = [&places, &projects].iter().any(|i| matches!(i, Ok(Imported::Unparsed)));
    let notice = if places_due || projects_due { Some(NOT_BROUGHT) } else { unparsed.then_some(UNPARSED) };
    Brought { places_due, projects_due, notice, tell: false }
}

fn kept(support: &Path, ledger: &Ledger) {
    if let Err(e) = ledger::write(support, ledger) {
        timing::line(&format!("support folder: the ledger could not be written ({e})"));
    }
}

fn said(imported: &std::io::Result<Imported>) -> String {
    match imported {
        Ok(Imported::Brought) => "imported".into(),
        Ok(Imported::Nothing) => "as they were".into(),
        Ok(Imported::Unparsed) => "not imported, the old file does not parse".into(),
        Err(e) => format!("not imported, tried again at the next launch ({e})"),
    }
}

#[cfg(test)]
mod sweep_tests;
#[cfg(test)]
mod tests;
