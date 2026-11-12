//! A sidecar on disk brought to this format in place, or left byte for byte as it was: converted in
//! memory, written beside itself, read back and compared, then one rename, its modification date kept.

use super::{Facts, Mosaic, Refusal, Standing, asked, converted, draft, standing};
use crate::atomic::{self, Unwritten};
use crate::document::{self, Failure};
use crate::sidecar::{self, Error, held, identity};
use std::io;
use std::path::{Path, PathBuf};

/// What `file` found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Current,
    Migrated { from: i64 },
    /// No sidecar, or none beside a present original: nothing is written beside nothing.
    Missing,
}

/// One sidecar brought to this format in place with the installed facts, or left as it was;
/// `mosaic` as the caller knows it, else asked of the original only by the oldest formats.
pub fn file(original: &Path, mosaic: Option<bool>) -> Result<Outcome, Refusal> {
    asked(original, mosaic, |facts| file_with(original, facts))
}

/// `file` with the facts given.
pub fn file_with(original: &Path, facts: &Facts) -> Result<Outcome, Refusal> {
    file_through(original, facts, &atomic::renamed)
}

/// `file_with` through its own rename, which a check makes fail.
pub(crate) fn file_through(original: &Path, facts: &Facts, rename: &dyn Fn(&Path, &Path) -> io::Result<()>) -> Result<Outcome, Refusal> {
    let _held = held::hold(original);
    let path = sidecar::url_for(original);
    let old = match std::fs::read(&path) {
        Ok(old) => old,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Outcome::Missing),
        Err(e) => return Err(Refusal::Unreadable(Failure::Malformed(e.to_string()))),
    };
    match standing(&old) {
        Standing::Current => return Ok(Outcome::Current),
        Standing::Newer(version) => return Err(Refusal::Newer(version)),
        Standing::NotASidecar => return Err(Refusal::Unreadable(Failure::NotAnObject)),
        Standing::Older(_) => {}
    }
    match original.try_exists() {
        Ok(true) => {}
        Ok(false) => return Ok(Outcome::Missing),
        Err(e) => return Err(Refusal::Unwritable(e.to_string())),
    }
    // Another file's sidecar is never converted with this file's facts nor gives it its draft: the
    // print is read once, before the first fact, as reading it reads the original.
    let foreign = std::cell::OnceCell::new();
    let ours = || if *foreign.get_or_init(|| identity::foreign_bytes(&old, original)) { Err(Refusal::Foreign) } else { Ok(()) };
    let mosaic = || ours().and_then(|()| facts.mosaic.get());
    let matrix = || ours().and_then(|()| (facts.matrix)());
    let scan = |s: &crate::PipelineSettings| ours().and_then(|()| (facts.scan)(s));
    let guarded = Facts { mosaic: Mosaic::Asked(&mosaic), matrix: &matrix, scan: &scan };
    let converted = converted(&old, &guarded)?;
    if let (Some(stored), Some(folder), Some(name)) = (&converted.draft, path.parent(), original.file_name().and_then(|n| n.to_str())) {
        ours()?;
        // A draft a newer build keeps is its own: this frame's, transient, is left to it.
        match draft::kept_in(folder, name, stored) {
            Ok(()) | Err(Error::Newer) => {}
            Err(e) => return Err(Refusal::Drafts(e.to_string())),
        }
    }
    let text = document::encode(&converted.document).map_err(Refusal::Unreadable)?;
    // The old reading read again from the drive, and a fixed point: nothing the reader swallowed.
    let verify = |back: &[u8]| document::decode(back).is_ok_and(|d| d == converted.document && document::encode(&d).is_ok_and(|t| t == back));
    let unmoved = || std::fs::read(&path).is_ok_and(|now| now == old);
    atomic::verified_by(&path, &text, &verify, &unmoved, true, rename).map_err(|e| match e {
        Unwritten::Io(e) => Refusal::Unwritable(e.to_string()),
        Unwritten::Unverified => Refusal::Unverified,
        Unwritten::Moved => Refusal::Moved,
    })?;
    Ok(Outcome::Migrated { from: converted.from })
}

