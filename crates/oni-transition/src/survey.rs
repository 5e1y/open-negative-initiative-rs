//! The cameras behind the library's RAWs: each file named by LibRaw once, then remembered by its size
//! and date in `<support>/Transition/seen.json`, so a launch reads only what changed.

use crate::table;
use oni_decode::camera::Camera;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub const SEEN: &str = "seen.json";

/// What a file was found to be, and the size and date it was read at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Seen {
    pub size: u64,
    pub modified: f64,
    /// None for a file LibRaw does not decode as a mosaic.
    pub camera: Option<(String, String, bool)>,
}

#[derive(Debug, Default, PartialEq)]
pub struct Found {
    /// Each camera's RAWs, sorted; DNGs left out, as they decode alike in both apps.
    pub cameras: BTreeMap<Camera, Vec<PathBuf>>,
    pub unreachable: Vec<String>,
}

fn stamp(path: &Path) -> Option<(u64, f64)> {
    let m = std::fs::metadata(path).ok()?;
    let modified = m.modified().ok()?.duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
    Some((m.len(), modified))
}

fn read(raw: &Path) -> Option<Option<(String, String, bool)>> {
    if oni_decode::route(raw) != oni_decode::Branch::Mosaic {
        return Some(None);
    }
    let c = oni_decode::camera::identify(raw).ok()?;
    Some(Some((c.make, c.model, c.dng)))
}

/// Each file LibRaw names, on every core: a RAW's metadata takes milliseconds, a library thousands.
fn named(raws: &[(PathBuf, (u64, f64))]) -> Vec<Option<Seen>> {
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    let chunk = raws.len().div_ceil(workers).max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = raws
            .chunks(chunk)
            .map(|part| scope.spawn(move || part.iter().map(|(p, (size, modified))| read(p).map(|camera| Seen { size: *size, modified: *modified, camera })).collect::<Vec<_>>()))
            .collect();
        // A worker lost answers None for each of its files, so every later answer keeps its file.
        handles.into_iter().zip(raws.chunks(chunk)).flat_map(|(h, part)| h.join().unwrap_or_else(|_| vec![None; part.len()])).collect()
    })
}

/// Every RAW under the reachable `roots`, by camera. Reads the disk; remembers what it read.
pub fn survey(support: &Path, roots: &[String]) -> Found {
    let (reachable, unreachable): (Vec<String>, Vec<String>) = roots.iter().cloned().partition(|r| Path::new(r).is_dir());
    let path = table::folder(support).join(SEEN);
    let mut seen: HashMap<String, Seen> = std::fs::read(&path).ok().and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_default();
    let raws: Vec<(PathBuf, (u64, f64))> = oni_library::roots::scan(&reachable)
        .into_iter()
        .map(PathBuf::from)
        .filter(|p| oni_decode::is_raw(p))
        .filter_map(|p| stamp(&p).map(|s| (p, s)))
        .collect();
    let fresh: Vec<(PathBuf, (u64, f64))> = raws
        .iter()
        .filter(|(p, (size, modified))| seen.get(&*p.to_string_lossy()).is_none_or(|s| s.size != *size || s.modified != *modified))
        .cloned()
        .collect();
    let read_now = named(&fresh);
    let changed = read_now.iter().any(Option::is_some);
    for ((p, _), s) in fresh.iter().zip(read_now) {
        if let Some(s) = s {
            seen.insert(p.to_string_lossy().into_owned(), s);
        }
    }
    if changed && std::fs::create_dir_all(table::folder(support)).is_ok() {
        if let Ok(text) = serde_json::to_vec(&seen) {
            let _ = oni_library::write_atomically(&path, &text);
        }
    }
    let mut cameras: BTreeMap<Camera, Vec<PathBuf>> = BTreeMap::new();
    for (p, _) in &raws {
        if let Some(Seen { camera: Some((make, model, false)), .. }) = seen.get(&*p.to_string_lossy()) {
            cameras.entry(Camera { make: make.clone(), model: model.clone(), dng: false }).or_default().push(p.clone());
        }
    }
    cameras.values_mut().for_each(|v| v.sort());
    Found { cameras, unreachable }
}
