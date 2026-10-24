//! The size and dates each row was stamped at, in a table of its own the Swift app never reads: a
//! file changed outside ONI with its size kept, or a sidecar edited elsewhere, is read again.

use super::LibraryIndex;
use crate::paths::{holds, normal};
use rusqlite::{Connection, params};
use std::collections::HashMap;

/// What the disk said of a member when its row was written: the original's size and modification
/// date, and its sidecar's date, `None` when it had none.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stamped {
    pub size: i64,
    pub modified: Option<f64>,
    pub sidecar: Option<f64>,
}

impl Stamped {
    /// Whether the original moved since: another size, or another date when both are known.
    pub fn original_moved(&self, size: i64, modified: Option<f64>) -> bool {
        self.size != size || moved(self.modified, modified)
    }

    pub fn sidecar_moved(&self, sidecar: Option<f64>) -> bool {
        self.sidecar.is_some() != sidecar.is_some() || moved(self.sidecar, sidecar)
    }
}

/// Two dates of one file's metadata, read the same way, either side unknown never moving.
fn moved(was: Option<f64>, now: Option<f64>) -> bool {
    matches!((was, now), (Some(a), Some(b)) if (a - b).abs() > 1e-6)
}

pub(super) fn prepare(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS stamps (
            path TEXT PRIMARY KEY NOT NULL,
            size INTEGER NOT NULL,
            modified REAL,
            sidecar REAL
        );",
    )
}

impl LibraryIndex {
    /// Every stamp at `folder` or under it, by path.
    pub fn stamps_under(&self, folder: &str) -> HashMap<String, Stamped> {
        let Ok(mut statement) = self.connection.prepare_cached("SELECT path, size, modified, sidecar FROM stamps") else { return HashMap::new() };
        let Ok(rows) = statement.query_map([], |r| Ok((r.get::<_, String>(0)?, Stamped { size: r.get(1)?, modified: r.get(2)?, sidecar: r.get(3)? }))) else {
            return HashMap::new();
        };
        rows.filter_map(Result::ok).filter(|(p, _)| holds(folder, p)).collect()
    }

    /// The stamps written in one transaction, all or none.
    pub fn note_stamps(&self, stamps: &[(String, Stamped)]) -> rusqlite::Result<()> {
        if stamps.is_empty() {
            return Ok(());
        }
        self.in_one(|| {
            for (path, s) in stamps {
                self.connection
                    .prepare_cached("INSERT OR REPLACE INTO stamps (path, size, modified, sidecar) VALUES (?, ?, ?, ?)")?
                    .execute(params![normal(path), s.size, s.modified, s.sidecar])?;
            }
            Ok(())
        })
    }

    pub(super) fn forget_stamp(&self, path: &str) -> rusqlite::Result<()> {
        self.connection.execute("DELETE FROM stamps WHERE path IN (?, ?)", params![path, normal(path)])?;
        Ok(())
    }
}
