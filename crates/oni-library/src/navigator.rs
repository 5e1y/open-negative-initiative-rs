//! The navigator's walk of the disk, folders only and one level at a time: the favourites' section,
//! then Computer's (the home, the volumes, a folder opened beyond them), the unfolded rows and the picked one.

use crate::paths::{self, holds, same};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

mod computer;
mod names;
mod probe;
pub use computer::{Computer, DRIVES, home_folder, is_drive_root};
pub use names::{split_end, trails};

/// The ONI Trash's row in `selected` and `unfolded`: never a folder's path, which starts with `/`.
pub const TRASH_ROW: &str = "oni-trash";

/// The sections' heading rows in `rows` and `folded`, never a folder's path.
pub const FAVOURITES_ROW: &str = "oni-favourites";
pub const COMPUTER_ROW: &str = "oni-computer";

/// Whether a row is a section's heading rather than a folder.
pub fn is_heading(row: &str) -> bool {
    row == FAVOURITES_ROW || row == COMPUTER_ROW
}

/// The section a row of `rows` lies in: the heading above it, Computer's when none is.
pub fn heading_above(rows: &[String], i: usize) -> &'static str {
    match rows.iter().take(i + 1).rev().find(|r| is_heading(r)).map(String::as_str) {
        Some(FAVOURITES_ROW) => FAVOURITES_ROW,
        _ => COMPUTER_ROW,
    }
}

/// A folder's subfolders in the Finder's order, hidden ones and packages out, a link by the folder it
/// leads to; `None` while the disk refuses it (asleep, unmounted, denied), never empty. Reads the disk.
pub fn children(folder: &str) -> Option<Vec<String>> {
    crate::presence::looked(folder);
    let listing = std::fs::read_dir(folder).ok()?;
    let mut out = Vec::new();
    for entry in listing.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // The listing's own type (`d_type`) leaves files out with no look each; folders alone are looked at.
        let Ok(kind) = entry.file_type() else { continue };
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        if kind.is_dir() {
            let shown = std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir() && !crate::roots::is_hidden(&path, &m));
            if shown && !crate::roots::is_package(&path) {
                out.push(paths::join(folder, &name));
            }
        } else if let Some(target) = linked(folder, &path, &kind, &name) {
            out.push(target);
        }
    }
    let mut keyed: Vec<(paths::NameKey, String)> = out.into_iter().map(|p| (paths::NameKey::new(&paths::name(&p)), p)).collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out: Vec<String> = keyed.into_iter().map(|(_, p)| p).collect();
    out.dedup();
    Some(out)
}

/// Files beside photos that are never an alias, so a folder of sidecars on a share pays no look each.
const SIDE_FILES: &[&str] = &[".json", ".xmp", ".pp3", ".dop", ".on1", ".txt", ".pdf", ".mov", ".mp4"];

/// The folder a symbolic link, a Windows junction or a Finder alias leads to, by its real path;
/// never one holding `folder`, which would list itself inside itself.
fn linked(folder: &str, path: &Path, kind: &std::fs::FileType, name: &str) -> Option<String> {
    let target = if kind.is_symlink() {
        path.to_path_buf()
    } else if kind.is_file() && !crate::library::is_importable(path) && !SIDE_FILES.iter().any(|e| name.to_ascii_lowercase().ends_with(e)) {
        crate::roots::alias_target(path)?
    } else {
        return None;
    };
    let hidden = std::fs::symlink_metadata(path).is_ok_and(|m| crate::roots::is_hidden(path, &m));
    if hidden || !std::fs::metadata(&target).is_ok_and(|m| m.is_dir()) || crate::roots::is_package(&target) {
        return None;
    }
    let real = paths::canonical(&target);
    (!holds(&real, &paths::canonical(Path::new(folder)))).then_some(real)
}

