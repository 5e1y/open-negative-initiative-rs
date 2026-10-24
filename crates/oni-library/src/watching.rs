//! The folders watched for what another app changes: the opened one with all it holds, each listed
//! one a level down. A burst of events leaves as one call once the disk quiets, off every caller's thread.

use crate::navigator::Computer;
use crate::paths::{self, holds};
use notify::event::{EventKind, ModifyKind};
use notify::{RecursiveMode, Watcher};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

/// How long the disk stays quiet before a burst is told, and the longest a burst waits: a copy of
/// 300 files lands as one call, a copy lasting minutes is told every few seconds.
#[derive(Debug, Clone, Copy)]
pub struct Pace {
    pub quiet: Duration,
    pub longest: Duration,
}

impl Pace {
    pub const APP: Pace = Pace { quiet: Duration::from_millis(500), longest: Duration::from_secs(3) };
}

/// The watched folders an event touched, in the spelling they were asked in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Touched {
    /// Watched with all they hold: something under them came, went or was renamed.
    pub deep: Vec<String>,
    /// Watched a level down: a folder or a photo right inside came, went or was renamed.
    pub flat: Vec<String>,
}

enum Order {
    Set { deep: Vec<String>, flat: Vec<String> },
    Event(notify::Result<notify::Event>),
    Stop,
}

/// The watching thread's handle; dropping it ends the thread and every watch.
pub struct Watch {
    orders: Sender<Order>,
}

impl Watch {
    /// The thread started with nothing watched; `told` runs on it, once a burst quiets.
    pub fn start(computer: Computer, pace: Pace, told: impl Fn(Touched) + Send + 'static) -> Watch {
        let (orders, inbox) = mpsc::channel::<Order>();
        let events = orders.clone();
        let _ = std::thread::Builder::new().name("watching".into()).spawn(move || {
            let mut watcher = notify::recommended_watcher(move |e| {
                let _ = events.send(Order::Event(e));
            })
            .ok();
            let mut held = Held::default();
            let mut burst = Burst::new(pace);
            loop {
                let wait = burst.due().map_or(Duration::from_secs(3600), |due| due.saturating_duration_since(Instant::now()));
                match inbox.recv_timeout(wait) {
                    Ok(Order::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                    Ok(Order::Set { deep, flat }) => {
                        if let Some(w) = watcher.as_mut() {
                            held.set(w, &computer, deep, flat);
                        }
                    }
                    Ok(Order::Event(Ok(event))) => burst.note(Instant::now(), held.touched(&event)),
                    Ok(Order::Event(Err(_))) => {}
                    Err(RecvTimeoutError::Timeout) => {}
                }
                if let Some(touched) = burst.take(Instant::now()) {
                    told(touched);
                }
            }
        });
        Watch { orders }
    }

    /// What is watched from now on, in place of what was; a folder the rules refuse is left out.
    pub fn set(&self, deep: Vec<String>, flat: Vec<String>) {
        let _ = self.orders.send(Order::Set { deep, flat });
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        let _ = self.orders.send(Order::Stop);
    }
}

/// Events gathered until the disk quiets or the burst has waited its longest.
pub struct Burst {
    pace: Pace,
    held: Option<(Instant, Instant, Touched)>,
}

impl Burst {
    pub fn new(pace: Pace) -> Burst {
        Burst { pace, held: None }
    }

    pub fn note(&mut self, now: Instant, touched: Touched) {
        if touched.deep.is_empty() && touched.flat.is_empty() {
            return;
        }
        let (_, last, all) = self.held.get_or_insert_with(|| (now, now, Touched::default()));
        *last = now;
        merge(&mut all.deep, touched.deep);
        merge(&mut all.flat, touched.flat);
    }

    /// When the burst held is told, if one is.
    pub fn due(&self) -> Option<Instant> {
        self.held.as_ref().map(|(first, last, _)| (*last + self.pace.quiet).min(*first + self.pace.longest))
    }

