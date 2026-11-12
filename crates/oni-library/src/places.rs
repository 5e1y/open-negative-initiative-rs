//! The favourites and what ONI knows of each one's whereabouts, in one file, `State/places.json`, in
//! the order the person starred them. A star reads no tree and indexes no photo.

use crate::paths::{self, holds, same};
use crate::roots::Roots;
use crate::state::{self, Read};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

mod apart;
mod identity;

pub use apart::{STUCK, read_apart};
pub use identity::{Identify, Lead, Volume, place, system};

/// Moved at every change of the file's shape; a newer one is shown and never written.
pub const VERSION: i64 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Places {
    pub version: i64,
    pub places: Vec<Place>,
}

/// A favourite: its path, the volume it was last seen on and its file id, which find it once moved.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<Volume>,
    #[serde(rename = "fileId", default, skip_serializing_if = "Option::is_none")]
    pub file_id: Option<u64>,
}

impl Place {
    pub fn at(path: &str) -> Place {
        Place { path: path.to_owned(), ..Place::default() }
    }
}

impl Default for Places {
    fn default() -> Self {
        Places { version: VERSION, places: Vec::new() }
    }
}

impl Places {
    pub fn file_path(support: &Path) -> PathBuf {
        state::folder(support).join("places.json")
    }

    /// `None` while the file was never written or could not be read, which seeds it from the roots;
    /// a newer build's file as far as this one reads it, which `save` then refuses to write over.
    pub fn load(support: &Path) -> Option<Places> {
        match state::read::<Places>(&Self::file_path(support), VERSION) {
            Read::Current(places) => Some(places.respelt()),
            Read::Newer(version, read) => Some(Places { version, ..read.unwrap_or_default() }),
            Read::Absent | Read::Unreadable => None,
        }
    }

    pub fn save(&self, support: &Path) -> std::io::Result<()> {
        if self.is_newer() {
            return Err(state::left_newer());
        }
        state::write(&Self::file_path(support), self)
    }

    /// Read from a newer build's file: shown as it is, never changed by this one.
    pub fn is_newer(&self) -> bool {
        self.version > VERSION
    }

    /// The roots of `roots.json` seed the first favourites; that file is only read.
    pub fn seeded(roots: &Roots) -> Places {
        Places { version: VERSION, places: roots.paths.iter().map(|p| Place::at(p)).collect() }.respelt()
    }

    pub fn paths(&self) -> Vec<String> {
        self.places.iter().map(|p| p.path.clone()).collect()
    }

    /// Each path in the archive's spelling, one spelt twice kept once, in their order. Case never
    /// merges two here, as a volume away cannot say whether it tells `Roll` from `roll`.
    pub fn respelt(self) -> Places {
        let mut places: Vec<Place> = Vec::with_capacity(self.places.len());
        for place in self.places {
            let path = paths::normal(&place.path).into_owned();
            if !places.iter().any(|p| p.path == path) {
                places.push(Place { path, ..place });
            }
        }
        Places { places, ..self }
    }

    pub fn contains(&self, path: &str) -> bool {
        self.places.iter().any(|p| same(&p.path, path))
    }

    /// A star off takes its own spelling, else the one a case-only rename outside ONI left behind.
    pub fn toggle(&mut self, path: &str) {
        let spelt = paths::normal(path).into_owned();
        if self.places.iter().any(|p| p.path == spelt) {
            self.places.retain(|p| p.path != spelt);
        } else if self.contains(path) {
            self.places.retain(|p| !same(&p.path, path));
        } else {
            self.places.push(Place::at(&spelt));
        }
    }

    /// A favourite renamed or moved on the disk follows it, a folder carrying those inside it; one
    /// landing on a favourite already there is kept once, in the earlier place.
    pub fn follow(&mut self, old: &str, new: &str) {
        let mut followed: Vec<Place> = Vec::with_capacity(self.places.len());
        for place in &self.places {
            let moved = paths::moved(&place.path, old, new);
            if !followed.iter().any(|p| same(&p.path, &moved)) {
                followed.push(Place { path: moved, ..place.clone() });
            }
        }
        self.places = followed;
    }

    /// The favourites at `folder` or under it taken away, as forgetting a folder gone asks; how many.
    pub fn forget(&mut self, folder: &str) -> usize {
        let before = self.places.len();
        self.places.retain(|p| !holds(folder, &p.path));
        before - self.places.len()
    }

    /// Each place read again by `read` where its folder answers, else kept as known: a disk away, or
    /// another volume now under the same path, never erases the identity known.
    pub fn noted(&self, read: impl Fn(&str) -> Option<Place>) -> Places {
        let uuid = |p: &Place| p.volume.as_ref().and_then(|v| v.uuid.clone());
        let places = self
            .places
            .iter()
            .map(|known| match read(&known.path) {
                Some(now) if uuid(known).is_none() || uuid(known) == uuid(&now) => Place { path: known.path.clone(), ..now },
                _ => known.clone(),
            })
            .collect();
        Places { version: self.version, places }
    }

    /// Each identity of `noted` laid on the place of its path still held; whether one changed.
    pub fn take_identities(&mut self, noted: &Places) -> bool {
        let mut changed = false;
        for place in &mut self.places {
            let Some(now) = noted.places.iter().find(|n| n.path == place.path) else { continue };
            if (&now.volume, now.file_id) != (&place.volume, place.file_id) {
                (place.volume, place.file_id) = (now.volume.clone(), now.file_id);
                changed = true;
            }
        }
        changed
    }
}

/// Each place's identity read off the interface thread, as a volume asleep can hold a `statfs`;
/// `land` takes what was read, for the holder of the places to keep and write.
pub fn record(places: Places, identify: Identify, land: impl FnOnce(Places) + Send + 'static) {
    std::thread::spawn(move || land(places.noted(|p| place(p, &identify))));
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod identity_tests;
