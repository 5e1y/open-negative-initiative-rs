//! The navigator's walk of the disk, folders only and one level at a time, and where it stands:
//! the base (`None` is Computer), the unfolded rows, the favourites view and the picked row.

use crate::paths::{self, holds};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

/// The ONI Trash's row in `selected` and `unfolded`: never a folder's path, which starts with `/`.
pub const TRASH_ROW: &str = "oni-trash";

/// The home folder and the folder volumes are mounted in, so a check walks a disk of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Computer {
    pub home: String,
    pub volumes: String,
}

/// `volumes` on Windows, which mounts no disk in one folder: each drive's root is a volume.
pub const DRIVES: &str = "*";

/// Whether `path` names a drive's root, `C:\` or `C:/`.
pub fn is_drive_root(path: &str) -> bool {
    let b = path.as_bytes();
    b.len() == 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
}

impl Computer {
    /// This machine's: `NSHomeDirectory()` resolved, and `/Volumes`, or the drives on Windows.
    pub fn system() -> Computer {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).unwrap_or_default();
        let volumes = if cfg!(windows) { DRIVES } else { "/Volumes" };
        Computer { home: paths::canonical(Path::new(&home)), volumes: volumes.into() }
    }

    /// No disk at all: what a check gets unless it hands one, so no test reads the person's folders.
    pub fn none() -> Computer {
        Computer { home: String::new(), volumes: String::new() }
    }

    /// The home folder, then every volume but the boot one, in the Finder's order. Reads the disk.
    pub fn computer(&self) -> Vec<String> {
        if self.volumes == DRIVES {
            let drives = (b'A'..=b'Z').map(|l| format!("{}:/", l as char)).filter(|d| Path::new(d).is_dir());
            return Some(self.home.clone()).filter(|h| !h.is_empty()).into_iter().chain(drives).collect();
        }
        let mut mounted: Vec<String> = std::fs::read_dir(&self.volumes)
            .map(|listing| {
                listing
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
                    .map(|p| paths::canonical(&p))
                    .filter(|p| p != "/" && self.is_volume_root(p))
                    .collect()
            })
            .unwrap_or_default();
        mounted.sort_by(|a, b| paths::standard_order(a, b));
        mounted.dedup();
        let mut out: Vec<String> = Some(self.home.clone()).filter(|h| !h.is_empty()).into_iter().collect();
        out.extend(mounted);
        out
    }

    pub fn is_volume_root(&self, path: &str) -> bool {
        if self.volumes == DRIVES {
            return is_drive_root(path);
        }
        paths::parent(path) == self.volumes
    }

    /// On a disk that may come and go: under `/Volumes`, or on Windows a drive other than the home's.
    pub fn is_on_volume(&self, path: &str) -> bool {
        if self.volumes == DRIVES {
            let drive = |p: &str| p.get(..2).map(str::to_ascii_uppercase);
            return path.as_bytes().get(1) == Some(&b':') && drive(path) != drive(&self.home);
        }
        path.strip_prefix(self.volumes.as_str()).is_some_and(|rest| rest.starts_with('/'))
    }

    /// One level up; `None` is Computer.
    pub fn parent(&self, base: &str) -> Option<String> {
        let up = paths::parent(base);
        if base == self.home || self.is_volume_root(base) || up == "/" || up == self.volumes || up.is_empty() || up == base {
            return None;
        }
        Some(up)
    }

    /// The base, then each folder above it, nearest first, up to the home or the volume.
    pub fn ancestors(&self, base: &str) -> Vec<String> {
        let mut out = vec![base.to_owned()];
        let mut at = base.to_owned();
        while let Some(up) = self.parent(&at) {
            out.push(up.clone());
            at = up;
        }
        out
    }
}

/// A folder's subfolders in the Finder's order, hidden ones and packages left out; `None` while the
/// disk refuses it (asleep, unmounted, access not granted yet), which is not empty. Reads the disk.
pub fn children(folder: &str) -> Option<Vec<String>> {
    let listing = std::fs::read_dir(folder).ok()?;
    let mut out: Vec<String> = listing
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            let dotted = e.file_name().to_string_lossy().starts_with('.');
            let directory = std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir() && !crate::roots::is_hidden(&path, &m));
            !dotted && directory && !crate::roots::is_package(&path)
        })
        .map(|e| paths::join(folder, &e.file_name().to_string_lossy()))
        .collect();
    out.sort_by(|a, b| paths::standard_order(&paths::name(a), &paths::name(b)));
    Some(out)
}

