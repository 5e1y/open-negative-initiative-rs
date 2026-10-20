//! The folders the navigator keeps at hand and the index keeps read, in a file of their own so a
//! build that rewrites `roots.json` never drops them; and the surveys that read them off the disk.

use crate::paths::holds;
use crate::roots::{self, Roots};
use oni_settings::{Date, Film, json};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Favourites {
    pub version: i64,
    pub paths: Vec<String>,
}

impl Default for Favourites {
    fn default() -> Self {
        Favourites { version: 1, paths: Vec::new() }
    }
}

impl Favourites {
    pub fn file_path(support: &Path) -> PathBuf {
        support.join("favourites.json")
    }

    /// `None` while the file was never written, which is what seeds it from the roots once.
    pub fn load(support: &Path) -> Option<Favourites> {
        crate::read_json(&Self::file_path(support))
    }

    /// A plain `JSONEncoder`'s text: compact, keys in declaration order.
    pub fn save(&self, support: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(support)?;
        let node = json::to_node(self).map_err(std::io::Error::other)?;
        crate::write_atomically(&Self::file_path(support), json::compact(&node).as_bytes())
    }

    /// The roots of `roots.json` seed the first favourites; that file is only read.
    pub fn seeded(roots: &Roots) -> Favourites {
        Favourites { version: 1, paths: roots.paths.clone() }
    }

    pub fn contains(&self, path: &str) -> bool {
        self.paths.iter().any(|p| p == path)
    }

    pub fn toggle(&mut self, path: &str) {
        if self.contains(path) {
            self.paths.retain(|p| p != path);
        } else {
            self.paths.push(path.to_owned());
        }
    }

    /// A favourite renamed or moved on the disk follows it, a folder carrying those inside it.
    pub fn follow(&mut self, old: &str, new: &str) {
        for path in &mut self.paths {
            if path == old {
                *path = new.to_owned();
            } else if holds(old, path) {
                *path = format!("{new}{}", &path[old.len()..]);
            }
        }
    }
}

/// One surveyed folder's images and subfolders, kept so a folder opened inside it reads no disk.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Area {
    pub images: Vec<String>,
    pub folders: Vec<String>,
    pub made: HashMap<String, Date>,
    pub rolls: HashMap<String, Film>,
    /// Each image's file date, which orders its waiting tile until its EXIF is read.
    pub dated: HashMap<String, Date>,
    /// Folders the disk refused inside the read: their photos were not seen, so none is forgotten.
    pub unread: Vec<String>,
    /// Each image's size as read, which tells a row stamped off a half-written file.
    pub sizes: HashMap<String, i64>,
}

type Handler = Box<dyn FnOnce() + Send>;

/// Raised once, by a folder opened elsewhere; a read checks it between entries.
#[derive(Default)]
pub struct Stop {
    state: Mutex<(bool, Vec<Handler>)>,
}

impl Stop {
    pub fn new() -> Arc<Stop> {
        Arc::new(Stop::default())
    }

    pub fn raised(&self) -> bool {
        self.state.lock().map(|s| s.0).unwrap_or(true)
    }

    pub fn raise(&self) {
        let handlers = match self.state.lock() {
            Ok(mut state) if !state.0 => {
                state.0 = true;
                std::mem::take(&mut state.1)
            }
            _ => Vec::new(),
        };
        for handler in handlers {
            handler();
        }
    }

    /// Runs `handler` when the stop is raised, at once if it already is.
    pub fn on_raise(&self, handler: Handler) {
        let now = match self.state.lock() {
            Ok(mut state) if !state.0 => {
                state.1.push(handler);
                return;
            }
            _ => true,
        };
        if now {
            handler();
        }
    }
}

/// Each surveyed folder's area; their union is what the index and the tree know.
#[derive(Default)]
pub struct Surveys {
    pub areas: BTreeMap<String, Area>,
    /// Each listed image's file date, the waiting tile's order.
    pub dated: HashMap<String, Date>,
    running: Option<(String, Arc<Stop>)>,
}

impl Surveys {

    pub fn store(&mut self, area: Area, root: &str) {
        // A folder read again inside an area held is laid into it, so one area answers for each photo.
        let outer = self.areas.keys().find(|k| k.as_str() != root && holds(k, root)).cloned();
        if let Some(outer) = outer
            && let Some(whole) = self.areas.get_mut(&outer)
        {
            let inside = |p: &str| p != root && holds(root, p);
            whole.images.retain(|p| !holds(root, p));
            whole.images.extend(area.images);
            whole.folders.retain(|p| !inside(p));
            whole.folders.extend(area.folders);
            whole.made.retain(|k, _| !inside(k));
            whole.made.extend(area.made);
            whole.rolls.retain(|k, _| !holds(root, k));
            whole.rolls.extend(area.rolls);
            whole.dated.retain(|k, _| !holds(root, k));
            whole.dated.extend(area.dated);
            whole.unread.retain(|p| !holds(root, p));
            whole.unread.extend(area.unread);
            whole.sizes.retain(|k, _| !holds(root, k));
            whole.sizes.extend(area.sizes);
            return;
        }
        // An area inside the one stored would be read twice; the outer one answers for it.
        self.areas.retain(|inner, _| inner == root || !holds(root, inner));
        self.areas.insert(root.to_owned(), area);
    }

