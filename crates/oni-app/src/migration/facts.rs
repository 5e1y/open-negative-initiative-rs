//! What a sidecar read asks of an original, installed once: the camera matrices a former Mac version
//! kept in `<support>/Transition/cameras.json`, and the scan's size from the file's metadata.

use oni_settings::PipelineSettings;
use oni_settings::geometry::Size;
use oni_settings::migrate::{self, Facts, M3};
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

pub type Matrices = HashMap<(String, String), M3>;

/// Each camera's matrix, by maker and model as LibRaw names them; empty without a table.
pub fn matrices(support: &Path) -> Matrices {
    let Ok(data) = std::fs::read(support.join(FOLDER).join(TABLE)) else { return HashMap::new() };
    let Ok(table) = serde_json::from_slice::<Table>(&data) else { return HashMap::new() };
    table.cameras.into_iter().map(|e| ((e.make, e.model), e.matrix)).collect()
}

/// The matrix `path`'s decode went through: a RAW LibRaw names, of a camera the table holds; a DNG
/// never took one. `Err` when the file cannot be read now.
pub fn matrix_of(table: &Matrices, path: &Path) -> Result<Option<M3>, oni_decode::DecodeError> {
    if !oni_decode::decodes_via_mosaic(path) {
        return Ok(None);
    }
    let camera = oni_decode::camera::identify(path)?;
    Ok((!camera.dng).then(|| table.get(&(camera.make, camera.model)).copied()).flatten())
}

/// The whole decode's size `path` gives with these settings: a lead's three passes are laid on their
/// green one, so its size is theirs.
pub fn scan_of(path: &Path, s: &PipelineSettings) -> Option<Size> {
    let green = s.rgb_passes.as_ref().map(|p| path.parent().map_or_else(|| Path::new(&p.green).to_path_buf(), |f| f.join(&p.green)));
    oni_decode::full_size(green.as_deref().unwrap_or(path)).map(|(w, h)| (w as f64, h as f64))
}

/// Every later sidecar read asks its original here; each file's camera is named once it reads.
pub fn install(support: &Path) {
    let table = matrices(support);
    let named: Mutex<HashMap<PathBuf, Option<M3>>> = Mutex::default();
    let matrix = move |path: &Path| -> Option<M3> {
        if table.is_empty() {
            return None;
        }
        if let Some(known) = named.lock().ok().and_then(|n| n.get(path).copied()) {
            return known;
        }
        // A file unreadable now is asked again at its next read, so a passing failure never sticks.
        let found = matrix_of(&table, path).ok()?;
        if let Ok(mut n) = named.lock() {
            n.insert(path.to_path_buf(), found);
        }
        found
    };
    migrate::install(Some(Arc::new(move |path: &Path, mosaic: bool, then: &mut dyn FnMut(&Facts)| {
        then(&Facts { mosaic, matrix: &|| matrix(path), scan: &|s| scan_of(path, s) })
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
