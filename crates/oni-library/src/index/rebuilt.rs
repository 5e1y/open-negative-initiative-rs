//! When the index's rows were last thrown away, in a table of its own the Swift app never reads: an
//! index rebuilt in an old file claims only what was read since.

use super::LibraryIndex;
use rusqlite::{Connection, params};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(super) fn prepare(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch("CREATE TABLE IF NOT EXISTS rebuilt (at REAL NOT NULL);")
}

/// Whether the photos table is still to make: a file new, or its rows just dropped.
pub(super) fn lacks_photos(connection: &Connection) -> rusqlite::Result<bool> {
    connection.query_row("SELECT NOT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'photos')", [], |r| r.get(0))
}

impl LibraryIndex {
    /// When the rows were last thrown away; `None` when this app never saw the table made.
    pub fn rebuilt(&self) -> Option<SystemTime> {
        let at: f64 = self.connection.query_row("SELECT at FROM rebuilt", [], |r| r.get(0)).ok()?;
        UNIX_EPOCH.checked_add(Duration::try_from_secs_f64(at).ok()?)
    }

    pub fn note_rebuilt(&self, at: SystemTime) -> rusqlite::Result<()> {
        let at = at.duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
        self.connection.execute_batch("DELETE FROM rebuilt")?;
        self.connection.execute("INSERT INTO rebuilt (at) VALUES (?)", params![at])?;
        Ok(())
    }
}
