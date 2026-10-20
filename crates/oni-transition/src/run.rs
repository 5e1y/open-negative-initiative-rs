//! The transition's run: the earlier app's RAW sidecars found, each camera's shift measured once on a
//! few of its frames, then every frame corrected and recorded. Safe to stop anywhere and run again.

use crate::apple::AppleDecode;
use crate::correct;
use crate::measure::{self, Plane};
use crate::records::{self, CameraRecord, Entry, Ledger, Outcome, State};
use crate::survey;
use oni_decode::camera::Camera;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The frames a camera's shift is the median of, and the most tried to find them.
pub const FRAMES: usize = 3;
pub const TRIES: usize = 6;

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The frames found, by camera name.
    Found { cameras: Vec<(String, usize)> },
    Measuring { camera: String },
    Measured { camera: String, stops: Result<[f64; 3], String> },
    Corrected { done: usize, of: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraLine {
    pub name: String,
    pub frames: usize,
    pub stops: Result<[f64; 3], String>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Summary {
    pub cameras: Vec<CameraLine>,
    /// The RAWs whose sidecar was corrected.
    pub corrected: Vec<PathBuf>,
    /// The RAWs left as they are, and why.
    pub left: Vec<(PathBuf, String)>,
    pub unreachable: Vec<String>,
}

/// The roots asked for and those unreachable at the last run, which may have come back.
fn with_deferred(roots: &[String], state: &State) -> Vec<String> {
    let mut all: Vec<String> = roots.iter().chain(&state.deferred).cloned().collect();
    all.sort();
    all.dedup();
    all
}

/// How many frames a run would correct now; the first call installs the transition, so a sidecar
/// written by this version from then on is never taken for the earlier one's. Zero on any doubt.
pub fn pending(support: &Path, roots: &[String]) -> usize {
    let Ok(state) = State::begin(support) else { return 0 };
    if state.version > records::VERSION {
        return 0;
    }
    let ledger = Ledger::load(support);
    let found = survey::survey(&with_deferred(roots, &state), state.installed_at, &ledger);
    found.raws.iter().filter(|r| !ledger.entries.contains_key(&*oni_settings::sidecar::url_for(r).to_string_lossy())).count()
}

enum Sorted {
    Camera(Camera),
    Left(Outcome, String),
}

fn sort(raw: &Path) -> Sorted {
    if oni_decode::route(raw) != oni_decode::Branch::Mosaic {
        return Sorted::Left(Outcome::Skipped, "not a camera RAW".into());
    }
    match oni_decode::camera::identify(raw) {
        Err(e) => Sorted::Left(Outcome::Failed, format!("LibRaw cannot read it ({})", e.0)),
        Ok(c) if c.dng => Sorted::Left(Outcome::Skipped, "a DNG decodes alike in both versions".into()),
        Ok(c) => Sorted::Camera(c),
    }
}

/// Each RAW's camera, read on every core: opening a RAW's metadata takes milliseconds, a library thousands.
fn sorted(raws: &[PathBuf]) -> Vec<Sorted> {
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    let chunk = raws.len().div_ceil(workers).max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = raws.chunks(chunk).map(|part| scope.spawn(move || part.iter().map(|r| sort(r)).collect::<Vec<_>>())).collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    })
}

/// The frames tried first spread over the roll, so one odd frame cannot carry a camera.
pub fn spread(n: usize) -> Vec<usize> {
    let mut out = Vec::new();
    for k in [1, 3, 5, 0, 2, 4] {
        let i = (2 * k + 1) * n / (2 * TRIES);
        if i < n && !out.contains(&i) {
            out.push(i);
        }
    }
    out
}

fn measure_one(apple: &dyn AppleDecode, raw: &Path) -> Result<[f64; 3], String> {
    let grid = apple.blocks(raw)?;
    let decoded = oni_decode::decode(raw, 1.0).map_err(|e| format!("LibRaw cannot decode it ({})", e.0))?;
    let plane = Plane { width: decoded.width, height: decoded.height, pixels: &decoded.pixels };
    measure::fit(&grid, plane).map(|s| s.stops).map_err(|r| r.to_string())
}