/// What a folder's pass found.
#[derive(Debug, Default)]
pub struct Folder {
    pub current: usize,
    pub migrated: usize,
    pub missing: usize,
    /// Written by a newer version: never rewritten, nothing to say.
    pub newer: usize,
    /// Written for another file of that name: set aside at the photo's first write, nothing to say.
    pub foreign: usize,
    pub refused: Vec<(PathBuf, Refusal)>,
}

/// The original `name` names when it is a sidecar's, `X` of `X.json`; `X (orphan).json` names no file.
pub fn sidecar_of(name: &str) -> Option<&str> {
    name.strip_suffix(".json").filter(|stem| !stem.is_empty())
}

/// Whether `bytes` carry an earlier sidecar's marks: an integer version and settings naming their mode,
/// so another program's `X.json` holding some `settings` is never taken for one.
pub fn marked(bytes: &[u8]) -> bool {
    #[derive(serde::Deserialize)]
    struct Marks {
        version: serde_json::Value,
        settings: serde_json::Map<String, serde_json::Value>,
    }
    serde_json::from_slice::<Marks>(bytes).is_ok_and(|m| crate::tolerant::ns_integer(Some(&m.version)).is_some() && m.settings.contains_key("mode"))
}

/// Every sidecar right inside `folder`: `X.json` with a sidecar's marks beside a file `X` that `wanted`
/// takes (a photo ONI imports). `alive` is read before each file; `Err` when the folder stops answering.
pub fn folder(folder: &Path, wanted: &dyn Fn(&Path) -> bool, alive: &dyn Fn() -> bool) -> io::Result<Folder> {
    let mut files = std::collections::BTreeSet::new();
    let mut named = Vec::new();
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else { continue };
        if name.starts_with('.') {
            continue;
        }
        if entry.path().is_file() {
            files.insert(name.clone());
        }
        named.push(name);
    }
    let mut out = Folder::default();
    for name in &named {
        let Some(stem) = sidecar_of(name).filter(|stem| files.contains(*stem) && files.contains(name) && wanted(&folder.join(stem))) else { continue };
        if !alive() {
            break;
        }
        let original = folder.join(stem);
        let bytes = match std::fs::read(folder.join(name)) {
            Ok(bytes) => bytes,
            Err(e) if answers(folder) => {
                out.refused.push((original, Refusal::Unreadable(Failure::Malformed(e.to_string()))));
                continue;
            }
            Err(e) => return Err(e),
        };
        match document::sniffed(&bytes) {
            Some(s) if s.named && s.version == document::VERSION => out.current += 1,
            Some(s) if s.version > document::VERSION => out.newer += 1,
            Some(_) if marked(&bytes) => match file(&original, None) {
                Ok(Outcome::Current) => out.current += 1,
                Ok(Outcome::Migrated { .. }) => out.migrated += 1,
                Ok(Outcome::Missing) => out.missing += 1,
                Err(Refusal::Newer(_)) => out.newer += 1,
                Err(Refusal::Foreign) => out.foreign += 1,
                // Another writer brought it meanwhile, as a second machine on the same disk does.
                Err(Refusal::Moved) if std::fs::read(folder.join(name)).is_ok_and(|b| standing(&b) == Standing::Current) => out.current += 1,
                Err(why) => out.refused.push((original, why)),
            },
            _ => {}
        }
    }
    // A disk gone midway leaves the folder to be taken again whole, never its files said refused.
    if !out.refused.is_empty() && !answers(folder) {
        return Err(io::ErrorKind::NotFound.into());
    }
    Ok(out)
}

fn answers(folder: &Path) -> bool {
    matches!(folder.try_exists(), Ok(true))
}
