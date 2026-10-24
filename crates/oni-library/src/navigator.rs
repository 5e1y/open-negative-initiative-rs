//! The navigator's walk of the disk, folders only and one level at a time, and where it stands:
//! the base (`None` is Computer), the unfolded rows, the favourites view and the picked row.

use crate::paths::{self, holds, same};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

mod names;
mod probe;
pub use names::{split_end, trails};

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

/// The person's home folder as the system names it: on Windows `USERPROFILE`, as a `HOME` that Git
/// Bash or MSYS2 sets reads `/c/Users/a`, which no Windows call takes.
pub fn home_folder() -> std::path::PathBuf {
    let (first, second) = if cfg!(windows) { ("USERPROFILE", "HOME") } else { ("HOME", "USERPROFILE") };
    std::env::var_os(first).filter(|h| !h.is_empty()).or_else(|| std::env::var_os(second)).unwrap_or_default().into()
}

impl Computer {
    /// This machine's: `NSHomeDirectory()` resolved, and `/Volumes`, or the drives on Windows.
    pub fn system() -> Computer {
        let home = home_folder();
        let volumes = if cfg!(windows) { DRIVES } else { "/Volumes" };
        Computer { home: paths::canonical(Path::new(&home)), volumes: volumes.into() }
    }

    /// No disk at all: what a check gets unless it hands one, so no test reads the person's folders.
    pub fn none() -> Computer {
        Computer { home: String::new(), volumes: String::new() }
    }

    /// The home folder, then every volume but the boot one, in the Finder's order. Reads the disk,
    /// each drive or mount on a thread of its own: one that does not answer is left out until it does.
    pub fn computer(&self) -> Vec<String> {
        let home = Some(self.home.clone()).filter(|h| !h.is_empty());
        let volumes = if self.volumes == DRIVES { probe::drives() } else { probe::mounted(&self.volumes) };
        home.into_iter().chain(volumes).collect()
    }

    pub fn is_volume_root(&self, path: &str) -> bool {
        if self.volumes == DRIVES {
            return is_drive_root(path);
        }
        paths::parent(path) == self.volumes
    }

    /// On a disk that may come and go: under `/Volumes`, or on Windows a drive other than the home's;
    /// never with no volumes' folder known, which would hold every path.
    pub fn is_on_volume(&self, path: &str) -> bool {
        if self.volumes.is_empty() {
            return false;
        }
        if self.volumes == DRIVES {
            let drive = |p: &str| p.get(..2).map(str::to_ascii_uppercase);
            return path.as_bytes().get(1) == Some(&b':') && drive(path) != drive(&self.home);
        }
        path.strip_prefix(self.volumes.as_str()).is_some_and(|rest| rest.starts_with('/'))
    }

    /// One level up; `None` is Computer.
    pub fn parent(&self, base: &str) -> Option<String> {
        let up = paths::parent(base);
        if same(base, &self.home) || self.is_volume_root(base) || up == "/" || up == self.volumes || up.is_empty() || up == base {
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
    out.sort_by(|a, b| paths::standard_order(&paths::name(a), &paths::name(b)));
    out.dedup();
    Some(out)
}

/// The folder a symbolic link, a Windows junction or a Finder alias leads to, by its real path;
/// never one holding `folder`, which would list itself inside itself.
fn linked(folder: &str, path: &Path, kind: &std::fs::FileType, name: &str) -> Option<String> {
    let target = if kind.is_symlink() {
        path.to_path_buf()
    } else if kind.is_file() && !crate::library::is_importable(path) && !name.ends_with(".json") {
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

/// `Navigator.title`: Computer for none, else the folder's name as the system words it.
pub fn title(path: Option<&str>, computer: &Computer) -> String {
    match path {
        None => oni_i18n::tr("Computer").into(),
        Some(path) => computer.name(path),
    }
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
    /// The remembered base and rows in the archive's spelling, which an earlier build kept as the disk listed them.
    pub fn new(base: Option<String>, favourites_only: bool, unfolded: BTreeSet<String>, volumes: Vec<String>) -> Navigation {
        let base = base.map(|b| paths::normal(&b).into_owned());
        let unfolded = unfolded.iter().map(|p| paths::normal(p).into_owned()).collect();
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
        self.walk(&top, 0, &mut out, &mut Vec::new());
        out
    }

    /// `chain` holds the rows above: a link back to one of them shows, and never unfolds again.
    fn walk(&mut self, folders: &[String], depth: usize, out: &mut Vec<(String, usize)>, chain: &mut Vec<String>) {
        for path in folders {
            out.push((path.clone(), depth));
            if self.unfolded.contains(path) && !chain.iter().any(|c| same(c, path)) {
                self.list(path, false);
                let children = self.listings.get(path).cloned().unwrap_or_default();
                chain.push(path.clone());
                self.walk(&children, depth + 1, out, chain);
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
        let moved = |path: &str| paths::moved(path, old, new);
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
        } else if let Some(base) = self.base.clone().filter(|b| !same(b, folder) && holds(b, folder)) {
            self.unfold(&base, folder, false);
        } else {
            self.go(computer.parent(folder));
        }
        self.selected = Some(folder.to_owned());
    }

    fn unfold(&mut self, top: &str, folder: &str, including: bool) {
        let mut at = paths::parent(folder);
        while !same(&at, top) && holds(top, &at) {
            self.unfolded.insert(at.clone());
            self.list(&at, false);
            let up = paths::parent(&at);
            if up == at {
                break;
            }
            at = up;
        }
        if including && !same(top, folder) {
            self.unfolded.insert(top.to_owned());
            self.list(top, false);
        }
    }

    /// A folder opened from elsewhere: picked where it shows, else the base moved to its parent.
    pub fn reveal(&mut self, folder: &str, computer: &Computer) {
        if !self.favourites_only
            && let Some((row, _)) = self.rows(&[]).into_iter().find(|(p, _)| same(p, folder))
        {
            self.selected = Some(row);
            return;
        }
        self.go(computer.parent(folder));
        self.selected = Some(folder.to_owned());
    }
}

#[cfg(test)]
mod tests;
