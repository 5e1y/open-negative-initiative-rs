//! The folders the archive shows. Every image under a root is a member, found on the disk and
//! never imported; the active root is the one the tree and the gallery lay out.

use crate::navigator::Computer;
use crate::paths::{self, canonical, holds};
use crate::presence::{self, Away, Presence};
use oni_settings::{Date, Film};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
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
    /// Each unreadable root or folder's state, offline or missing, told when the disk refused it.
    pub away: HashMap<String, Presence>,
    /// The roll marker of each folder holding an image, read on the survey's own thread.
    pub rolls: HashMap<String, Film>,
    /// The walk stopped part way: what it did not reach is unknown, never gone.
    pub stopped: bool,
    /// Each image's sidecar's modification date, read off the walk's own look at it.
    pub sidecars: HashMap<String, Date>,
    /// The images whose bytes the cloud keeps away (`relink::in_cloud`), never read to be indexed.
    pub cloud: HashSet<String>,
    /// The images still being written (`settled`): never stamped nor decoded until a later survey.
    pub settling: HashSet<String>,
}

/// How long a file stays unwritten before it is read: a copy in progress writes far more often.
pub const SETTLE: std::time::Duration = std::time::Duration::from_secs(2);
/// The pause before a file written within `SETTLE` is looked at again: a copy grows meanwhile.
const LOOK_AGAIN: std::time::Duration = std::time::Duration::from_millis(50);

/// A file the Finder marks busy while it copies it, its birth set to a date in 1946.
fn busy(metadata: &std::fs::Metadata) -> bool {
    let year_1946 = |c: SystemTime| UNIX_EPOCH.duration_since(c).is_ok_and(|d| (725_846_400..757_382_400).contains(&d.as_secs()));
    cfg!(target_os = "macos") && metadata.created().is_ok_and(year_1946)
}

/// A file busy, or written within `SETTLE`, which `settled` looks at again. A date ahead of the
/// clock, a camera's set wrong, is no copy: it would never rest.
fn maybe_settling(metadata: &std::fs::Metadata, now: SystemTime) -> bool {
    busy(metadata) || metadata.modified().is_ok_and(|m| now.duration_since(m).is_ok_and(|d| d < SETTLE))
}

/// The files still being written: busy, or grown or rewritten since the walk looked at them.
fn settled(out: &mut Survey) {
    if out.settling.is_empty() {
        return;
    }
    std::thread::sleep(LOOK_AGAIN);
    let (sizes, dated) = (&out.sizes, &out.dated);
    out.settling.retain(|image| match std::fs::metadata(image) {
        Ok(m) => busy(&m) || sizes.get(image) != Some(&(m.len() as i64)) || dated.get(image) != date(m.modified()).as_ref(),
        Err(_) => false,
    });
}

impl Survey {
    /// The state of a path the survey covers: its unreadable folder's, else present.
    pub fn presence(&self, path: &str) -> Presence {
        self.unreadable
            .iter()
            .find(|u| holds(u, path))
            .map_or(Presence::Present, |u| self.away.get(u).copied().unwrap_or(Presence::Offline(Away::Disconnected)))
    }
}

mod finder;
pub use finder::{PACKAGE_EXTENSIONS, alias_target, is_package, package_holding};
#[cfg(target_os = "macos")]
pub use finder::macos::make_alias;

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

/// `surveyed` with no volume known, so a path not found on a folder that answers is missing.
pub fn survey(roots: &[String], alive: &dyn Fn() -> bool) -> Survey {
    surveyed(roots, alive, &Computer::none())
}

