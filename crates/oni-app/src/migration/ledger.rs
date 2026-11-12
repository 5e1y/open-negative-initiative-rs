//! `<support>/State/migration.json`: how far this support folder and the sidecars it knows were
//! brought, so each step runs once; nothing needs it to be right, it only saves walking.

use oni_library::state::{self, Read};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use oni_settings::migrate::Refusal;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Moved at every change of the ledger's shape.
pub const VERSION: i64 = 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ledger {
    pub version: i64,
    /// The support folder's layout reached, `migration::SUPPORT` once brought.
    #[serde(default)]
    pub support: i64,
    /// `migrate::last_written` as the import found it: newer at a launch, an older build wrote since.
    #[serde(rename = "formerWritten", default, skip_serializing_if = "Option::is_none")]
    pub former_written: Option<f64>,
    /// The sidecar format a whole sweep last reached; a launch sweeps the library while it is below this build's.
    #[serde(default)]
    pub sidecars: i64,
    /// The places a sweep could not take yet, their disk away: taken again at each launch and when a
    /// disk comes back, dropped once gone from a disk present.
    #[serde(default)]
    pub pending: Vec<String>,
    /// Each sidecar left as it was, by its original's path: said once, taken again only once it changed.
    #[serde(default)]
    pub left: BTreeMap<String, Left>,
    /// The sidecar format the person was shown the sheet of.
    #[serde(default)]
    pub told: i64,
}

/// A sidecar left as it was, why, and its stamp then.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Left {
    pub why: Why,
    /// The sidecar's size and modification date in nanoseconds when it was left.
    pub stamp: Option<(u64, u64)>,
    /// Shown in the notice once.
    #[serde(default)]
    pub said: bool,
}

/// Why a sidecar was left, in the words the notice groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Why {
    /// The disk refused the write, or the file read back did not say what was converted.
    Unwritable,
    /// The sidecar, or its folder's Prepare file, cannot be read.
    Unreadable,
    /// Its original is kept in the cloud: brought once it is downloaded and opened.
    Away,
    /// Its original could not be read for its scan size or its camera.
    Original,
}

impl Why {
    pub fn of(refusal: &Refusal) -> Why {
        match refusal {
            Refusal::Unwritable(_) | Refusal::Unverified | Refusal::Moved => Why::Unwritable,
            Refusal::Unreadable(_) | Refusal::Drafts(_) | Refusal::Newer(_) | Refusal::Foreign => Why::Unreadable,
            Refusal::Away => Why::Away,
            Refusal::ScanUnknown | Refusal::CameraUnknown | Refusal::MatrixUnknown => Why::Original,
        }
    }
}

impl Default for Ledger {
    fn default() -> Self {
        Ledger { version: VERSION, support: 0, former_written: None, sidecars: 0, pending: Vec::new(), left: BTreeMap::new(), told: 0 }
    }
}

/// The size and modification date `left` compares a sidecar by.
pub fn stamp(sidecar: &Path) -> Option<(u64, u64)> {
    let m = std::fs::metadata(sidecar).ok()?;
    let at = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
    Some((m.len(), u64::try_from(at).ok()?))
}

pub fn path(support: &Path) -> PathBuf {
    state::folder(support).join("migration.json")
}

/// The ledger, a fresh one where none was written or it could not be read; `None` for a newer build's.
pub fn read(support: &Path) -> Option<Ledger> {
    match state::read::<Ledger>(&path(support), VERSION) {
        Read::Current(ledger) => Some(ledger),
        Read::Newer(..) => None,
        Read::Absent | Read::Unreadable => Some(Ledger::default()),
    }
}

pub fn write(support: &Path, ledger: &Ledger) -> std::io::Result<()> {
    state::write(&path(support), &Ledger { version: VERSION, ..ledger.clone() })
}

/// One writer of the ledger at a time across the process: the sweep, the gate and the window.
static WRITING: Mutex<()> = Mutex::new(());

/// The ledger read, changed by `change` and written, no other change of this process between; a
/// newer build's left as it is.
pub fn update(support: &Path, change: impl FnOnce(&mut Ledger)) -> std::io::Result<()> {
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let Some(mut ledger) = read(support) else { return Ok(()) };
    change(&mut ledger);
    write(support, &ledger)
}
