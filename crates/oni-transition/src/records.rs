//! What the transition keeps in `<support>/Transition`: when it was installed and each camera's
//! shift, a ledger of every sidecar it settled, and the backups with their manifest.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const FOLDER: &str = "Transition";
pub const STATE: &str = "state.json";
pub const LEDGER: &str = "ledger.jsonl";
pub const BACKUP: &str = "backup";
pub const MANIFEST: &str = "manifest.jsonl";
/// The records' own format; a newer one is left alone and the transition does nothing.
pub const VERSION: i64 = 1;

pub fn folder(support: &Path) -> PathBuf {
    support.join(FOLDER)
}

pub fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
}

/// SHA-256 of a file's bytes, in hex: what a ledger line and a backup are matched on.
pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// One camera's measured shift, kept so every later frame of it, on a disk plugged in later too,
/// gets the very same correction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraRecord {
    pub make: String,
    pub model: String,
    /// Apple over LibRaw per channel, in stops.
    pub stops: [f64; 3],
    /// The frames the shift was measured on.
    pub frames: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub version: i64,
    /// Unix seconds: a sidecar last written before this instant was written by the earlier app.
    #[serde(rename = "installedAt")]
    pub installed_at: f64,
    pub cameras: Vec<CameraRecord>,
    /// Roots that could not be reached, looked at again when they come back.
    pub deferred: Vec<String>,
}

impl State {
    pub fn path(support: &Path) -> PathBuf {
        folder(support).join(STATE)
    }

    pub fn load(support: &Path) -> Option<State> {
        std::fs::read(Self::path(support)).ok().and_then(|d| serde_json::from_slice(&d).ok())
    }

    /// The state on file, else a new one installed now and written at once, so the instant holds.
    pub fn begin(support: &Path) -> std::io::Result<State> {
        if let Some(state) = Self::load(support) {
            return Ok(state);
        }
        if Self::path(support).exists() {
            return Err(std::io::Error::other("the transition's state is unreadable"));
        }
        let state = State { version: VERSION, installed_at: now(), cameras: Vec::new(), deferred: Vec::new() };
        state.save(support)?;
        Ok(state)
    }

    pub fn save(&self, support: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(folder(support))?;
        let text = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        oni_library::write_atomically(&Self::path(support), &text)
    }

    pub fn camera(&self, make: &str, model: &str) -> Option<&CameraRecord> {
        self.cameras.iter().find(|c| c.make == make && c.model == model)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    /// Shifted once; never looked at again.
    Corrected,
    /// Left as it is for good: a newer format, RGB passes, a DNG.
    Skipped,
    /// Left as it is for now, tried again at the next run.
    Failed,
}

/// One ledger line: a sidecar and what the transition did to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub sidecar: String,
    pub outcome: Outcome,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reason: Option<String>,
    /// Digest of the bytes found, and of the bytes written.
    pub before: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub stops: Option<[f64; 3]>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub backup: Option<String>,
    pub at: f64,
}

/// Appends one JSON line and hands it to the drive: a crash tears at most the last line, never one before.
fn append(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut line = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    line.push(b'\n');
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(&line)?;
    file.sync_all()
}

/// Every line that reads, a torn or foreign one passed over.
fn lines<T: for<'de> Deserialize<'de>>(path: &Path) -> Vec<T> {
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// The ledger by sidecar, its last line for each.
#[derive(Debug, Default)]
pub struct Ledger {
    pub entries: HashMap<String, Entry>,
}

impl Ledger {
    pub fn path(support: &Path) -> PathBuf {
        folder(support).join(LEDGER)
    }

    pub fn load(support: &Path) -> Ledger {
        Ledger { entries: lines::<Entry>(&Self::path(support)).into_iter().map(|e| (e.sidecar.clone(), e)).collect() }
    }

    pub fn record(&mut self, support: &Path, entry: Entry) -> std::io::Result<()> {
        append(&Self::path(support), &entry)?;
        self.entries.insert(entry.sidecar.clone(), entry);
        Ok(())
    }

    /// Settled for good: corrected or skipped. A failure is tried again.
    pub fn settled(&self, sidecar: &str) -> bool {
        self.entries.get(sidecar).is_some_and(|e| e.outcome != Outcome::Failed)
    }
}

/// One manifest line: a backup's file under `backup/` and the sidecar it restores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Backup {
    pub backup: String,
    pub original: String,
    pub digest: String,
    pub at: f64,
}

pub fn backup_folder(support: &Path) -> PathBuf {
    folder(support).join(BACKUP)
}

pub fn manifest(support: &Path) -> Vec<Backup> {
    lines(&backup_folder(support).join(MANIFEST))
}

/// Keeps `bytes`, the sidecar as found, under a name of its digest, then maps it to `original`.
pub fn back_up(support: &Path, original: &Path, bytes: &[u8]) -> std::io::Result<String> {
    let digest = digest(bytes);
    let name = format!("{}.json", &digest[..32]);
    let folder = backup_folder(support);
    std::fs::create_dir_all(&folder)?;
    let kept = folder.join(&name);
    if std::fs::read(&kept).ok().as_deref() != Some(bytes) {
        oni_library::write_atomically(&kept, bytes)?;
    }
    let line = Backup { backup: name.clone(), original: original.to_string_lossy().into_owned(), digest, at: now() };
    append(&folder.join(MANIFEST), &line)?;
    Ok(name)
}

#[cfg(test)]
#[path = "records_tests.rs"]
mod tests;
