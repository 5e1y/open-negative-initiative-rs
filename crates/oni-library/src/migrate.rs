//! The support files builds before `State/` kept, each read once into its file there. The old files
//! stay as they are: those builds still read them, and no file of `State/` can be set aside by them.

use crate::paths;
use crate::places::{Place, Places, Volume};
use crate::project::{self, ProjectsFile};
use crate::state;
use oni_settings::Date;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::path::Path;

pub mod projects;

/// `favourites.json`: the starred paths, in their order.
#[derive(Deserialize)]
struct Favourites {
    paths: Vec<String>,
}

/// `bookmarks.json`: each favourite's volume and file id, by its path.
#[derive(Deserialize)]
struct Bookmarks {
    #[serde(default)]
    places: BTreeMap<String, Identity>,
}

#[derive(Deserialize)]
struct Identity {
    #[serde(default)]
    volume: Option<Volume>,
    #[serde(rename = "fileId", default)]
    file_id: Option<u64>,
}

/// `State/places.json` from `favourites.json` and `bookmarks.json`, unless it stands already; whether
/// it imported. Nothing when no favourite was ever kept: the roots seed them then, as at a first launch.
pub fn places(support: &Path) -> std::io::Result<bool> {
    if Places::file_path(support).exists() {
        return Ok(false);
    }
    places_imported(support)
}

/// The import itself, whatever stands at the new name.
fn places_imported(support: &Path) -> std::io::Result<bool> {
    let Some(favourites) = old::<Favourites>(&support.join("favourites.json")) else { return Ok(false) };
    let known = old::<Bookmarks>(&support.join("bookmarks.json")).map(|b| b.places).unwrap_or_default();
    let identity = |path: &str| known.iter().find(|(p, _)| paths::normal(p) == path).map(|(_, i)| i);
    let mut built = Places { version: crate::places::VERSION, places: favourites.paths.iter().map(|p| Place::at(p)).collect() }.respelt();
    for place in &mut built.places {
        if let Some(found) = identity(&place.path) {
            (place.volume, place.file_id) = (found.volume.clone(), found.file_id);
        }
    }
    let path = Places::file_path(support);
    built.save(support)?;
    verified(&path, state::read(&path, crate::places::VERSION), built)
}

/// `State/projects.json` from `projects.json`, any format 1 to 4, unless it stands already; whether it imported.
pub fn projects(support: &Path) -> std::io::Result<bool> {
    if project::file_path(support).exists() {
        return Ok(false);
    }
    projects_imported(support)
}

fn projects_imported(support: &Path) -> std::io::Result<bool> {
    let Some(read) = std::fs::read(support.join("projects.json")).ok().and_then(|b| projects::read(&b)) else { return Ok(false) };
    // The file keeps whole seconds, so the value compared after the write is built with them.
    let whole = |d: Date| Date::since_reference_date(d.time_interval_since_reference_date().floor());
    let named = |m: &project::Member| m.path.is_some() || m.print.is_some();
    let projects = read.into_iter().map(|p| project::Project { created_at: whole(p.created_at), members: p.members.into_iter().filter(named).collect(), ..p }).collect();
    let built = ProjectsFile { version: project::FORMAT, projects };
    project::save(support, &built)?;
    let path = project::file_path(support);
    verified(&path, state::read(&path, project::FORMAT), built)
}

/// An old file as its build wrote it, `None` when absent or unreadable; never set aside nor written.
fn old<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// The new file read back and compared with what was built; one that differs is taken away, so the
/// next launch imports again.
fn verified<T: PartialEq>(new_file: &Path, back: state::Read<T>, built: T) -> std::io::Result<bool> {
    if back == state::Read::Current(built) {
        return Ok(true);
    }
    let _ = std::fs::remove_file(new_file);
    Err(std::io::Error::other("the file written does not read back as built"))
}

#[cfg(test)]
mod tests;