    pub fn keep(&mut self, wanted: &[String]) {
        self.areas.retain(|root, _| wanted.iter().any(|w| holds(root, w)));
    }

    /// Every image, folder, date and roll under a wanted folder, the area holding `first` laid
    /// first so the folder on screen is indexed before the rest.
    pub fn union(&self, first: Option<&str>, wanted: &[String]) -> Area {
        let inside = |p: &str| wanted.iter().any(|w| holds(w, p));
        let leads = |a: &str| first.is_some_and(|f| holds(a, f));
        let mut order: Vec<&String> = self.areas.keys().collect();
        order.sort_by(|a, b| leads(b).cmp(&leads(a)).then_with(|| a.cmp(b)));
        let mut seen = HashSet::new();
        let mut out = Area::default();
        for root in order {
            let area = &self.areas[root];
            out.images.extend(area.images.iter().filter(|p| inside(p) && seen.insert((*p).clone())).cloned());
            out.folders.extend(area.folders.iter().filter(|p| inside(p) && seen.insert((*p).clone())).cloned());
            for (k, v) in &area.made {
                out.made.entry(k.clone()).or_insert(*v);
            }
            for (k, v) in &area.dated {
                out.dated.entry(k.clone()).or_insert(*v);
            }
            for (k, v) in &area.rolls {
                out.rolls.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        out
    }

    /// The read running now, so opening a folder it does not cover stops it at once.
    pub fn begin(&mut self, root: &str) -> Arc<Stop> {
        let stop = Stop::new();
        self.running = Some((root.to_owned(), stop.clone()));
        stop
    }

    pub fn end(&mut self) {
        self.running = None;
    }

    /// Stops the running read unless its folder is one of `wanted`: a folder left behind stops
    /// even when it holds the new one, whose own read is far shorter.
    pub fn yield_keeping(&self, wanted: &[String]) {
        if let Some((root, stop)) = &self.running
            && !wanted.contains(root)
        {
            stop.raise();
        }
    }
}

/// The areas to read for `wanted`: each wanted folder no stored area covers, outermost first.
pub fn missing(wanted: &[String], stored: &[String]) -> Vec<String> {
    let outermost = wanted.iter().filter(|p| !wanted.iter().any(|w| w != *p && holds(w, p)));
    let set: std::collections::BTreeSet<String> =
        outermost.filter(|p| !stored.iter().any(|s| holds(s, p))).cloned().collect();
    set.into_iter().collect()
}

/// A pass's reads in their turn: the opened folder alone before any area holding it, then the
/// areas holding it, innermost first, then the rest by name.
pub fn ordered(reading: &[String], opened: Option<&str>) -> Vec<String> {
    let mut out = reading.to_vec();
    if let Some(opened) = opened
        && !out.iter().any(|r| r == opened)
        && out.iter().any(|r| holds(r, opened))
    {
        out.push(opened.to_owned());
    }
    let leads = |a: &str| opened.is_some_and(|o| holds(a, o));
    out.sort_by(|a, b| {
        let (ha, hb) = (leads(a), leads(b));
        if ha != hb {
            hb.cmp(&ha)
        } else if ha {
            b.len().cmp(&a.len())
        } else {
            a.cmp(b)
        }
    });
    out
}

/// Reads one folder's whole tree; `None` once `stop` is raised or when the disk refuses the
/// folder itself, so a read never stands for a folder it did not see.
pub fn read(root: &str, stop: &Stop) -> Option<Area> {
    let found = roots::survey(&[root.to_owned()], &|| !stop.raised());
    if stop.raised() || found.unreadable.iter().any(|u| u == root) {
        return None;
    }
    Some(Area {
        rolls: crate::folders::rolls(&found.images),
        images: found.images,
        folders: found.folders,
        made: found.made,
        dated: found.dated,
        unread: found.unreadable,
        sizes: found.sizes,
    })
}

/// Reads a folder on a thread of its own and answers once it is read or stopped, whichever comes
/// first: a read stuck in the disk finishes alone, and nothing waits for it.
pub fn reading(
    root: &str,
    stop: Arc<Stop>,
    by: impl FnOnce(String, Arc<Stop>) -> Option<Area> + Send + 'static,
) -> Option<Area> {
    let (send, receive) = mpsc::channel::<Option<Area>>();
    let stopped = send.clone();
    stop.on_raise(Box::new(move || {
        let _ = stopped.send(None);
    }));
    let root = root.to_owned();
    let worker_stop = stop.clone();
    std::thread::spawn(move || {
        let _ = send.send(by(root, worker_stop));
    });
    receive.recv().ok().flatten()
}

#[cfg(test)]
mod tests;
