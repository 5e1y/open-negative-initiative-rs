//! The folders the archive shows. Every image under a root is a member, found on the disk and
//! never imported; the active root is the one the tree and the gallery lay out.

use crate::paths::{self, canonical, holds};
use oni_settings::Date;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Roots {
    pub version: i64,
    pub paths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub active: Option<String>,
}

impl Default for Roots {
    fn default() -> Self {
        Roots { version: 1, paths: Vec::new(), active: None }
    }
}

impl Roots {
    /// The root on screen: the one picked, else the first added.
    pub fn current(&self) -> Option<&str> {
        match &self.active {
            Some(active) if self.paths.contains(active) => Some(active),
            _ => self.paths.first().map(String::as_str),
        }
    }

    pub fn file_path(support: &Path) -> PathBuf {
        support.join("roots.json")
    }

    pub fn load(support: &Path) -> Roots {
        std::fs::read(Self::file_path(support))
            .ok()
            .and_then(|data| serde_json::from_slice(&data).ok())
            .unwrap_or_default()
    }

    /// Adds a folder and makes it the one on screen. A folder inside a root only activates that
    /// root, and a folder holding roots takes their place, so no photo is ever a member twice.
    pub fn add(&mut self, path: &str) {
        if let Some(holder) = self.paths.iter().find(|root| holds(root, path)) {
            self.active = Some(holder.clone());
            return;
        }
        self.paths.retain(|root| !holds(path, root));
        self.paths.push(path.to_owned());
        self.active = Some(path.to_owned());
    }
}

/// Every image under the roots, sidecars and other files left out. Reads the disk.
pub fn scan(roots: &[String]) -> Vec<String> {
    survey(roots, &|| true).images
}

/// The images and every folder under the roots, empty ones included, so a folder made in the app
/// shows before anything is put in it; `unreadable` names a root or folder the disk refused.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Survey {
    pub images: Vec<String>,
    pub folders: Vec<String>,
    pub made: HashMap<String, Date>,
    pub dated: HashMap<String, Date>,
    pub unreadable: Vec<String>,
    pub sizes: HashMap<String, i64>,
}

/// Directories the Finder shows as one file: their contents belong to their app, not the archive.
pub const PACKAGE_EXTENSIONS: &[&str] = &[
    "app",
    "bundle",
    "framework",
    "plugin",
    "kext",
    "photoslibrary",
    "photolibrary",
    "migratedphotolibrary",
    "aplibrary",
    "fcpbundle",
    "imovielibrary",
    "tvlibrary",
    "musiclibrary",
    "theater",
    "rtfd",
    "pages",
    "numbers",
    "key",
    "xcodeproj",
    "xcworkspace",
    "playground",
    "logicx",
    "band",
    "lrdata",
    "lrlibrary",
];

pub(crate) fn is_package(path: &Path) -> bool {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|e| PACKAGE_EXTENSIONS.contains(&e.as_str()))
}

pub(crate) fn is_hidden(path: &Path, metadata: &std::fs::Metadata) -> bool {
    let dotted = path.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'));
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x2 != 0 {
            return true;
        }
    }
    // `.skipsHiddenFiles` also skips what the Finder hides by its flag: `UF_HIDDEN`, 0x8000.
    #[cfg(target_os = "macos")]
    {
        use std::os::macos::fs::MetadataExt;
        if metadata.st_flags() & 0x8000 != 0 {
            return true;
        }
    }
    let _ = metadata;
    dotted
}

fn date(time: std::io::Result<SystemTime>) -> Option<Date> {
    let time = time.ok()?;
    let seconds = match time.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs_f64(),
        Err(e) => -e.duration().as_secs_f64(),
    };
    Some(Date::from_unix_seconds(seconds))
}

