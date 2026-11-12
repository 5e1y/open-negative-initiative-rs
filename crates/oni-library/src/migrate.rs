//! The support files builds before `State/` kept, each read once into its file there. The old files
//! stay as they are: those builds still read them, and no file of `State/` can be set aside by them.

use crate::paths;
use crate::places::{Place, Places, Volume};
use crate::project::{self, ProjectsFile};
use crate::roots::Roots;
use crate::state;
use oni_settings::Date;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::UNIX_EPOCH;

pub mod projects;

/// The old files a person's changes reach: favourites and projects.
const FORMER: [&str; 2] = ["favourites.json", "projects.json"];

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

/// An old file as its build wrote it; a disk refusing to read it is an error, tried again later.
#[derive(Debug, PartialEq)]
pub enum Old<T> {
    Absent,
    /// It does not parse: its own build set such a file aside and read none, so none is taken.
    Unparsed,
    Read(T),
}

impl<T> Old<T> {
    pub fn read(self) -> Option<T> {
        match self {
            Old::Read(value) => Some(value),
            Old::Absent | Old::Unparsed => None,
        }
    }
}

/// What an import did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Imported {
    /// The new file written from the old one, as read back.
    Brought,
    /// The new file stands already, or no old file was kept.
    Nothing,
    /// The old file does not parse: nothing taken, the file left as it is.
    Unparsed,
}

/// `State/places.json` from `favourites.json` and `bookmarks.json`, unless it stands already. Nothing
/// when no favourite was ever kept: the roots seed them then, as at a first launch.
pub fn places(support: &Path) -> std::io::Result<Imported> {
    if Places::file_path(support).exists() {
        return Ok(Imported::Nothing);
    }
    let built = match former_places(support)? {
        Old::Read(built) => built,
        Old::Absent => return Ok(Imported::Nothing),
        Old::Unparsed => return Ok(Imported::Unparsed),
    };
    let path = Places::file_path(support);
    built.save(support)?;
    verified(&path, state::read(&path, crate::places::VERSION), built)
}

/// The favourites as the old files hold them, in their order, each with the identity kept on its path.
pub fn former_places(support: &Path) -> std::io::Result<Old<Places>> {
    let favourites = match old::<Favourites>(&support.join("favourites.json"))? {
        Old::Read(favourites) => favourites,
        Old::Absent => return Ok(Old::Absent),
        Old::Unparsed => return Ok(Old::Unparsed),
    };
    let known = old::<Bookmarks>(&support.join("bookmarks.json"))?.read().map(|b| b.places).unwrap_or_default();
    let identity = |path: &str| known.iter().find(|(p, _)| paths::normal(p) == path).map(|(_, i)| i);
    let mut built = Places { version: crate::places::VERSION, places: favourites.paths.iter().map(|p| Place::at(p)).collect() }.respelt();
    for place in &mut built.places {
        if let Some(found) = identity(&place.path) {
            (place.volume, place.file_id) = (found.volume.clone(), found.file_id);
        }
    }
    Ok(Old::Read(built))
}

/// The favourites shown while their import is due, which nothing writes: the old files' when they
/// read, else the roots', as a first launch seeds them.
pub fn places_held(support: &Path) -> Places {
    former_places(support).ok().and_then(Old::read).unwrap_or_else(|| Places::seeded(&Roots::load(support)))
}

/// `State/projects.json` from `projects.json`, any format 1 to 4, unless it stands already.
pub fn projects(support: &Path) -> std::io::Result<Imported> {
    if project::file_path(support).exists() {
        return Ok(Imported::Nothing);
    }
    let built = match former_projects(support)? {
        Old::Read(built) => built,
        Old::Absent => return Ok(Imported::Nothing),
        Old::Unparsed => return Ok(Imported::Unparsed),
    };
    project::save(support, &built)?;
    let path = project::file_path(support);
    verified(&path, state::read(&path, project::FORMAT), built)
}

/// The projects as the old file's build showed them, in this build's format.
pub fn former_projects(support: &Path) -> std::io::Result<Old<ProjectsFile>> {
    let bytes = match std::fs::read(support.join("projects.json")) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Old::Absent),
        Err(e) => return Err(e),
    };
    let Some(read) = projects::read(&bytes) else { return Ok(Old::Unparsed) };
    // The file keeps whole seconds, so the value compared after the write is built with them.
    let whole = |d: Date| Date::since_reference_date(d.time_interval_since_reference_date().floor());
    let named = |m: &project::Member| m.path.is_some() || m.print.is_some();
    let projects = read.into_iter().map(|p| project::Project { created_at: whole(p.created_at), members: p.members.into_iter().filter(named).collect(), ..p }).collect();
    Ok(Old::Read(ProjectsFile { version: project::FORMAT, projects }))
}

/// The newest modification of the old files a person's changes reach, in seconds since 1970; `None`
/// when neither stands. Newer than at the import, an older build wrote them since.
pub fn last_written(support: &Path) -> Option<f64> {
    let seconds = |name: &str| std::fs::metadata(support.join(name)).and_then(|m| m.modified()).ok()?.duration_since(UNIX_EPOCH).ok();
    FORMER.iter().filter_map(|name| seconds(name)).max().map(|d| d.as_secs_f64())
}

/// An old file as its build wrote it; never set aside nor written.
fn old<T: DeserializeOwned>(path: &Path) -> std::io::Result<Old<T>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes).map_or(Old::Unparsed, Old::Read)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Old::Absent),
        Err(e) => Err(e),
    }
}

/// The new file read back and compared with what was built; one that differs is taken away, so the
/// next launch imports again.
fn verified<T: PartialEq>(new_file: &Path, back: state::Read<T>, built: T) -> std::io::Result<Imported> {
    if back == state::Read::Current(built) {
        return Ok(Imported::Brought);
    }
    let _ = std::fs::remove_file(new_file);
    Err(std::io::Error::other("the file written does not read back as built"))
}

#[cfg(test)]
mod tests;
