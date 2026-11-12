//! Every sidecar of an earlier format a person keeps, brought in the background, one thread a volume so a
//! slow disk never holds another; and the gate, which brings a photo's sidecar before its row is read off it.

use super::ledger::{self, Ledger, Left, Why};
use super::walk;
use crate::anonymity::token;
use crate::timing;
use oni_library::navigator::Computer;
use oni_library::presence::{self, Away, Presence};
use oni_library::{library, paths, roots};
use oni_settings::migrate::{self, Refusal, Standing};
use oni_settings::{document, sidecar};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

/// How long a quit waits for the sweep: a mount that stopped answering holds a read in the kernel at
/// will, so the thread is left to the exit, which a temporary not yet renamed survives whole.
const QUIT_WAIT: Duration = Duration::from_secs(2);

/// How a sweep stands, read by the interface while it runs.
#[derive(Debug, Default)]
pub struct Progress {
    pub done: AtomicUsize,
    pub total: AtomicUsize,
    /// Sidecars left as they were.
    pub refused: AtomicUsize,
    pub migrated: AtomicUsize,
    pub running: AtomicBool,
}

/// A sweep under way; dropped, it stops within one file and is joined, or left to the exit past `QUIT_WAIT`.
pub struct Sweep {
    pub progress: Arc<Progress>,
    alive: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    ended: Receiver<()>,
}

impl Sweep {
    pub fn running(&self) -> bool {
        self.progress.running.load(Ordering::Relaxed)
    }
}

impl Drop for Sweep {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Relaxed);
        let ended = self.ended.recv_timeout(QUIT_WAIT).is_ok();
        if let Some(worker) = self.worker.take().filter(|_| ended) {
            let _ = worker.join();
        }
    }
}

/// Whether a sweep is owed: none ended at this format, or places left to take once they answer.
/// A file left as it was owes none: it is said once and taken again only by a read once it changed.
pub fn due(ledger: &Ledger) -> bool {
    ledger.sidecars < document::VERSION || !ledger.pending.is_empty()
}

/// Where a sweep reads and writes: the support folder, the caches, the volumes.
#[derive(Clone)]
pub struct Place {
    pub support: PathBuf,
    pub caches: PathBuf,
    pub computer: Computer,
}

/// The sweep, when one is owed; its end writes the ledger.
pub fn start(place: Place) -> Option<Sweep> {
    let ledger = ledger::read(&place.support)?;
    if !due(&ledger) {
        return None;
    }
    let progress = Arc::new(Progress { running: AtomicBool::new(true), ..Progress::default() });
    let alive = Arc::new(AtomicBool::new(true));
    let (p, a) = (progress.clone(), alive.clone());
    let (end, ended) = channel();
    let worker = std::thread::Builder::new()
        .name("migration".into())
        .spawn(move || {
            swept(&place, &ledger, &p, &a);
            let _ = end.send(());
        })
        .ok()?;
    Some(Sweep { progress, alive, worker: Some(worker), ended })
}

/// What one volume's thread left: the places to take again, the files left as they were, the
/// folders it went through whole.
#[derive(Default)]
struct Part {
    pending: Vec<String>,
    left: Vec<(String, Left)>,
    passed: Vec<String>,
    /// Files of `passed` folders not taken again, as they were left unchanged.
    kept: HashSet<String>,
}

fn swept(place: &Place, ledger: &Ledger, progress: &Arc<Progress>, alive: &Arc<AtomicBool>) {
    let started = std::time::Instant::now();
    let live = || alive.load(Ordering::Relaxed);
    // Once a whole sweep ended at this format, only the places left pending are taken again.
    let whole = ledger.sidecars < document::VERSION;
    let plan = if whole { walk::planned(&place.support, &place.caches, &ledger.pending, &live) } else { walk::Plan { trees: ledger.pending.clone(), folders: Vec::new() } };
    let mut volumes: BTreeMap<String, Vec<(String, bool)>> = BTreeMap::new();
    for (path, tree) in plan.trees.iter().map(|t| (t, true)).chain(plan.folders.iter().map(|f| (f, false))) {
        volumes.entry(place.computer.volume_root(path).unwrap_or_default()).or_default().push((path.clone(), tree));
    }
    let known = Arc::new(ledger.left.clone());
    let threads: Vec<JoinHandle<Part>> = volumes
        .into_values()
        .filter_map(|units| {
            let (progress, alive, computer, known) = (progress.clone(), alive.clone(), place.computer.clone(), known.clone());
            std::thread::Builder::new().name("migration volume".into()).spawn(move || volume(&units, &computer, &known, &progress, &alive)).ok()
        })
        .collect();
    let mut part = Part::default();
    for thread in threads {
        if let Ok(p) = thread.join() {
            part.pending.extend(p.pending);
            part.left.extend(p.left);
            part.passed.extend(p.passed);
            part.kept.extend(p.kept);
        }
    }
    let pending: BTreeSet<String> = part.pending.into_iter().collect();
    if live() {
        let ended = ledger::update(&place.support, |now| {
            if whole {
                now.sidecars = document::VERSION;
            }
            now.pending = pending.iter().cloned().collect();
            let passed: HashSet<&str> = part.passed.iter().map(String::as_str).collect();
            now.left.retain(|path, _| !passed.contains(paths::parent(path).as_str()) || part.kept.contains(path));
            for (path, mut left) in part.left {
                left.said = now.left.get(&path).is_some_and(|was| was.said && was.why == left.why && was.stamp == left.stamp);
                now.left.insert(path, left);
            }
        });
        if let Err(e) = ended {
            timing::line(&format!("migration: the ledger could not be written ({e})"));
        }
    }
    timing::line(&format!(
        "migration: {} folders in {:.1} s, {} sidecars brought, {} left as they are, {} places pending{}",
        progress.done.load(Ordering::Relaxed),
        started.elapsed().as_secs_f64(),
        progress.migrated.load(Ordering::Relaxed),
        progress.refused.load(Ordering::Relaxed),
        pending.len(),
        if live() { "" } else { ", stopped" },
    ));
    progress.running.store(false, Ordering::Relaxed);
}

