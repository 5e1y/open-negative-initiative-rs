//! `<support>/State/migration.json`: how far this support folder was brought, so each step runs once.

use oni_library::state::{self, Read};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Moved at every change of the ledger's shape.
pub const VERSION: i64 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ledger {
    pub version: i64,
    /// The support folder's layout reached, `migration::SUPPORT` once brought.
    #[serde(default)]
    pub support: i64,
    /// `migrate::last_written` as the import found it: newer at a launch, an older build wrote since.
    #[serde(rename = "formerWritten", default, skip_serializing_if = "Option::is_none")]
    pub former_written: Option<f64>,
}

impl Default for Ledger {
    fn default() -> Self {
        Ledger { version: VERSION, support: 0, former_written: None }
    }
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
    state::write(&path(support), ledger)
}