pub fn survey(roots: &[String], alive: &dyn Fn() -> bool) -> Survey {
    let mut out = Survey::default();
    for root in roots {
        // A root the disk refuses, gone, unmounted or denied, lists nothing, which is not empty.
        if std::fs::read_dir(root).is_err() {
            out.unreadable.push(root.clone());
            continue;
        }
        // A survey stopped part way still lists every later root the disk refuses.
        walk(Path::new(root), alive, &mut out);
    }
    out
}

/// Depth first, a folder before its contents, names in Finder order; false once `alive` turns false.
fn walk(folder: &Path, alive: &dyn Fn() -> bool, out: &mut Survey) -> bool {
    let Ok(listing) = std::fs::read_dir(folder) else {
        out.unreadable.push(canonical(folder));
        return true;
    };
    let mut children: Vec<PathBuf> = listing.filter_map(|e| e.ok().map(|e| e.path())).collect();
    children.sort_by(|a, b| paths::standard_order(&a.to_string_lossy(), &b.to_string_lossy()).then_with(|| a.cmp(b)));
    for child in children {
        if !alive() {
            return false;
        }
        let Ok(metadata) = std::fs::symlink_metadata(&child) else { continue };
        if is_hidden(&child, &metadata) {
            continue;
        }
        if metadata.is_dir() {
            if is_package(&child) {
                continue;
            }
            let folder = canonical(&child);
            out.folders.push(folder.clone());
            if let Some(made) = date(metadata.created()) {
                out.made.insert(folder, made);
            }
            if !walk(&child, alive, out) {
                return false;
            }
        } else if metadata.is_file() && crate::library::is_importable(&child) {
            let image = canonical(&child);
            out.images.push(image.clone());
            if let Some(dated) = date(metadata.modified()) {
                out.dated.insert(image.clone(), dated);
            }
            out.sizes.insert(image, metadata.len() as i64);
        }
    }
    true
}