/// Every root walked, each refusal told offline or missing against `computer`'s volumes, and the
/// rolls of the folders holding images read. Reads the disk.
pub fn surveyed(roots: &[String], alive: &dyn Fn() -> bool, computer: &Computer) -> Survey {
    let mut out = Survey::default();
    for root in roots {
        presence::looked(root);
        // A root the disk refuses, gone, unmounted or denied, lists nothing, which is not empty.
        if let Err(e) = std::fs::read_dir(root) {
            refuse(&mut out, root.clone(), &e, computer);
            continue;
        }
        // A survey stopped part way still lists every later root the disk refuses.
        let spelt = canonical(Path::new(root));
        let mut links = Links { root: spelt.clone(), followed: Vec::new() };
        out.stopped |= !walk(Path::new(root), &spelt, alive, computer, &mut links, &mut out);
        // A disk that left during the walk refused what it had not listed: none of it is gone.
        if let Err(e) = std::fs::read_dir(root) {
            refuse(&mut out, root.clone(), &e, computer);
        }
    }
    settled(&mut out);
    out.rolls = crate::folders::rolls(&out.images);
    out
}

fn refuse(out: &mut Survey, folder: String, error: &std::io::Error, computer: &Computer) {
    if out.away.contains_key(&folder) {
        return;
    }
    out.away.insert(folder.clone(), presence::refused(&folder, error, computer));
    out.unreadable.push(folder);
}

/// The folders a walk reached through a link, by their real paths, so none is walked twice.
struct Links {
    root: String,
    followed: Vec<String>,
}

impl Links {
    /// A link's folder, walked under the link's name unless it holds the root, lies inside it (walked
    /// by its own path) or inside a folder a link reached already: no loop, no photo twice.
    fn follows(&mut self, link: &Path) -> bool {
        if !std::fs::metadata(link).is_ok_and(|m| m.is_dir()) || is_package(link) {
            return false;
        }
        let real = canonical(link);
        if holds(&real, &self.root) || holds(&self.root, &real) || self.followed.iter().any(|f| holds(f, &real) || holds(&real, f)) {
            return false;
        }
        if whole(&real) {
            return false;
        }
        self.followed.push(real);
        true
    }
}

/// A disk's root, `/` or a folder holding the home: a link to one would walk a whole disk.
fn whole(real: &str) -> bool {
    let home = paths::normal(&crate::navigator::home_folder().to_string_lossy()).into_owned();
    if real == "/" || real.trim_end_matches('/').ends_with(':') || (!home.is_empty() && holds(real, &home)) {
        return true;
    }
    mount_point(real)
}

/// A folder on another device than its parent's: where a volume is mounted.
#[cfg(unix)]
fn mount_point(real: &str) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Some(parent) = Path::new(real).parent() else { return true };
    match (std::fs::metadata(real), std::fs::metadata(parent)) {
        (Ok(here), Ok(up)) => here.dev() != up.dev(),
        _ => false,
    }
}

#[cfg(not(unix))]
fn mount_point(_: &str) -> bool {
    false
}