/// A place that did not answer, as its disk says: pending while its disk is away, else said once and dropped.
fn unanswered(place: &str, presence: Presence, part: &mut Part) {
    match presence {
        Presence::Offline(Away::Disconnected) => part.pending.push(place.to_owned()),
        Presence::Missing => timing::line(&format!("migration: {} is gone from its disk, dropped", token(place))),
        Presence::Offline(Away::Denied) => timing::line(&format!("migration: {} refuses to be read, left as it is", token(place))),
        Presence::Present => {}
    }
}

/// One volume's trees and folders, one file after another; each file left unchanged since it was
/// left (`known`) is not taken again.
fn volume(units: &[(String, bool)], computer: &Computer, known: &BTreeMap<String, Left>, progress: &Progress, alive: &AtomicBool) -> Part {
    let mut part = Part::default();
    let live = || alive.load(Ordering::Relaxed);
    for (path, tree) in units {
        if !live() {
            break;
        }
        let folders = if *tree {
            let survey = roots::surveyed(std::slice::from_ref(path), &live, computer);
            if survey.stopped {
                break;
            }
            for (place, presence) in &survey.away {
                unanswered(place, *presence, &mut part);
            }
            if survey.away.keys().any(|u| paths::same(u, path)) {
                continue;
            }
            let mut folders: BTreeSet<String> = survey.folders.into_iter().collect();
            folders.insert(path.clone());
            folders.extend(survey.images.iter().map(|i| paths::parent(i)));
            folders.into_iter().collect()
        } else {
            vec![path.clone()]
        };
        progress.total.fetch_add(folders.len(), Ordering::Relaxed);
        for folder in folders {
            if !live() {
                break;
            }
            if let Err(e) = brought(&folder, known, progress, &live, &mut part) {
                unanswered(&folder, presence::refused(&folder, &e, computer), &mut part);
            }
            progress.done.fetch_add(1, Ordering::Relaxed);
        }
    }
    part
}

/// Whether a file left as it was is left again: its sidecar unchanged since. One whose original is in
/// the cloud is brought by the gate once opened, never by a walk at each launch.
fn unchanged(original: &Path, known: &BTreeMap<String, Left>) -> bool {
    let key = original.to_string_lossy();
    known.get(key.as_ref()).is_some_and(|l| l.stamp.is_some() && l.stamp == ledger::stamp(&sidecar::url_for(original)))
}

/// One folder brought, each refusal logged by its token.
fn brought(folder: &str, known: &BTreeMap<String, Left>, progress: &Progress, alive: &dyn Fn() -> bool, part: &mut Part) -> std::io::Result<()> {
    let kept = std::cell::RefCell::new(Vec::new());
    let wanted = |original: &Path| {
        if !library::is_importable(original) {
            return false;
        }
        let again = !unchanged(original, known);
        if !again {
            kept.borrow_mut().push(original.to_string_lossy().into_owned());
        }
        again
    };
    let pass = migrate::folder(Path::new(folder), &wanted, alive)?;
    part.passed.push(folder.to_owned());
    part.kept.extend(kept.into_inner());
    for (original, why) in pass.refused {
        timing::line(&format!("migration: {} left as it is: {}", token(&original.to_string_lossy()), why.kind()));
        let stamp = ledger::stamp(&sidecar::url_for(&original));
        part.left.push((original.to_string_lossy().into_owned(), Left { why: Why::of(&why), stamp, said: false }));
        progress.refused.fetch_add(1, Ordering::Relaxed);
    }
    progress.migrated.fetch_add(pass.migrated, Ordering::Relaxed);
    Ok(())
}

/// A photo's sidecar of an earlier format brought before its row is read off it, sweep or not: a
/// folder no sweep knew, on a disk seen for the first time, is brought as its rows are read.
pub fn gated(photo: &str) {
    if let Ok(bytes) = std::fs::read(sidecar::url_for(Path::new(photo))) {
        gated_bytes(photo, &bytes);
    }
}

/// `gated` off the sidecar's `bytes` a read already took.
pub fn gated_bytes(photo: &str, bytes: &[u8]) {
    if !matches!(migrate::standing(bytes), Standing::Older(_)) || !migrate::marked(bytes) {
        return;
    }
    match migrate::file(Path::new(photo), None) {
        Ok(_) | Err(Refusal::Moved | Refusal::Newer(_) | Refusal::Foreign) => {}
        Err(why) => timing::line(&format!("migration: {} left as it is: {}", token(photo), why.kind())),
    }
}

/// A sweep handle around `work`, for the checks of its quit.
#[cfg(test)]
pub(super) fn around(work: impl FnOnce() + Send + 'static) -> Sweep {
    let (end, ended) = channel();
    let worker = std::thread::spawn(move || {
        work();
        let _ = end.send(());
    });
    Sweep { progress: Arc::default(), alive: Arc::new(AtomicBool::new(true)), worker: Some(worker), ended }
}