/// A camera's shift, the median of up to `FRAMES` frames that measure cleanly.
pub fn measure(apple: &dyn AppleDecode, raws: &[PathBuf]) -> Result<([f64; 3], Vec<String>), String> {
    let (mut shifts, mut used, mut last) = (Vec::new(), Vec::new(), String::from("no frame to measure"));
    for i in spread(raws.len()) {
        if shifts.len() == FRAMES {
            break;
        }
        match measure_one(apple, &raws[i]) {
            Ok(s) => {
                shifts.push(s);
                used.push(raws[i].to_string_lossy().into_owned());
            }
            Err(e) => last = e,
        }
    }
    if shifts.is_empty() { Err(last) } else { Ok((measure::combined(&shifts), used)) }
}

fn left(raw: &Path, outcome: Outcome, reason: &str) -> Entry {
    let path = oni_settings::sidecar::url_for(raw);
    let before = std::fs::read(&path).map(|b| records::digest(&b)).unwrap_or_default();
    Entry {
        sidecar: path.to_string_lossy().into_owned(),
        outcome,
        reason: Some(reason.to_owned()),
        before,
        after: None,
        stops: None,
        backup: None,
        at: records::now(),
    }
}

/// The whole run over `roots`; `progress` hears each step. Every frame's outcome is on file before
/// the next starts, so a run stopped midway resumes where it stood.
pub fn run(support: &Path, roots: &[String], apple: &dyn AppleDecode, progress: &mut dyn FnMut(Event)) -> std::io::Result<Summary> {
    let mut state = State::begin(support)?;
    if state.version > records::VERSION {
        return Ok(Summary::default());
    }
    let mut ledger = Ledger::load(support);
    let found = survey::survey(&with_deferred(roots, &state), state.installed_at, &ledger);
    state.deferred = found.unreachable.clone();
    state.save(support)?;
    let mut summary = Summary { unreachable: found.unreachable, ..Summary::default() };
    let mut cameras: BTreeMap<Camera, Vec<PathBuf>> = BTreeMap::new();
    for (raw, sorted) in found.raws.iter().zip(sorted(&found.raws)) {
        match sorted {
            Sorted::Camera(c) => cameras.entry(c).or_default().push(raw.clone()),
            Sorted::Left(outcome, reason) => {
                ledger.record(support, left(raw, outcome, &reason))?;
                summary.left.push((raw.clone(), reason));
            }
        }
    }
    progress(Event::Found { cameras: cameras.iter().map(|(c, r)| (c.name(), r.len())).collect() });
    let of: usize = cameras.values().map(Vec::len).sum();
    let mut done = 0;
    for (camera, raws) in &cameras {
        let name = camera.name();
        let stops = match state.camera(&camera.make, &camera.model) {
            Some(known) => Ok(known.stops),
            None => {
                progress(Event::Measuring { camera: name.clone() });
                let measured = measure(apple, raws);
                if let Ok((stops, frames)) = &measured {
                    state.cameras.push(CameraRecord { make: camera.make.clone(), model: camera.model.clone(), stops: *stops, frames: frames.clone() });
                    state.save(support)?;
                }
                measured.map(|(s, _)| s)
            }
        };
        progress(Event::Measured { camera: name.clone(), stops: stops.clone() });
        summary.cameras.push(CameraLine { name: name.clone(), frames: raws.len(), stops: stops.clone() });
        for raw in raws {
            let entry = match &stops {
                Ok(s) => correct::correct(support, raw, *s),
                Err(why) => left(raw, Outcome::Failed, &format!("{name} could not be measured: {why}")),
            };
            match entry.outcome {
                Outcome::Corrected => summary.corrected.push(raw.clone()),
                _ => summary.left.push((raw.clone(), entry.reason.clone().unwrap_or_default())),
            }
            ledger.record(support, entry)?;
            done += 1;
            progress(Event::Corrected { done, of });
        }
    }
    Ok(summary)
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