/// `FolderNavigation`: the sections, the folders unfolded and listed. A listing is asked through
/// `wanted` and lands through `listed`, so the disk is read off the interface thread.
#[derive(Debug, Clone, Default)]
pub struct Navigation {
    /// The sections folded by their heading, `FAVOURITES_ROW` or `COMPUTER_ROW`.
    pub folded: BTreeSet<String>,
    /// Computer's unfolded rows and the ONI Trash's; a favourite's own, apart, so a folder in both
    /// sections unfolds where the hand unfolds it alone.
    pub unfolded: BTreeSet<String>,
    pub unfolded_favourites: BTreeSet<String>,
    pub selected: Option<String>,
    /// The section the pick lies in, so a folder both sections show is picked in one alone.
    pub picked_in: &'static str,
    pub listings: HashMap<String, Vec<String>>,
    pub renaming: Option<String>,
    /// The home folder and the mounted volumes, Computer's rows.
    pub volumes: Vec<String>,
    /// Folders opened beyond the home and the volumes, each laid under them as itself.
    pub beyond: BTreeSet<String>,
    listing: HashSet<String>,
    wanted: Vec<String>,
    /// Folders the disk refused, left unlisted rather than empty until `retry` or a click.
    pub refused: BTreeSet<String>,
}

/// What a landed listing asks of the app: never an open, so no listing lands over a newer one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Landed {
    Nothing,
    /// The listing was held already, so the rows need no new publish.
    Same,
}

impl Navigation {
    /// The remembered sections and rows in the archive's spelling, which an earlier build kept as the disk listed them.
    pub fn new(folded: BTreeSet<String>, unfolded: BTreeSet<String>, unfolded_favourites: BTreeSet<String>, volumes: Vec<String>) -> Navigation {
        let normal = |set: BTreeSet<String>| set.iter().map(|p| paths::normal(p).into_owned()).collect();
        Navigation { folded, unfolded: normal(unfolded), unfolded_favourites: normal(unfolded_favourites), volumes, ..Navigation::default() }
    }

    /// What `remember()` writes: the sections folded, Computer's unfolded rows and the favourites'.
    pub fn remembered(&self) -> (Vec<String>, Vec<String>, Vec<String>) {
        let list = |set: &BTreeSet<String>| set.iter().cloned().collect();
        (list(&self.folded), list(&self.unfolded), list(&self.unfolded_favourites))
    }

    fn unfolded_in(&mut self, heading: &str) -> &mut BTreeSet<String> {
        if heading == FAVOURITES_ROW { &mut self.unfolded_favourites } else { &mut self.unfolded }
    }

    /// Whether `folder` stands unfolded in the section under `heading`.
    pub fn is_unfolded(&self, heading: &str, folder: &str) -> bool {
        if heading == FAVOURITES_ROW { self.unfolded_favourites.contains(folder) } else { self.unfolded.contains(folder) }
    }

    /// The row `folder` stands on: in the pick's section where both sections show it, else the first.
    pub fn row_of(&self, rows: &[String], folder: &str) -> Option<usize> {
        let shown = |i: &usize| rows[*i] == folder;
        (0..rows.len()).find(|i| shown(i) && heading_above(rows, *i) == self.picked_in).or_else(|| (0..rows.len()).find(shown))
    }

    /// The picked row among `rows`.
    pub fn picked_row(&self, rows: &[String]) -> Option<usize> {
        self.row_of(rows, self.selected.as_deref()?)
    }

    /// Row `i` of `rows` picked, in its own section.
    pub fn pick(&mut self, rows: &[String], i: usize) {
        self.selected = rows.get(i).cloned();
        self.picked_in = heading_above(rows, i);
    }

    /// The folders to list, each asked once until it lands.
    pub fn take_wanted(&mut self) -> Vec<String> {
        std::mem::take(&mut self.wanted)
    }

    /// Lists a folder once, or again when `fresh`.
    pub fn list(&mut self, folder: &str, fresh: bool) {
        let known = self.listings.contains_key(folder) || self.refused.contains(folder);
        if (fresh || !known) && self.listing.insert(folder.to_owned()) {
            self.wanted.push(folder.to_owned());
        }
    }