    /// The burst, once due.
    pub fn take(&mut self, now: Instant) -> Option<Touched> {
        if self.due()? > now {
            return None;
        }
        self.held.take().map(|(_, _, t)| t)
    }
}

/// Never a whole disk nor the home folder, nor a folder holding either, which would watch them too.
pub fn may_watch(computer: &Computer, folder: &str) -> bool {
    let wide = |root: &str| !root.is_empty() && holds(folder, root);
    !folder.is_empty() && folder != "/" && !computer.is_volume_root(folder) && !wide(&computer.home) && !wide(&computer.volumes)
}

/// Each watched folder as asked and as the disk spells it, which is how events name its contents.
#[derive(Default)]
struct Held {
    deep: Vec<(String, String)>,
    flat: Vec<(String, String)>,
}

impl Held {
    fn set(&mut self, watcher: &mut notify::RecommendedWatcher, computer: &Computer, deep: Vec<String>, flat: Vec<String>) {
        let mut changes = watcher.paths_mut();
        for (_, real) in self.deep.iter().chain(&self.flat) {
            let _ = changes.remove(Path::new(real));
        }
        let mut added = |folders: Vec<String>, mode: RecursiveMode| -> Vec<(String, String)> {
            let unique: BTreeSet<String> = folders.into_iter().filter(|f| may_watch(computer, f)).collect();
            // A folder gone since it was asked is left out; the presence looks tell its absence.
            unique.into_iter().filter_map(|f| real(&f).filter(|r| changes.add(Path::new(r), mode).is_ok()).map(|r| (f, r))).collect()
        };
        let (deep, flat) = (added(deep, RecursiveMode::Recursive), added(flat, RecursiveMode::NonRecursive));
        let _ = changes.commit();
        (self.deep, self.flat) = (deep, flat);
    }

    fn touched(&self, event: &notify::Event) -> Touched {
        let mut out = Touched::default();
        let lost = event.need_rescan();
        for path in &event.paths {
            let path = paths::normal(&path.to_string_lossy()).into_owned();
            if !lost && !counts(&path, &event.kind) {
                continue;
            }
            for (asked, real) in &self.deep {
                if holds(real, &path) || (lost && holds(&path, real)) {
                    merge(&mut out.deep, vec![asked.clone()]);
                }
            }
            for (asked, real) in &self.flat {
                if paths::same(&paths::parent(&path), real) || (lost && holds(&path, real)) {
                    merge(&mut out.flat, vec![asked.clone()]);
                }
            }
        }
        if lost && event.paths.is_empty() {
            merge(&mut out.deep, self.deep.iter().map(|(a, _)| a.clone()).collect());
            merge(&mut out.flat, self.flat.iter().map(|(a, _)| a.clone()).collect());
        }
        out
    }
}

/// What a folder and those under it hold, names alone: two listings differ when a photo or a folder
/// came, went or was renamed, never for a sidecar written. `None` while the disk refuses the folder.
pub type Listing = BTreeMap<String, Vec<String>>;

pub fn listing(folder: &str) -> Option<Listing> {
    let mut out = Listing::new();
    let mut next = vec![folder.to_owned()];
    while let Some(at) = next.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            if at == folder {
                return None;
            }
            continue;
        };
        let mut names = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || paths::extension(&name).eq_ignore_ascii_case("json") {
                continue;
            }
            let path = entry.path();
            if entry.file_type().is_ok_and(|t| t.is_dir()) && !crate::roots::is_package(&path) {
                next.push(paths::join(&at, &name));
            }
            names.push(name);
        }
        names.sort();
        out.insert(at, names);
    }
    Some(out)
}

/// How the disk spells a folder, as events name what it holds: on macOS `/private/var`, never `/var`.
fn real(folder: &str) -> Option<String> {
    if cfg!(windows) {
        return Path::new(folder).is_dir().then(|| folder.to_owned());
    }
    std::fs::canonicalize(folder).ok().map(|r| paths::normal(&r.to_string_lossy()).into_owned())
}

/// A photo written, or a photo or a folder that came, went or was renamed; never a sidecar, a
/// hidden file or a tag.
fn counts(path: &str, kind: &EventKind) -> bool {
    let named = match kind {
        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_)) => true,
        EventKind::Modify(ModifyKind::Data(_) | ModifyKind::Any) => false,
        _ => return false,
    };
    if paths::name(path).starts_with('.') || paths::extension(path).eq_ignore_ascii_case("json") {
        return false;
    }
    if crate::library::is_importable(Path::new(path)) {
        return true;
    }
    // A removed folder cannot be looked at: one with no extension is taken for a folder.
    named && std::fs::symlink_metadata(path).map_or(paths::extension(path).is_empty(), |m| m.is_dir())
}

fn merge(into: &mut Vec<String>, more: Vec<String>) {
    for m in more {
        if !into.contains(&m) {
            into.push(m);
        }
    }
}

#[cfg(test)]
#[path = "watching/tests.rs"]
mod tests;
