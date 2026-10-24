//! A roll made on the disk the Finder's way: a folder beside the photos, each frame moved in whole
//! with its sidecars and companions, and the same moves walked back. The disk is the truth.

use crate::folders;
use crate::gallery_history::{Disk, Move};
use crate::paths;
use chrono::{DateTime, Local};
use oni_i18n::{tr, trn};
use oni_settings::{Date, Film};
use std::io;
use std::path::Path;

mod carry;
mod units;

pub use carry::{Carried, Rename, carry, rename_new, said};
pub use units::{Unit, files, plan_units};

/// The earliest capture as year, month, day in local time, numbered past the names taken beside it.
pub fn name_for(earliest: Date, parent: &str) -> String {
    let base = dated(earliest);
    let mut name = base.clone();
    let mut rank = 2;
    while std::fs::symlink_metadata(paths::join(parent, &name)).is_ok() {
        name = format!("{base} {rank}");
        rank += 1;
    }
    name
}

pub fn dated(date: Date) -> String {
    DateTime::from_timestamp(date.unix_seconds().floor() as i64, 0)
        .unwrap_or_default()
        .with_timezone(&Local)
        .format("%Y-%m-%d")
        .to_string()
}

/// The folder made and marked a roll; a name already there is refused, never reused, and a marker
/// the disk refuses leaves no folder behind.
pub fn make(name: &str, parent: &str, film: &Film) -> io::Result<String> {
    let folder = paths::join(parent, name);
    std::fs::create_dir(&folder)?;
    if let Err(e) = folders::set_roll(Some(film), &folder) {
        let _ = folders::set_roll(None, &folder);
        let _ = std::fs::remove_dir(&folder);
        return Err(e);
    }
    Ok(folder)
}

/// The one folder New Roll gathers from; photos spread over several are refused, in words.
pub fn one_folder(photos: &[String]) -> Result<String, String> {
    let mut folders: Vec<String> = photos.iter().map(|p| paths::parent(p)).collect();
    folders.sort();
    folders.dedup();
    match folders.as_slice() {
        [one] => Ok(one.clone()),
        many => Err(tr!(
            "New Roll moves the photos of one folder into a folder made inside it, and these lie in {count} folders. Pick the photos of one folder, then make the roll.",
            count = many.len()
        )),
    }
}

/// What New Roll will do, said in its confirm before anything moves.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub parent: String,
    pub name: String,
    pub film: Film,
    pub units: Vec<Unit>,
    /// The photos picked that move, a RAW's JPEG or a frame's other passes aside.
    pub picked: usize,
    /// Every file that moves: the photos, their sidecars and the files other apps keep beside them.
    pub files: usize,
    /// The photos picked whose original is not there, left out.
    pub absent: usize,
}

impl Plan {
    pub fn folder(&self) -> String {
        paths::join(&self.parent, &self.name)
    }
}

/// New Roll laid out on the disk as it stands: the folder's name, each frame and the files it carries.
pub fn plan(photos: &[String], earliest: Date, parent: &str) -> Plan {
    let present: Vec<String> = photos.iter().filter(|p| std::fs::symlink_metadata(p).is_ok()).cloned().collect();
    let name = name_for(earliest, parent);
    let units = plan_units(&present, &paths::join(parent, &name));
    let moving: std::collections::HashSet<&str> = units.iter().flat_map(|u| u.moves.iter().map(|m| m.from.as_str())).collect();
    Plan {
        parent: parent.to_owned(),
        film: Film::new(&name),
        name,
        files: files(&units),
        picked: present.iter().filter(|p| moving.contains(p.as_str())).count(),
        absent: photos.len() - present.len(),
        units,
    }
}

/// What New Roll did: its folder, and the frames it carried or left.
#[derive(Debug, Clone)]
pub struct Made {
    pub disk: Disk,
    pub carried: Carried,
}

/// The planned folder made, then each frame moved in whole; a folder that took nothing goes again.
pub fn carry_out(plan: &Plan, rename: &Rename) -> io::Result<Made> {
    let folder = make(&plan.name, &plan.parent, &plan.film)?;
    let carried = carry(&plan.units, rename);
    if carried.made.is_empty() {
        remove_if_left(&folder, Some(&plan.film));
    }
    let disk = Disk { folder: Some(folder), film: Some(plan.film.clone()), moves: carried.moves(), units: carried.sizes() };
    Ok(Made { disk, carried })
}