    /// A listing landed: the rows it lays, whatever was clicked or opened while it was read.
    pub fn listed(&mut self, folder: &str, children: Vec<String>) -> Landed {
        self.listing.remove(folder);
        self.refused.remove(folder);
        let same = self.listings.get(folder) == Some(&children);
        self.listings.insert(folder.to_owned(), children);
        if same { Landed::Same } else { Landed::Nothing }
    }

    /// The disk refused the folder: it stays unlisted with its chevron, and a click reads it again.
    pub fn refused(&mut self, folder: &str) -> Landed {
        self.listing.remove(folder);
        let had = self.listings.remove(folder).is_some();
        if self.refused.insert(folder.to_owned()) || had { Landed::Nothing } else { Landed::Same }
    }

    /// The refused folders read again, once the disk may answer.
    pub fn retry(&mut self) {
        for folder in self.refused.clone() {
            self.list(&folder, true);
        }
    }

    /// Computer's rows: the home and the volumes, then the folders opened beyond them.
    pub fn computer_tops(&self) -> Vec<String> {
        self.volumes.iter().chain(self.beyond.iter()).cloned().collect()
    }

    /// The home and the volumes as the disk lists them now, the former returned: a folder beyond them
    /// that one now holds is unfolded down to under it, so a reveal outlives the volumes listed after it.
    pub fn set_volumes(&mut self, volumes: Vec<String>) -> Vec<String> {
        let before = std::mem::replace(&mut self.volumes, volumes);
        let held: Vec<(String, String)> = self.beyond.iter().filter_map(|b| self.volumes.iter().find(|v| holds(v, b)).map(|v| (v.clone(), b.clone()))).collect();
        for (volume, folder) in held {
            self.beyond.remove(&folder);
            self.unfold(COMPUTER_ROW, &volume, &folder);
        }
        before
    }

    /// The rows laid flat with their depth, each unfolded row's children under it: the favourites'
    /// heading and rows while there are any, then Computer's; a folded section keeps its heading alone.
    pub fn rows(&mut self, favourites: &[String]) -> Vec<(String, usize)> {
        let mut out = Vec::new();
        let sections = [(FAVOURITES_ROW, favourites.to_vec()), (COMPUTER_ROW, self.computer_tops())];
        for (heading, tops) in sections {
            if heading == FAVOURITES_ROW && tops.is_empty() {
                continue;
            }
            out.push((heading.to_owned(), 0));
            if !self.folded.contains(heading) {
                self.walk(heading, &tops, 0, &mut out, &mut Vec::new());
            }
        }
        out
    }

    /// A section's heading clicked: its rows fold away, or come back.
    pub fn toggle_section(&mut self, heading: &str) {
        if !self.folded.remove(heading) {
            self.folded.insert(heading.to_owned());
        }
    }

    /// `chain` holds the rows above: a link back to one of them shows, and never unfolds again.
    fn walk(&mut self, heading: &str, folders: &[String], depth: usize, out: &mut Vec<(String, usize)>, chain: &mut Vec<String>) {
        for path in folders {
            out.push((path.clone(), depth));
            if self.is_unfolded(heading, path) && !chain.iter().any(|c| same(c, path)) {
                self.list(path, false);
                let children = self.listings.get(path).cloned().unwrap_or_default();
                chain.push(path.clone());
                self.walk(heading, &children, depth + 1, out, chain);
                chain.pop();
            }
        }
    }

    /// The Finder sidebar's click: the row picked and its folder opened at once, whatever it holds;
    /// a double click's second opens nothing more. A refused folder is read again for its chevron.
    pub fn click(&mut self, folder: &str, count: usize) -> Option<String> {
        self.selected = Some(folder.to_owned());
        if self.refused.contains(folder) {
            self.list(folder, true);
        }
        (count == 1).then(|| folder.to_owned())
    }