/// `Navigator.title`: Computer for none, else the folder's name.
pub fn title(path: Option<&str>) -> String {
    match path {
        None => oni_i18n::tr("Computer").into(),
        Some(path) => paths::name(path),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clicked {
    Open,
    Toggle,
    Read,
    Nothing,
}

/// A roll or a folder holding none opens on the first click; one holding others unfolds, and opens
/// on the second. `Read` while its children are not listed yet: they decide.
pub fn clicked(count: usize, roll: bool, children: Option<&[String]>) -> Clicked {
    if roll || children.is_some_and(<[String]>::is_empty) {
        return if count == 1 { Clicked::Open } else { Clicked::Nothing };
    }
    if count >= 2 {
        return Clicked::Open;
    }
    if children.is_none() { Clicked::Read } else { Clicked::Toggle }
}

/// `FolderNavigation`: where the navigator stands and the folders listed. A listing is asked
/// through `wanted` and lands through `listed`, so the disk is read off the interface thread.
#[derive(Debug, Clone, Default)]
pub struct Navigation {
    pub base: Option<String>,
    pub favourites_only: bool,
    pub unfolded: BTreeSet<String>,
    pub selected: Option<String>,
    pub listings: HashMap<String, Vec<String>>,
    pub renaming: Option<String>,
    /// The home folder and the mounted volumes, Computer's rows.
    pub volumes: Vec<String>,
    listing: HashSet<String>,
    wanted: Vec<String>,
    /// Folders clicked before their children were listed, which open or unfold once they are.
    deciding: HashSet<String>,
    /// Folders the disk refused, left unlisted rather than empty until `retry` or a click.
    pub refused: BTreeSet<String>,
}

/// What a landed listing asks of the app: the folder clicked before it was listed, to open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Landed {
    Open(String),
    Toggled,
    Nothing,
    /// The listing was held already, so the rows need no new publish.
    Same,
}

impl Navigation {
    pub fn new(base: Option<String>, favourites_only: bool, unfolded: BTreeSet<String>, volumes: Vec<String>) -> Navigation {
        Navigation { base, favourites_only, unfolded, volumes, ..Navigation::default() }
    }

    /// What `remember()` writes: the base, the favourites view and the unfolded rows.
    pub fn remembered(&self) -> (Option<String>, bool, Vec<String>) {
        (self.base.clone(), self.favourites_only, self.unfolded.iter().cloned().collect())
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

    /// A listing landed; a folder clicked while unlisted opens when it holds none, else unfolds.
    pub fn listed(&mut self, folder: &str, children: Vec<String>) -> Landed {
        self.listing.remove(folder);
        self.refused.remove(folder);
        let empty = children.is_empty();
        let same = self.listings.get(folder) == Some(&children);
        self.listings.insert(folder.to_owned(), children);
        if !self.deciding.remove(folder) {
            return if same { Landed::Same } else { Landed::Nothing };
        }
        if empty {
            Landed::Open(folder.to_owned())
        } else {
            self.toggle(folder);
            Landed::Toggled
        }
    }

    /// The disk refused the folder: it stays unlisted with its chevron, and a click reads it again.
    pub fn refused(&mut self, folder: &str) -> Landed {
        self.listing.remove(folder);
        self.deciding.remove(folder);
        let had = self.listings.remove(folder).is_some();
        if self.refused.insert(folder.to_owned()) || had { Landed::Nothing } else { Landed::Same }
    }

    /// The refused folders read again, once the disk may answer.
    pub fn retry(&mut self) {
        for folder in self.refused.clone() {
            self.list(&folder, true);
        }
    }

    pub fn top(&mut self, favourites: &[String]) -> Vec<String> {
        if self.favourites_only {
            return favourites.to_vec();
        }
        let Some(base) = self.base.clone() else { return self.volumes.clone() };
        self.list(&base, false);
        self.listings.get(&base).cloned().unwrap_or_default()
    }

    /// The rows laid flat with their depth, each unfolded row's children under it.
    pub fn rows(&mut self, favourites: &[String]) -> Vec<(String, usize)> {
        let top = self.top(favourites);
        let mut out = Vec::new();
        self.walk(&top, 0, &mut out);
        out
    }

    fn walk(&mut self, folders: &[String], depth: usize, out: &mut Vec<(String, usize)>) {
        for path in folders {
            out.push((path.clone(), depth));
            if self.unfolded.contains(path) {
                self.list(path, false);
                let children = self.listings.get(path).cloned().unwrap_or_default();
                self.walk(&children, depth + 1, out);
            }
        }
    }

    /// A click on a folder's row: picked, then opened, unfolded, or read first. `Some` to open.
    pub fn click(&mut self, folder: &str, count: usize, roll: bool) -> Option<String> {
        self.selected = Some(folder.to_owned());
        match clicked(count, roll, self.listings.get(folder).map(Vec::as_slice)) {
            Clicked::Open => Some(folder.to_owned()),
            Clicked::Toggle => {
                self.toggle(folder);
                None
            }
            Clicked::Nothing => None,
            Clicked::Read => {
                if self.deciding.insert(folder.to_owned()) && !self.listing.contains(folder) {
                    self.wanted.push(folder.to_owned());
                }
                None
            }
        }
    }

    pub fn toggle(&mut self, folder: &str) {
        self.selected = Some(folder.to_owned());
        if !self.unfolded.remove(folder) {
            self.unfolded.insert(folder.to_owned());
            self.list(folder, true);
        }
    }

    pub fn toggle_trash(&mut self) {
        self.selected = Some(TRASH_ROW.to_owned());
        if !self.unfolded.remove(TRASH_ROW) {
            self.unfolded.insert(TRASH_ROW.to_owned());
        }
    }

    pub fn go(&mut self, folder: Option<String>) {
        self.base = folder.clone();
        self.favourites_only = false;
        if let Some(folder) = folder {
            self.list(&folder, true);
        }
    }

    pub fn go_up(&mut self, computer: &Computer) {
        if self.favourites_only {
            if let Some(selected) = self.selected.clone() {
                self.go(computer.parent(&selected));
            }
            return;
        }
        if let Some(base) = self.base.clone() {
            self.go(computer.parent(&base));
        }
    }

    pub fn can_go_up(&self) -> bool {
        if self.favourites_only { self.selected.is_some() } else { self.base.is_some() }
    }

    pub fn show_favourites(&mut self, on: bool) {
        self.favourites_only = on;
    }

    /// A folder renamed or moved: the base, the pick and the unfolded rows follow it.
    pub fn follow(&mut self, old: &str, new: &str) {
        let moved = |path: &str| -> String {
            if path == old {
                new.to_owned()
            } else if holds(old, path) {
                format!("{new}{}", &path[old.len()..])
            } else {
                path.to_owned()
            }
        };
        self.base = self.base.as_deref().map(moved);
        self.selected = self.selected.as_deref().map(moved);
        self.unfolded = self.unfolded.iter().map(|p| moved(p)).collect();
        self.listings.retain(|key, _| !holds(old, key));
        self.list(&paths::parent(new), true);
    }

    /// A folder made inside another: its parent unfolds, and the new row is picked and named.
    pub fn made(&mut self, folder: &str) {
        let parent = paths::parent(folder);
        self.unfolded.insert(parent.clone());
        self.list(&parent, true);
        self.selected = Some(folder.to_owned());
        self.renaming = Some(folder.to_owned());
    }

    /// The open photo's folder beside it: unfolded down to from a row on screen, else the base moved.
    pub fn show(&mut self, folder: &str, favourites: &[String], computer: &Computer) {
        let tops: Vec<String> = if self.favourites_only {
            favourites.to_vec()
        } else if self.base.is_none() {
            self.volumes.clone()
        } else {
            Vec::new()
        };
        if let Some(top) = tops.iter().find(|t| holds(t, folder)).cloned() {
            self.unfold(&top, folder, true);
        } else if let Some(base) = self.base.clone().filter(|b| b != folder && holds(b, folder)) {
            self.unfold(&base, folder, false);
        } else {
            self.go(computer.parent(folder));
        }
        self.selected = Some(folder.to_owned());
    }

    fn unfold(&mut self, top: &str, folder: &str, including: bool) {
        let mut at = paths::parent(folder);
        while at != top && holds(top, &at) {
            self.unfolded.insert(at.clone());
            self.list(&at, false);
            let up = paths::parent(&at);
            if up == at {
                break;
            }
            at = up;
        }
        if including && top != folder {
            self.unfolded.insert(top.to_owned());
            self.list(top, false);
        }
    }

    /// A folder opened from elsewhere: picked where it shows, else the base moved to its parent.
    pub fn reveal(&mut self, folder: &str, computer: &Computer) {
        if !self.favourites_only && self.rows(&[]).iter().any(|(p, _)| p == folder) {
            self.selected = Some(folder.to_owned());
            return;
        }
        self.go(computer.parent(folder));
        self.selected = Some(folder.to_owned());
    }
}

#[cfg(test)]
mod tests;
