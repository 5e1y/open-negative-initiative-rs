//! `<support>/Transition/cameras.json`: each camera's rendition matrix with the frames it was fitted on
//! and its residual, written whole or not at all, and installed into every decode of the process.

use crate::measure::{M3, Rgb};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const FOLDER: &str = "Transition";
pub const FILE: &str = "cameras.json";
/// The table's own format; a newer one is read by no decode and never written over.
pub const VERSION: i64 = 1;

pub fn folder(support: &Path) -> PathBuf {
    support.join(FOLDER)
}

pub fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub make: String,
    pub model: String,
    /// LibRaw's Rec. 2020 to Apple's, rows first.
    pub matrix: M3,
    pub frames: Vec<String>,
    /// Density error left per channel over the frames' squares (log10, root mean square).
    pub residual: Rgb,
    #[serde(rename = "measuredAt")]
    pub measured_at: f64,
}

/// A camera whose reachable frames would not match, so a launch does not try it again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Unmatched {
    pub make: String,
    pub model: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub version: i64,
    pub cameras: Vec<Entry>,
    #[serde(default)]
    pub unmatched: Vec<Unmatched>,
}

impl Default for Table {
    fn default() -> Self {
        Table { version: VERSION, cameras: Vec::new(), unmatched: Vec::new() }
    }
}

impl Table {
    pub fn path(support: &Path) -> PathBuf {
        folder(support).join(FILE)
    }

    /// The table on file, empty when there is none; an unreadable or newer one is an error, never
    /// replaced by an empty table.
    pub fn load(support: &Path) -> std::io::Result<Table> {
        let data = match std::fs::read(Self::path(support)) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Table::default()),
            Err(e) => return Err(e),
        };
        let table: Table = serde_json::from_slice(&data).map_err(|e| std::io::Error::other(format!("the camera table is unreadable: {e}")))?;
        if table.version > VERSION {
            return Err(std::io::Error::other("the camera table was written by a newer version"));
        }
        Ok(table)
    }

    pub fn save(&self, support: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(folder(support))?;
        let text = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        oni_library::write_atomically(&Self::path(support), &text)
    }

    pub fn get(&self, make: &str, model: &str) -> Option<&Entry> {
        self.cameras.iter().find(|c| c.make == make && c.model == model)
    }

    /// Whether a run has this camera's answer, a matrix or a refusal.
    pub fn settled(&self, make: &str, model: &str) -> bool {
        self.get(make, model).is_some() || self.unmatched.iter().any(|u| u.make == make && u.model == model)
    }

    /// Adds or replaces a camera's entry.
    pub fn put(&mut self, entry: Entry) {
        self.unmatched.retain(|u| !(u.make == entry.make && u.model == entry.model));
        self.cameras.retain(|c| !(c.make == entry.make && c.model == entry.model));
        self.cameras.push(entry);
    }

    /// Every later decode of the process folds these matrices.
    pub fn install(&self) {
        let narrow = |m: &M3| m.map(|row| row.map(|v| v as f32));
        oni_decode::rendition::install(self.cameras.iter().map(|c| (c.make.clone(), c.model.clone(), narrow(&c.matrix))));
    }
}

/// The table on file installed at launch; how many cameras it holds.
pub fn install_from(support: &Path) -> std::io::Result<usize> {
    let table = Table::load(support)?;
    table.install();
    Ok(table.cameras.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    fn entry(model: &str) -> Entry {
        let matrix = [[1.1252, -0.1698, -0.0064], [0.0259, 0.9537, -0.064], [0.0092, -0.0109, 0.8969]];
        Entry { make: "Nikon".into(), model: model.into(), matrix, frames: vec!["/r/a.NEF".into()], residual: [7e-4, 8e-4, 4e-3], measured_at: 1.0 }
    }

    #[test]
    fn the_table_round_trips_and_an_unreadable_one_is_never_replaced() {
        let scratch = Scratch::new("table");
        assert_eq!(Table::load(scratch.path()).unwrap(), Table::default(), "no file, no camera");
        let mut table = Table::default();
        table.put(entry("Z f"));
        table.put(entry("Z 6"));
        table.put(entry("Z f"));
        table.save(scratch.path()).unwrap();
        let read = Table::load(scratch.path()).unwrap();
        assert_eq!(read, table);
        assert_eq!(read.cameras.len(), 2, "a camera measured again replaces its entry");
        let leftovers = std::fs::read_dir(folder(scratch.path())).unwrap().count();
        assert_eq!(leftovers, 1, "the atomic write leaves no temporary file");
        // Twin: a torn file and a newer format are errors, and the file keeps its bytes.
        std::fs::write(Table::path(scratch.path()), b"{\"version\":1,\"cam").unwrap();
        assert!(Table::load(scratch.path()).is_err());
        std::fs::write(Table::path(scratch.path()), b"{\"version\":2,\"cameras\":[]}").unwrap();
        assert!(Table::load(scratch.path()).is_err());
        assert_eq!(std::fs::read(Table::path(scratch.path())).unwrap(), b"{\"version\":2,\"cameras\":[]}");
    }
}
