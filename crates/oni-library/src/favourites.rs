//! The favourites, bookmarks and nothing more (a star reads no tree, indexes no photo), kept in Swift's
//! `{version, paths}` apart from `roots.json`; and the pool their rows are read on.

use crate::paths::{self, holds, same};
use crate::roots::Roots;
use oni_settings::json;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

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

    /// `None` while the file was never written, which is what seeds it from the roots once. Read in
    /// the archive's spelling, as the Swift app or an earlier build may have kept another.
    pub fn load(support: &Path) -> Option<Favourites> {
        crate::read_json::<Favourites>(&Self::file_path(support)).map(Favourites::respelt)
    }

    /// Each path in the archive's spelling, one spelt twice kept once, in their order. Case never
    /// merges two here, as a volume away cannot say whether it tells `Roll` from `roll`.
    pub fn respelt(self) -> Favourites {
        let mut paths: Vec<String> = Vec::with_capacity(self.paths.len());
        for path in self.paths {
            let path = paths::normal(&path).into_owned();
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
        Favourites { paths, ..self }
    }

    /// A plain `JSONEncoder`'s text: compact, keys in declaration order.
    pub fn save(&self, support: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(support)?;
        let node = json::to_node(self).map_err(std::io::Error::other)?;
        crate::replace_readable(&Self::file_path(support), json::compact(&node).as_bytes())
    }

    /// Read again and merged with what this side changed since `read`, then written: a favourite
    /// another app added or removed meanwhile stays so. `read` becomes what was written.
    pub fn save_merged(&mut self, read: &mut Favourites, support: &Path) -> std::io::Result<()> {
        if let Some(theirs) = Favourites::load(support) {
            self.paths = crate::merge::list(&read.paths, &self.paths, &theirs.paths);
        }
        self.save(support)?;
        *read = self.clone();
        Ok(())
    }

    /// The roots of `roots.json` seed the first favourites; that file is only read.
    pub fn seeded(roots: &Roots) -> Favourites {
        Favourites { version: 1, paths: roots.paths.clone() }.respelt()
    }

    pub fn contains(&self, path: &str) -> bool {
        self.paths.iter().any(|p| same(p, path))
    }

    /// A star off takes its own spelling, else the one a case-only rename outside ONI left behind.
    pub fn toggle(&mut self, path: &str) {
        let spelt = paths::normal(path).into_owned();
        if self.paths.contains(&spelt) {
            self.paths.retain(|p| *p != spelt);
        } else if self.contains(path) {
            self.paths.retain(|p| !same(p, path));
        } else {
            self.paths.push(spelt);
        }
    }

    /// A favourite renamed or moved on the disk follows it, a folder carrying those inside it; one
    /// landing on a favourite already there is kept once, in the earlier place.
    pub fn follow(&mut self, old: &str, new: &str) {
        let mut followed: Vec<String> = Vec::with_capacity(self.paths.len());
        for path in &self.paths {
            let moved = paths::moved(path, old, new);
            if !followed.iter().any(|p| same(p, &moved)) {
                followed.push(moved);
            }
        }
        self.paths = followed;
    }

    /// The favourites at `folder` or under it taken away, as forgetting a folder gone asks; how many.
    pub fn forget(&mut self, folder: &str) -> usize {
        let before = self.paths.len();
        self.paths.retain(|p| !holds(folder, p));
        before - self.paths.len()
    }
}

/// How long one read may hold a thread of `read_apart`'s before the next folder goes without it.
pub const STUCK: Duration = Duration::from_millis(250);

/// Each folder read on a thread of its own, `threads` counted at once, answers landing together as
/// they come; a read held past `stuck` stops counting, so one stuck in its disk holds no other.
pub fn read_apart<T: Send + 'static>(
    folders: Vec<String>,
    threads: usize,
    stuck: Duration,
    read: impl Fn(&str) -> T + Send + Sync + 'static,
    land: impl Fn(Vec<(String, T)>) + Send + 'static,
) {
    if folders.is_empty() {
        return;
    }
    let (send, receive) = mpsc::channel::<(String, T)>();
    let _ = std::thread::Builder::new().name("landing".into()).spawn(move || {
        while let Ok(first) = receive.recv() {
            let mut landed = vec![first];
            landed.extend(receive.try_iter());
            land(landed);
        }
    });
    let read = Arc::new(read);
    let _ = std::thread::Builder::new().name("reading".into()).spawn(move || {
        let held: Arc<(Mutex<Vec<(usize, Instant)>>, Condvar)> = Arc::default();
        for (n, folder) in folders.into_iter().enumerate() {
            let (lock, freed) = &*held;
            let Ok(mut running) = lock.lock() else { return };
            loop {
                running.retain(|(_, at)| at.elapsed() < stuck);
                if running.len() < threads.max(1) {
                    break;
                }
                let wait = running.iter().map(|(_, at)| stuck.saturating_sub(at.elapsed())).min().unwrap_or(stuck);
                let Ok((again, _)) = freed.wait_timeout(running, wait) else { return };
                running = again;
            }
            running.push((n, Instant::now()));
            drop(running);
            let (read, send, held) = (read.clone(), send.clone(), held.clone());
            let _ = std::thread::Builder::new().name("folder".into()).spawn(move || {
                let answer = read(&folder);
                let _ = send.send((folder, answer));
                let (lock, freed) = &*held;
                if let Ok(mut running) = lock.lock() {
                    running.retain(|(i, _)| *i != n);
                }
                freed.notify_one();
            });
        }
    });
}

#[cfg(test)]
mod tests;