/// An undo or a redo of a roll on the disk, run off the interface's thread.
#[derive(Debug, Clone, PartialEq)]
pub struct Walk {
    pub disk: Disk,
    pub undoing: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Walked {
    pub made: Vec<Move>,
    /// The folder holding what could not move, when some could not.
    pub left_in: Option<String>,
    pub said: Option<String>,
    /// Nothing moved and the step stays to walk again.
    pub refused: bool,
}

pub fn walk(walk: &Walk, rename: &Rename) -> Walked {
    if walk.undoing { take_back(&walk.disk, rename) } else { lay(&walk.disk, rename) }
}

/// The disk's frames as recorded, each whole; a record without frames moves item by item.
fn units_of(disk: &Disk) -> Vec<Unit> {
    let sizes = if disk.units.iter().sum::<usize>() == disk.moves.len() && !disk.units.is_empty() {
        disk.units.clone()
    } else {
        vec![1; disk.moves.len()]
    };
    let mut rest = disk.moves.as_slice();
    sizes
        .iter()
        .map(|n| {
            let (unit, after) = rest.split_at(*n);
            rest = after;
            Unit { moves: unit.to_vec() }
        })
        .collect()
}

/// Every frame back where it was, then the roll's marker and the folder the gesture made, once
/// nothing else lies in it: a file added there meanwhile keeps the folder.
pub fn take_back(disk: &Disk, rename: &Rename) -> Walked {
    let reversed: Vec<Unit> = units_of(disk)
        .into_iter()
        .rev()
        .map(|u| Unit { moves: u.moves.iter().rev().map(|m| Move { from: m.to.clone(), to: m.from.clone() }).collect() })
        .collect();
    let made = carry(&reversed, rename).moves();
    if let Some(folder) = &disk.folder {
        remove_if_left(folder, disk.film.as_ref());
    }
    walked(disk, made, true)
}

/// The roll's folder made again, or found as this roll left it, then every frame moved in again.
/// A folder of that name made meanwhile is never taken over: the redo is refused.
pub fn lay(disk: &Disk, rename: &Rename) -> Walked {
    let refused = |said: String| Walked { said: Some(said), refused: true, ..Walked::default() };
    let mut made_here = false;
    if let Some(folder) = &disk.folder {
        let film = disk.film.as_ref();
        match std::fs::create_dir(folder) {
            Ok(()) => {
                made_here = true;
                if let Err(e) = folders::set_roll(film, folder) {
                    remove_if_left(folder, None);
                    return refused(tr!("The roll's folder could not be made: {e}", e = crate::refusal::cause(&e)));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && ours(folder, film) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                return refused(tr!(
                    "Nothing moved: a folder “{name}” was made in “{parent}” since, and ONI moves no photo into a folder it did not make.",
                    name = paths::name(folder),
                    parent = paths::name(&paths::parent(folder))
                ));
            }
            Err(e) => return refused(tr!("The roll's folder could not be made: {e}", e = crate::refusal::cause(&e))),
        }
    }
    let made = carry(&units_of(disk), rename).moves();
    if made.is_empty() && made_here && let Some(folder) = &disk.folder {
        remove_if_left(folder, disk.film.as_ref());
    }
    walked(disk, made, false)
}

fn walked(disk: &Disk, made: Vec<Move>, undoing: bool) -> Walked {
    let missed = disk.moves.len().saturating_sub(made.len());
    let left_in = if undoing { disk.folder.clone() } else { disk.moves.first().map(|m| paths::parent(&m.from)) };
    Walked { made, left_in: left_in.filter(|_| missed > 0), said: (missed > 0).then(|| shortfall(missed, undoing)), refused: false }
}

/// The folder carries this roll's own marker.
fn ours(folder: &str, film: Option<&Film>) -> bool {
    film.is_some_and(|film| folders::roll(folder).is_some_and(|f| f.id == film.id))
}

/// What a disk lays beside every file and folder on its own: macOS's `._` twins on exFAT, the
/// Finder's `.DS_Store`, Windows' `Thumbs.db`. None of them keeps a folder.
pub fn leftover(name: &str) -> bool {
    name.starts_with("._") || name == ".DS_Store" || name.eq_ignore_ascii_case("Thumbs.db")
}

/// The folder removed when it holds nothing but this roll's marker and what the disk lays there.
pub fn remove_if_left(folder: &str, film: Option<&Film>) -> bool {
    let Ok(listing) = std::fs::read_dir(folder) else { return false };
    let names: Vec<String> = listing.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    if !names.iter().all(|n| n == folders::ROLL_MARKER || leftover(n)) {
        return false;
    }
    if names.iter().any(|n| n == folders::ROLL_MARKER) && !ours(folder, film) {
        return false;
    }
    for twin in names.iter().filter(|n| leftover(n)).map(|n| Path::new(folder).join(n)) {
        let _ = std::fs::remove_file(twin);
    }
    let _ = folders::set_roll(None, folder);
    // Only an empty folder goes: `remove_dir` refuses one that still holds anything.
    std::fs::remove_dir(folder).is_ok()
}

/// What an undo or a redo of a roll says for the `missed` items it could not move, as Swift's `takeBack`.
pub fn shortfall(missed: usize, undoing: bool) -> String {
    if undoing {
        trn(
            "One item could not be put back: its folder moved or a file took its name.",
            "{n} of the items could not be put back: their folder moved or a file took their name.",
            missed as u64,
        )
    } else {
        trn(
            "One item could not be moved into the roll again: its folder moved or a file took its name.",
            "{n} of the items could not be moved into the roll again: their folder moved or a file took their name.",
            missed as u64,
        )
    }
}

/// Exists for the path itself, a link that leads nowhere included.
fn there(path: &str) -> bool {
    std::fs::symlink_metadata(Path::new(path)).is_ok()
}

#[cfg(test)]
mod tests;
