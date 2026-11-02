//! The camera matrices a former Mac version kept in `<support>/Transition/cameras.json`, which no decode
//! folds any more: each camera's matrix handed to the sidecar reads, the thumbnails drawn again once.

use oni_settings::retired_matrices::M3;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const FOLDER: &str = "Transition";
const TABLE: &str = "cameras.json";
/// Each RAW the former version named, by path: its camera, `None` for a file not a mosaic.
const SEEN: &str = "seen.json";
/// Left once the thumbnails the matrices drew are dropped.
const SWEPT: &str = "retired";

#[derive(Deserialize)]
struct Table {
    cameras: Vec<Entry>,
}

#[derive(Deserialize)]
struct Seen {
    camera: Option<(String, String, bool)>,
}

#[derive(Deserialize)]
struct Entry {
    make: String,
    model: String,
    matrix: M3,
}

/// Each camera's matrix, by maker and model as LibRaw names them; empty without a table.
pub fn matrices(support: &Path) -> HashMap<(String, String), M3> {
    let Ok(data) = std::fs::read(support.join(FOLDER).join(TABLE)) else { return HashMap::new() };
    let Ok(table) = serde_json::from_slice::<Table>(&data) else { return HashMap::new() };
    table.cameras.into_iter().map(|e| ((e.make, e.model), e.matrix)).collect()
}

/// The sidecar reads ask a file's camera here, named once a file LibRaw reads; a DNG never took a matrix.
pub fn install(support: &Path) {
    let matrices = matrices(support);
    if matrices.is_empty() {
        return oni_settings::retired_matrices::install(None);
    }
    let named: Mutex<HashMap<PathBuf, Option<M3>>> = Mutex::default();
    oni_settings::retired_matrices::install(Some(Arc::new(move |path: &Path| {
        if let Some(known) = named.lock().ok().and_then(|n| n.get(path).copied()) {
            return known;
        }
        if !oni_decode::decodes_via_mosaic(path) {
            return None;
        }
        // A file unreadable now is asked again at its next read, so a passing failure never sticks.
        let camera = oni_decode::camera::identify(path).ok()?;
        let found = (!camera.dng).then(|| matrices.get(&(camera.make, camera.model)).copied()).flatten();
        if let Ok(mut n) = named.lock() {
            n.insert(path.to_path_buf(), found);
        }
        found
    })));
}

/// The thumbnails the matrices drew dropped once: a RAW of a camera the table holds, or one the former
/// version never named; never a DNG.
pub fn sweep(support: &Path, index: &Path) {
    let swept = support.join(FOLDER).join(SWEPT);
    let matrices = matrices(support);
    if swept.exists() || matrices.is_empty() {
        return;
    }
    let Some(index) = oni_library::index::LibraryIndex::open(index) else { return };
    let seen: HashMap<String, Seen> = std::fs::read(support.join(FOLDER).join(SEEN)).ok().and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_default();
    let drawn = |path: &str| {
        let p = Path::new(path);
        let dng = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dng"));
        oni_decode::is_raw(p) && !dng && seen.get(path).is_none_or(|s| s.camera.as_ref().is_some_and(|(make, model, dng)| !dng && matrices.contains_key(&(make.clone(), model.clone()))))
    };
    let copies = index.copies();
    for e in index.all().into_iter().filter(|e| drawn(&e.path)) {
        crate::thumbnails::forget(support, &e.fingerprint);
        crate::thumbnails::forget(support, &crate::thumbnails::key(&e.fingerprint, &e.path, &copies));
    }
    let _ = std::fs::write(swept, b"");
}