    /// A row's chevron in the section under `heading`: the row picked, folded or unfolded there alone.
    pub fn toggle_in(&mut self, heading: &'static str, folder: &str) {
        self.selected = Some(folder.to_owned());
        self.picked_in = heading;
        let set = self.unfolded_in(heading);
        if !set.remove(folder) {
            set.insert(folder.to_owned());
            self.list(folder, true);
        }
    }

    /// A Computer row's chevron.
    pub fn toggle(&mut self, folder: &str) {
        self.toggle_in(COMPUTER_ROW, folder);
    }

    pub fn toggle_trash(&mut self) {
        self.selected = Some(TRASH_ROW.to_owned());
        if !self.unfolded.remove(TRASH_ROW) {
            self.unfolded.insert(TRASH_ROW.to_owned());
        }
    }

    /// A folder renamed or moved: the pick, the unfolded rows and the folders beyond follow it.
    pub fn follow(&mut self, old: &str, new: &str) {
        let moved = |path: &str| paths::moved(path, old, new);
        self.beyond = self.beyond.iter().map(|p| moved(p)).collect();
        self.selected = self.selected.as_deref().map(moved);
        self.unfolded = self.unfolded.iter().map(|p| moved(p)).collect();
        self.unfolded_favourites = self.unfolded_favourites.iter().map(|p| moved(p)).collect();
        self.listings.retain(|key, _| !holds(old, key));
        self.list(&paths::parent(new), true);
    }

    /// A folder made inside another: its parent unfolds in the pick's section, and the new row is
    /// picked and named there.
    pub fn made(&mut self, folder: &str) {
        let parent = paths::parent(folder);
        self.unfolded_in(self.picked_in).insert(parent.clone());
        self.list(&parent, true);
        self.selected = Some(folder.to_owned());
        self.renaming = Some(folder.to_owned());
    }

    /// The open photo's folder, or one opened from elsewhere: its section open and unfolded down to it
    /// under a favourite holding it, else under Computer, where a folder beyond the home and the volumes
    /// comes as itself, so nothing above it is listed.
    pub fn show(&mut self, folder: &str, favourites: &[String]) {
        let favourite = favourites.iter().find(|t| holds(t, folder)).cloned();
        let (heading, top) = match favourite {
            Some(top) => (FAVOURITES_ROW, top),
            None => {
                let held = self.computer_tops().into_iter().find(|t| holds(t, folder));
                let top = held.unwrap_or_else(|| {
                    self.beyond.retain(|b| !holds(folder, b));
                    self.beyond.insert(folder.to_owned());
                    folder.to_owned()
                });
                (COMPUTER_ROW, top)
            }
        };
        self.folded.remove(heading);
        self.unfold(heading, &top, folder);
        self.selected = Some(folder.to_owned());
        self.picked_in = heading;
    }

    /// Every folder from `top` down to the one holding `folder` unfolded in the section under `heading`.
    fn unfold(&mut self, heading: &str, top: &str, folder: &str) {
        let mut at = paths::parent(folder);
        while !same(&at, top) && holds(top, &at) {
            self.unfolded_in(heading).insert(at.clone());
            self.list(&at, false);
            let up = paths::parent(&at);
            if up == at {
                break;
            }
            at = up;
        }
        if !same(top, folder) {
            self.unfolded_in(heading).insert(top.to_owned());
            self.list(top, false);
        }
    }

    /// A folder opened from elsewhere: picked where a row shows it, in the pick's section first, else
    /// shown as `show` lays it.
    pub fn reveal(&mut self, folder: &str, favourites: &[String]) {
        let rows: Vec<String> = self.rows(favourites).into_iter().map(|(p, _)| p).collect();
        let shown = self.row_of(&rows, folder).or_else(|| rows.iter().position(|p| !is_heading(p) && same(p, folder)));
        match shown {
            Some(i) => self.pick(&rows, i),
            None => self.show(folder, favourites),
        }
    }
}

#[cfg(test)]
mod tests;
