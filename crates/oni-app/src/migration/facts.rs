//! What a sidecar read asks of an original, installed once: the camera matrices a former Mac version
//! kept in `<support>/Transition/cameras.json`, and the scan's size from the file's metadata.

use oni_settings::PipelineSettings;
use oni_settings::geometry::Size;
use oni_settings::migrate::{self, Facts, M3, Mosaic, Refusal};
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

/// The matrices this computer's former version fitted, by camera.
#[derive(Default)]
pub struct Known {
    pub matrices: Matrices,
}

impl Known {
    /// The matrix a camera's decodes went through here: its fit, else none, as no build on a computer
    /// without one for it (Windows, a Mac 1.4.0 to 1.5.1 never ran on) decoded through a matrix.
    pub fn answer(&self, make: &str, model: &str) -> Option<M3> {
        self.matrices.get(&(make.to_owned(), model.to_owned())).copied()
    }
}

/// Each camera's matrix, by maker and model as LibRaw names them; empty without a table.
pub fn matrices(support: &Path) -> Matrices {
    known(support).matrices
}

/// The table as this computer's former version left it; empty without one.
pub fn known(support: &Path) -> Known {
    let Ok(data) = std::fs::read(support.join(FOLDER).join(TABLE)) else { return Known::default() };
    let Ok(table) = serde_json::from_slice::<Table>(&data) else { return Known::default() };
    Known { matrices: table.cameras.into_iter().map(|e| ((e.make, e.model), e.matrix)).collect() }
}

/// The matrix `path`'s decode went through: none for a file outside the mosaic or a DNG, which never
/// took one; else the table's answer for its camera. Reads the file's header, never its pixels.
pub fn matrix_of(known: &Known, path: &Path) -> Result<Option<M3>, Refusal> {
    if !oni_decode::decodes_via_mosaic(path) {
        return Ok(None);
    }
    let camera = oni_decode::camera::identify(path).map_err(|_| Refusal::CameraUnknown)?;
    if camera.dng {
        return Ok(None);
    }
    Ok(known.answer(&camera.make, &camera.model))
}

/// The whole decode's size `path` gives with these settings: a lead's three passes are laid on their
/// green one, so its size is theirs.
pub fn scan_of(path: &Path, s: &PipelineSettings) -> Option<Size> {
    oni_decode::full_size(&scanned(path, s)).map(|(w, h)| (w as f64, h as f64))
}

/// The file a scan's size is read from: the green pass of a lead, else the photo itself.
fn scanned(path: &Path, s: &PipelineSettings) -> PathBuf {
    match &s.rgb_passes {
        Some(p) => path.parent().map_or_else(|| Path::new(&p.green).to_path_buf(), |f| f.join(&p.green)),
        None => path.to_path_buf(),
    }
}

/// Whether `path`'s bytes are kept in the cloud, so asking it a fact would download it whole.
fn away(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| oni_settings::sidecar::identity::in_cloud(&m))
}

/// Every later sidecar read asks its original here; each file's camera is named once it reads, and a
/// file that cannot say now is refused, never taken for a camera without a matrix.
pub fn install(support: &Path) {
    install_with(support, away);
}

/// `install` with `away` telling a file the cloud keeps, which a check stands in for iCloud's.
pub fn install_with(support: &Path, away: fn(&Path) -> bool) {
    migrate::install(Some(Arc::new(lookup(known(support), away))));
}

/// The facts of a file asked through `known`, each fact read only once `away` says the file it reads
/// is on the disk: an iCloud or OneDrive placeholder read for a header would download it whole.
pub fn lookup(known: Known, away: fn(&Path) -> bool) -> impl Fn(&Path, Option<bool>, &mut dyn FnMut(&Facts)) + Send + Sync {
    let named: Mutex<HashMap<PathBuf, Option<M3>>> = Mutex::default();
    let matrix = move |path: &Path| -> Result<Option<M3>, Refusal> {
        if let Some(known) = named.lock().ok().and_then(|n| n.get(path).copied()) {
            return Ok(known);
        }
        if away(path) {
            return Err(Refusal::Away);
        }
        let found = matrix_of(&known, path)?;
        if let Ok(mut n) = named.lock() {
            n.insert(path.to_path_buf(), found);
        }
        Ok(found)
    };
    move |path: &Path, mosaic: Option<bool>, then: &mut dyn FnMut(&Facts)| {
        let asked = || if away(path) { Err(Refusal::Away) } else { Ok(oni_decode::decodes_via_mosaic(path)) };
        let scan = |s: &PipelineSettings| {
            let file = scanned(path, s);
            if away(&file) {
                return Err(Refusal::Away);
            }
            scan_of(path, s).ok_or(Refusal::ScanUnknown)
        };
        let mosaic = mosaic.map_or(Mosaic::Asked(&asked), Mosaic::Known);
        then(&Facts { mosaic, matrix: &|| matrix(path), scan: &scan })
    }
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
