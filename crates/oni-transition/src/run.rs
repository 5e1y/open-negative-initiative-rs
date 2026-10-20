//! The transition's run: every camera behind the library's RAWs that the table lacks is matched once,
//! on a few of its frames, to Apple's decode; nothing in any photo's settings is read or written.

use crate::apple::AppleDecode;
use crate::measure::{self, M3, Plane, Rgb};
use crate::survey;
use crate::table::{self, Entry, Table};
use std::path::{Path, PathBuf};

/// The frames a camera's matrix is fitted on together, and the most tried to find them.
pub const FRAMES: usize = 3;
pub const TRIES: usize = 6;
/// A fitted matrix this far from the identity is a failed measure, not a camera.
const PLAUSIBLE: (f64, f64) = (0.5, 0.5);

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The cameras to match, by name, with their frame counts.
    Found { cameras: Vec<(String, usize)> },
    Measuring { camera: String },
    Measured { camera: String, residual: Result<Rgb, String> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraLine {
    pub name: String,
    pub frames: usize,
    pub residual: Result<Rgb, String>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Summary {
    pub cameras: Vec<CameraLine>,
    /// Every frame of the cameras matched by this run, whose renders change.
    pub matched: Vec<PathBuf>,
    pub unreachable: Vec<String>,
}

/// How many cameras of the library the table lacks; none when the table cannot be read.
pub fn pending(support: &Path, roots: &[String]) -> usize {
    let Ok(table) = Table::load(support) else { return 0 };
    survey::survey(support, roots).cameras.keys().filter(|c| table.get(&c.make, &c.model).is_none()).count()
}

/// The frames tried first spread over the camera's, so one odd frame cannot carry it.
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

fn measure_one(apple: &dyn AppleDecode, raw: &Path) -> Result<Vec<(Rgb, Rgb)>, String> {
    let grid = apple.blocks(raw)?;
    let decoded = oni_decode::decode_native(raw, 1.0).map_err(|e| format!("LibRaw cannot decode it ({})", e.0))?;
    let plane = Plane { width: decoded.width, height: decoded.height, pixels: &decoded.pixels };
    measure::fit(&grid, plane).map(|s| s.pairs).map_err(|r| r.to_string())
}

fn plausible(m: &M3) -> bool {
    (0..3).all(|r| (0..3).all(|c| if r == c { (m[r][c] - 1.0).abs() < PLAUSIBLE.0 } else { m[r][c].abs() < PLAUSIBLE.1 }))
}

/// A camera's matrix fitted on the pooled squares of up to `FRAMES` frames that measure cleanly.
pub fn measure(apple: &dyn AppleDecode, make: &str, model: &str, raws: &[PathBuf]) -> Result<Entry, String> {
    let (mut pairs, mut frames, mut last) = (Vec::new(), Vec::new(), String::from("no frame could be read"));
    for i in spread(raws.len()) {
        if frames.len() == FRAMES {
            break;
        }
        match measure_one(apple, &raws[i]) {
            Ok(p) => {
                pairs.extend(p);
                frames.push(raws[i].to_string_lossy().into_owned());
            }
            Err(e) => last = e,
        }
    }
    if frames.is_empty() {
        return Err(last);
    }
    let matrix = measure::matrix(&pairs).filter(plausible).ok_or("the two decoders could not be matched")?;
    let residual = measure::residual(&pairs, &matrix);
    Ok(Entry { make: make.into(), model: model.into(), matrix, frames, residual, measured_at: table::now() })
}

/// The whole run over `roots`; `progress` hears each step. Each camera's matrix is on file and
/// installed before the next is measured, so a run stopped midway resumes where it stood.
pub fn run(support: &Path, roots: &[String], apple: &dyn AppleDecode, progress: &mut dyn FnMut(Event)) -> std::io::Result<Summary> {
    let mut table = Table::load(support)?;
    let found = survey::survey(support, roots);
    let missing: Vec<_> = found.cameras.into_iter().filter(|(c, _)| table.get(&c.make, &c.model).is_none()).collect();
    progress(Event::Found { cameras: missing.iter().map(|(c, r)| (c.name(), r.len())).collect() });
    let mut summary = Summary { unreachable: found.unreachable, ..Summary::default() };
    for (camera, raws) in missing {
        let name = camera.name();
        progress(Event::Measuring { camera: name.clone() });
        let residual = match measure(apple, &camera.make, &camera.model, &raws) {
            Ok(entry) => {
                let residual = entry.residual;
                table.put(entry);
                table.save(support)?;
                table.install();
                summary.matched.extend(raws.iter().cloned());
                Ok(residual)
            }
            Err(e) => Err(e),
        };
        progress(Event::Measured { camera: name.clone(), residual: residual.clone() });
        summary.cameras.push(CameraLine { name, frames: raws.len(), residual });
    }
    Ok(summary)
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