/// The folders of a previous library, offered and never added: per volume, the deepest folder
/// holding them all, or its children holding photos when that folder is a volume or the home.
pub fn suggested(members: &[String], home: &str) -> Vec<String> {
    let volume = |path: &str| -> String {
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        if parts.first() == Some(&"Volumes") && parts.len() > 1 {
            return format!("/Volumes/{}", parts[1]);
        }
        if holds(home, path) { home.to_owned() } else { "/".to_owned() }
    };
    let folders: BTreeSet<String> = members.iter().map(|m| paths::parent(m)).collect();
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for folder in folders {
        groups.entry(volume(&folder)).or_default().push(folder);
    }
    let mut out = Vec::new();
    for (base, group) in groups {
        let split: Vec<Vec<String>> =
            group.iter().map(|f| f.split('/').filter(|p| !p.is_empty()).map(str::to_owned).collect()).collect();
        let mut common = split[0].clone();
        for parts in &split[1..] {
            let shared = common.iter().zip(parts).take_while(|(a, b)| a == b).count();
            common.truncate(shared);
        }
        let ancestor = format!("/{}", common.join("/"));
        if ancestor != base && ancestor != "/" {
            out.push(ancestor);
            continue;
        }
        let depth = common.len() + 1;
        let children: BTreeSet<String> = split
            .iter()
            .filter(|parts| parts.len() >= depth)
            .map(|parts| format!("/{}", parts[..depth].join("/")))
            .collect();
        out.extend(children);
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    #[test]
    fn a_scan_keeps_images_and_leaves_sidecars_hidden_folders_and_packages_out() {
        let scratch = Scratch::new("roots");
        for relative in [
            "a.RW2",
            "a.RW2.json",
            "Roll 1/b.tif",
            "Roll 1/Deep/c.jpg",
            ".hidden/d.RW2",
            "e.txt",
            "Album.photoslibrary/f.jpg",
            ".oni-roll.json",
        ] {
            let path = scratch.path().join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, [0u8]).unwrap();
        }
        let root = scratch.path().to_string_lossy().into_owned();
        let scanned: BTreeSet<String> =
            scan(std::slice::from_ref(&root)).into_iter().map(|p| p[root.len() + 1..].to_owned()).collect();
        assert_eq!(scanned, BTreeSet::from(["a.RW2", "Roll 1/b.tif", "Roll 1/Deep/c.jpg"].map(String::from)));
        // Twin: a root one folder down finds its two images alone.
        assert_eq!(scan(&[paths::join(&root, "Roll 1")]).len(), 2);
        let surveyed = survey(std::slice::from_ref(&root), &|| true);
        assert_eq!(surveyed.folders.len(), 2);
        assert_eq!(surveyed.sizes.values().copied().collect::<Vec<_>>(), [1, 1, 1]);
        // Twin: a root that is not there is unreadable, never an empty folder.
        let gone = paths::join(&root, "gone");
        assert_eq!(survey(std::slice::from_ref(&gone), &|| true).unreadable, [gone]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_folder_the_finder_hides_by_its_flag_is_left_out() {
        let scratch = Scratch::new("roots-flagged");
        for relative in ["Recovery/a.RW2", "Film/b.RW2"] {
            let path = scratch.path().join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, [0u8]).unwrap();
        }
        let root = scratch.path().to_string_lossy().into_owned();
        assert_eq!(scan(std::slice::from_ref(&root)).len(), 2, "twin: unflagged, both folders count");
        let flagged = std::process::Command::new("/usr/bin/chflags").arg("hidden").arg(scratch.path().join("Recovery")).status();
        assert!(flagged.is_ok_and(|s| s.success()));
        assert_eq!(scan(std::slice::from_ref(&root)), [paths::join(&root, "Film/b.RW2")]);
        assert_eq!(crate::navigator::children(&root), Some(vec![paths::join(&root, "Film")]));
    }

    #[test]
    fn a_stopped_survey_still_names_every_root_the_disk_refuses() {
        let scratch = Scratch::new("roots-stopped");
        std::fs::write(scratch.path().join("a.RW2"), [0u8]).unwrap();
        let here = scratch.path().to_string_lossy().into_owned();
        let gone = paths::join(&here, "gone");
        let both = [here.clone(), gone.clone()];
        let stopped = survey(&both, &|| false);
        assert_eq!((stopped.images.len(), stopped.unreadable), (0, vec![gone.clone()]));
        // Twin: left running, the same survey also lists the photo.
        let running = survey(&both, &|| true);
        assert_eq!((running.images.len(), running.unreadable), (1, vec![gone]));
    }

    #[test]
    fn roots_never_hold_a_photo_twice() {
        let mut roots = Roots::default();
        roots.add("/r/Roll 1");
        roots.add("/r");
        assert_eq!((roots.paths.clone(), roots.current()), (vec!["/r".to_owned()], Some("/r")));
        roots.add("/r/Roll 1/Deep");
        assert_eq!(roots.paths, ["/r"]);
        // Twin: a sibling sharing a prefix is a root of its own.
        roots.add("/ro");
        assert_eq!(roots.paths, ["/r", "/ro"]);
        assert_eq!(roots.current(), Some("/ro"));
    }

    #[test]
    fn a_former_library_is_offered_per_volume_never_the_home_itself() {
        let home = "/Users/a";
        let proposed = suggested(
            &[
                "/Users/a/Pictures/Scans/2025/a.RW2",
                "/Users/a/Pictures/Scans/2026/Roll/b.RW2",
                "/Volumes/NAS/Film/c.tif",
                "/Users/a/Downloads/d.jpg",
            ]
            .map(String::from),
            home,
        );
        assert_eq!(proposed, ["/Users/a/Downloads", "/Users/a/Pictures", "/Volumes/NAS/Film"]);
        // Twin: photos under one folder of the home offer that folder.
        assert_eq!(
            suggested(
                &["/Users/a/Pictures/Scans/2025/a.RW2", "/Users/a/Pictures/Scans/2026/b.RW2"].map(String::from),
                home
            ),
            ["/Users/a/Pictures/Scans"]
        );
    }
}