/// Depth first, names in Finder order, false once `alive` turns false. Each child is its name
/// joined to `spelt`, a link's too, and only a link pays a `realpath` (six times a look, on an SSD).
fn walk(folder: &Path, spelt: &str, alive: &dyn Fn() -> bool, computer: &Computer, links: &mut Links, out: &mut Survey) -> bool {
    presence::looked(spelt);
    let listing = match std::fs::read_dir(folder) {
        Ok(listing) => listing,
        Err(e) => {
            refuse(out, spelt.to_owned(), &e, computer);
            return true;
        }
    };
    let mut children: Vec<PathBuf> = listing.filter_map(|e| e.ok().map(|e| e.path())).collect();
    children.sort_by(|a, b| paths::standard_order(&a.to_string_lossy(), &b.to_string_lossy()).then_with(|| a.cmp(b)));
    for child in children {
        let name = child.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !alive() {
            return false;
        }
        let metadata = match std::fs::symlink_metadata(&child) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            // An entry listed and then refused leaves its folder unread, so no row in it is dropped.
            Err(e) => {
                refuse(out, spelt.to_owned(), &e, computer);
                continue;
            }
        };
        if is_hidden(&child, &metadata) {
            continue;
        }
        // A symbolic link or a Windows junction to a folder is walked as the folder it leads to.
        let linked = metadata.file_type().is_symlink() && links.follows(&child);
        if metadata.is_dir() || linked {
            if !linked && is_package(&child) {
                continue;
            }
            let folder = paths::join(spelt, &name);
            out.folders.push(folder.clone());
            if let Some(made) = date(if linked { std::fs::metadata(&child).and_then(|m| m.created()) } else { metadata.created() }) {
                out.made.insert(folder.clone(), made);
            }
            if !walk(&child, &folder, alive, computer, links, out) {
                return false;
            }
        } else if metadata.is_file() && crate::library::is_importable(&child) {
            let image = paths::join(spelt, &name);
            if crate::relink::in_cloud(&metadata) {
                out.cloud.insert(image.clone());
            }
            if maybe_settling(&metadata, SystemTime::now()) {
                out.settling.insert(image.clone());
            }
            out.images.push(image.clone());
            if let Some(dated) = date(metadata.modified()) {
                out.dated.insert(image.clone(), dated);
            }
            out.sizes.insert(image, metadata.len() as i64);
        } else if metadata.is_file()
            && let Some(original) = name.strip_suffix(".json").filter(|o| crate::library::is_importable(Path::new(o)))
            && let Some(dated) = date(metadata.modified())
        {
            out.sidecars.insert(paths::join(spelt, original), dated);
        }
    }
    true
}

/// The folders of a previous library, offered and never added: per volume, the deepest folder
/// holding them all, or its children holding photos when that folder is a volume or the home.
pub fn suggested(members: &[String], home: &str) -> Vec<String> {
    // A path's parts, a share's `//server/share` standing as one, as a drive does.
    let parts_of = |path: &str| -> Vec<String> {
        let mut parts: Vec<String> = path.split('/').filter(|p| !p.is_empty()).map(str::to_owned).collect();
        if path.starts_with("//") && parts.len() > 1 {
            let share = format!("//{}/{}", parts[0], parts[1]);
            parts.splice(..2, [share]);
        }
        parts
    };
    // A path laid back from its parts: rooted at `/`, or at the drive or the share Windows names first.
    let rebuilt = |parts: &[String]| match parts.first() {
        Some(first) if first.ends_with(':') || first.starts_with("//") => parts.join("/"),
        _ => format!("/{}", parts.join("/")),
    };
    let volume = |path: &str| -> String {
        let parts = parts_of(path);
        if parts.first().map(String::as_str) == Some("Volumes") && parts.len() > 1 {
            return format!("/Volumes/{}", parts[1]);
        }
        if holds(home, path) {
            return home.to_owned();
        }
        match parts.first().filter(|d| d.ends_with(':') || d.starts_with("//")) {
            Some(root) => root.clone(),
            None => "/".to_owned(),
        }
    };
    let folders: BTreeSet<String> = members.iter().map(|m| paths::parent(m)).collect();
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for folder in folders {
        groups.entry(volume(&folder)).or_default().push(folder);
    }
    let mut out = Vec::new();
    for (base, group) in groups {
        let split: Vec<Vec<String>> = group.iter().map(|f| parts_of(f)).collect();
        let mut common = split[0].clone();
        for parts in &split[1..] {
            let shared = common.iter().zip(parts).take_while(|(a, b)| a == b).count();
            common.truncate(shared);
        }
        let ancestor = rebuilt(&common);
        if ancestor != base && ancestor != "/" && !ancestor.ends_with(':') {
            out.push(ancestor);
            continue;
        }
        let depth = common.len() + 1;
        let children: BTreeSet<String> = split
            .iter()
            .filter(|parts| parts.len() >= depth)
            .map(|parts| rebuilt(&parts[..depth]))
            .collect();
        out.extend(children);
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests;
