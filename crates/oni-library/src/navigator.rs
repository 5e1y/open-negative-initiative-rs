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

impl Computer {
    /// This machine's: `NSHomeDirectory()` resolved, and `/Volumes`.
    pub fn system() -> Computer {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).unwrap_or_default();
        Computer { home: paths::canonical(Path::new(&home)), volumes: "/Volumes".into() }
    }

    /// No disk at all: what a check gets unless it hands one, so no test reads the person's folders.
    pub fn none() -> Computer {
        Computer { home: String::new(), volumes: String::new() }
    }

    /// The home folder, then every volume but the boot one, in the Finder's order. Reads the disk.
    pub fn computer(&self) -> Vec<String> {
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
        paths::parent(path) == self.volumes
    }

    pub fn is_on_volume(&self, path: &str) -> bool {
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

/// A folder's subfolders in the Finder's order, hidden ones and packages left out. Reads the disk.
pub fn children(folder: &str) -> Vec<String> {
    let Ok(listing) = std::fs::read_dir(folder) else { return Vec::new() };
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
    out
}

/// `Navigator.title`: Computer for none, else the folder's name.
pub fn title(path: Option<&str>) -> String {
    match path {
        None => "Computer".into(),
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
}

/// What a landed listing asks of the app: the folder clicked before it was listed, to open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Landed {
    Open(String),
    Toggled,
    Nothing,
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

    pub fn is_listing(&self, folder: &str) -> bool {
        self.listing.contains(folder) || self.deciding.contains(folder)
    }

    /// Lists a folder once, or again when `fresh`.
    pub fn list(&mut self, folder: &str, fresh: bool) {
        if (fresh || !self.listings.contains_key(folder)) && self.listing.insert(folder.to_owned()) {
            self.wanted.push(folder.to_owned());
        }
    }

    /// A listing landed; a folder clicked while unlisted opens when it holds none, else unfolds.
    pub fn listed(&mut self, folder: &str, children: Vec<String>) -> Landed {
        self.listing.remove(folder);
        let empty = children.is_empty();
        self.listings.insert(folder.to_owned(), children);
        if !self.deciding.remove(folder) {
            return Landed::Nothing;
        }
        if empty {
            Landed::Open(folder.to_owned())
        } else {
            self.toggle(folder);
            Landed::Toggled
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
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn mac() -> Computer {
        Computer { home: "/Users/someone".into(), volumes: "/Volumes".into() }
    }

    #[test]
    fn the_navigator_climbs_one_folder_at_a_time_up_to_computer() {
        let c = mac();
        assert_eq!(c.parent("/Users/someone/Scans/2026").as_deref(), Some("/Users/someone/Scans"));
        assert_eq!((c.parent("/Users/someone"), c.parent("/Volumes/T7")), (None, None));
        assert_eq!(c.parent("/tmp"), None, "the boot volume's top reaches Computer, never `/`");
        // Twin: a folder named like a volume but one level deeper climbs to the volume.
        assert_eq!(c.parent("/Volumes/T7/Scans").as_deref(), Some("/Volumes/T7"));
        assert_eq!(c.ancestors("/Volumes/T7/Scans/2026"), ["/Volumes/T7/Scans/2026", "/Volumes/T7/Scans", "/Volumes/T7"]);
        assert!(c.is_on_volume("/Volumes/T7/a") && !c.is_on_volume("/Volumesque/a") && !c.is_on_volume("/Users/someone"));
    }

    #[test]
    fn computer_lists_the_home_first_then_every_volume_but_the_boot_one() {
        let scratch = Scratch::new("computer");
        let root = scratch.path().to_string_lossy().into_owned();
        let volumes = paths::join(&root, "Volumes");
        for name in ["T7 10", "T7 2", ".hidden"] {
            std::fs::create_dir_all(paths::join(&volumes, name)).unwrap();
        }
        std::fs::create_dir_all(paths::join(&root, "home")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("/", paths::join(&volumes, "Macintosh HD")).unwrap();
        let c = Computer { home: paths::join(&root, "home"), volumes: volumes.clone() };
        let listed = c.computer();
        assert_eq!(listed, [c.home.clone(), paths::join(&volumes, "T7 2"), paths::join(&volumes, "T7 10")]);
        // Twin: the boot volume, a link to `/`, is never listed, nor `/` itself.
        assert!(!listed.iter().any(|p| p == "/" || p.ends_with("Macintosh HD")));
    }

    #[test]
    fn a_folder_lists_its_subfolders_alone_in_the_finders_order() {
        let scratch = Scratch::new("navigator");
        for relative in ["Roll 10/a.RW2", "Roll 2/b.RW2", "Roll 1", ".hidden", "Album.photoslibrary"] {
            std::fs::create_dir_all(scratch.path().join(relative)).unwrap();
        }
        std::fs::write(scratch.path().join("loose.RW2"), [0u8]).unwrap();
        let root = scratch.path().to_string_lossy().into_owned();
        let listed: Vec<String> = children(&root).iter().map(|p| paths::name(p)).collect();
        assert_eq!(listed, ["Roll 1", "Roll 2", "Roll 10"]);
        // Twin: an empty folder lists nothing.
        assert!(children(&paths::join(&root, "Roll 1")).is_empty());
        assert_eq!((title(None), title(Some("/Volumes/T7"))), ("Computer".to_string(), "T7".to_string()));
    }

    #[test]
    fn a_row_opens_or_unfolds_as_it_holds() {
        let some = strings(&["/a/b"]);
        let none: Vec<String> = Vec::new();
        assert_eq!(clicked(1, true, Some(&some)), Clicked::Open);
        assert_eq!(clicked(2, true, None), Clicked::Nothing);
        assert_eq!(clicked(1, false, Some(&none)), Clicked::Open);
        assert_eq!((clicked(1, false, Some(&some)), clicked(2, false, Some(&some))), (Clicked::Toggle, Clicked::Open));
        assert_eq!(clicked(1, false, None), Clicked::Read);
        // Twin: one first click, two answers by what the row holds.
        assert_ne!(clicked(1, false, Some(&none)), clicked(1, false, Some(&some)));
    }

    #[test]
    fn an_unlisted_folder_is_read_then_opened_when_bare_and_unfolded_when_not() {
        let mut nav = Navigation::new(Some("/r".into()), false, BTreeSet::new(), vec![]);
        assert_eq!(nav.click("/r/a", 1, false), None);
        assert_eq!(nav.take_wanted(), ["/r/a"]);
        assert_eq!(nav.listed("/r/a", strings(&["/r/a/x"])), Landed::Toggled);
        assert!(nav.unfolded.contains("/r/a"));
        assert_eq!(nav.click("/r/b", 1, false), None);
        nav.take_wanted();
        assert_eq!(nav.listed("/r/b", Vec::new()), Landed::Open("/r/b".into()));
        // Twin: a listing nobody clicked for decides nothing.
        assert_eq!(nav.listed("/r/c", Vec::new()), Landed::Nothing);
        assert!(!nav.unfolded.contains("/r/b") && !nav.unfolded.contains("/r/c"));
        // A second click on an unfolded parent opens it and leaves it unfolded.
        assert_eq!(nav.click("/r/a", 2, false).as_deref(), Some("/r/a"));
        assert!(nav.unfolded.contains("/r/a"));
    }

    #[test]
    fn the_rows_walk_the_unfolded_folders_and_ask_each_listing_once() {
        let mut nav = Navigation::new(None, false, BTreeSet::from(["/h".to_string()]), strings(&["/h", "/Volumes/T7"]));
        assert_eq!(nav.rows(&[]), [("/h".to_string(), 0), ("/Volumes/T7".to_string(), 0)]);
        assert_eq!(nav.take_wanted(), ["/h"]);
        nav.rows(&[]);
        assert!(nav.take_wanted().is_empty(), "a listing under way is asked once");
        nav.listed("/h", strings(&["/h/a", "/h/b"]));
        let rows = nav.rows(&[]);
        assert_eq!(rows.iter().map(|(p, d)| (p.as_str(), *d)).collect::<Vec<_>>(), [("/h", 0), ("/h/a", 1), ("/h/b", 1), ("/Volumes/T7", 0)]);
        // Twin: the favourites view lists the favourites alone, whatever the base.
        nav.show_favourites(true);
        assert_eq!(nav.rows(&strings(&["/f"])), [("/f".to_string(), 0)]);
    }

    #[test]
    fn going_up_moves_the_base_and_a_rename_carries_what_the_navigator_holds() {
        let c = mac();
        let mut nav = Navigation::new(Some("/Users/someone/Scans".into()), false, BTreeSet::new(), vec![]);
        assert!(nav.can_go_up());
        nav.go_up(&c);
        assert_eq!(nav.base.as_deref(), Some("/Users/someone"));
        nav.go_up(&c);
        assert_eq!(nav.base, None);
        assert!(!nav.can_go_up());
        // In the favourites view, Parent climbs from the picked favourite.
        nav.show_favourites(true);
        assert!(!nav.can_go_up());
        nav.selected = Some("/Volumes/T7/Film".into());
        nav.go_up(&c);
        assert_eq!((nav.base.as_deref(), nav.favourites_only), (Some("/Volumes/T7"), false));

        let mut moved = Navigation::new(Some("/r/trip".into()), false, BTreeSet::from(["/r/trip/day2".to_string(), "/r/tripod".to_string()]), vec![]);
        moved.selected = Some("/r/trip/day2".into());
        moved.listings.insert("/r/trip".into(), vec![]);
        moved.listings.insert("/r/tripod".into(), vec![]);
        moved.follow("/r/trip", "/s/trip");
        assert_eq!(moved.base.as_deref(), Some("/s/trip"));
        assert_eq!(moved.selected.as_deref(), Some("/s/trip/day2"));
        // Twin: a sibling whose name starts alike keeps its row and its listing.
        assert_eq!(moved.unfolded, BTreeSet::from(["/s/trip/day2".to_string(), "/r/tripod".to_string()]));
        assert!(moved.listings.contains_key("/r/tripod") && !moved.listings.contains_key("/r/trip"));
    }

    #[test]
    fn the_open_photos_folder_is_unfolded_down_to_or_the_base_moves() {
        let c = mac();
        let mut nav = Navigation::new(None, false, BTreeSet::new(), strings(&["/Users/someone", "/Volumes/T7"]));
        nav.show("/Users/someone/Scans/2026", &[], &c);
        assert_eq!(nav.unfolded, BTreeSet::from(["/Users/someone".to_string(), "/Users/someone/Scans".to_string()]));
        assert_eq!((nav.base.as_deref(), nav.selected.as_deref()), (None, Some("/Users/someone/Scans/2026")));
        // Under a base, the folders between it and the photo's unfold; the base itself is the list.
        let mut based = Navigation::new(Some("/Volumes/T7".into()), false, BTreeSet::new(), vec![]);
        based.show("/Volumes/T7/Film/Roll 1", &[], &c);
        assert_eq!(based.unfolded, BTreeSet::from(["/Volumes/T7/Film".to_string()]));
        // Twin: a folder outside the base moves the base to its parent.
        based.show("/Users/someone/Scans/2026", &[], &c);
        assert_eq!(based.base.as_deref(), Some("/Users/someone/Scans"));
        // A folder made inside another is picked and named, its parent unfolded.
        based.made("/Users/someone/Scans/New Folder");
        assert_eq!(based.renaming.as_deref(), Some("/Users/someone/Scans/New Folder"));
        assert!(based.unfolded.contains("/Users/someone/Scans"));
    }

    #[test]
    fn a_folder_opened_elsewhere_is_picked_where_it_shows_else_the_base_moves() {
        let c = mac();
        let mut nav = Navigation::new(Some("/Users/someone".into()), false, BTreeSet::new(), vec![]);
        nav.listings.insert("/Users/someone".into(), strings(&["/Users/someone/Scans"]));
        nav.reveal("/Users/someone/Scans", &c);
        assert_eq!((nav.base.as_deref(), nav.selected.as_deref()), (Some("/Users/someone"), Some("/Users/someone/Scans")));
        // Twin: one not on screen moves the base to its parent.
        nav.reveal("/Volumes/T7/Film", &c);
        assert_eq!((nav.base.as_deref(), nav.selected.as_deref()), (Some("/Volumes/T7"), Some("/Volumes/T7/Film")));
        nav.toggle_trash();
        assert_eq!(nav.selected.as_deref(), Some(TRASH_ROW));
        assert!(nav.unfolded.contains(TRASH_ROW));
    }
}
