//! A cache, disposable without loss: rebuildable from the sidecars, which stay the only truth.
//! Bumping the schema version throws the table away and rebuilds it, never a migration.

use crate::paths::normal;
use oni_settings::{ConversionMode, Date, Film, Id, Medal};
use rusqlite::{Connection, OptionalExtension, Row, params};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: i32 = 9;

/// One member of the library as the gallery cuts it, cached from its file and its sidecar.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub path: String,
    pub imported_at: Date,
    pub captured_at: Option<Date>,
    pub fingerprint: String,
    pub label: Medal,
    /// The roll the sidecar names, cached so cutting the gallery opens no sidecar.
    pub film: Option<Film>,
    /// A hand-placed position among its siblings; `None` sorts by capture.
    pub manual_order: Option<f64>,
    /// Cached at import and on relink alone, a decorative default allowed to drift stale.
    pub mode: Option<ConversionMode>,
    pub pass_of: Option<String>,
    pub leads_passes: bool,
    pub trashed_at: Option<Date>,
}

impl Entry {
    pub fn new(path: &str, imported_at: Date, fingerprint: &str) -> Entry {
        Entry {
            path: path.to_owned(),
            imported_at,
            captured_at: None,
            fingerprint: fingerprint.to_owned(),
            label: Medal::None,
            film: None,
            manual_order: None,
            mode: None,
            pass_of: None,
            leads_passes: false,
            trashed_at: None,
        }
    }
}

/// `Medal`'s raw value, the text the `label` column holds.
pub fn medal_raw(label: Medal) -> &'static str {
    match label {
        Medal::None => "none",
        Medal::Bronze => "bronze",
        Medal::Silver => "silver",
        Medal::Gold => "gold",
    }
}

pub fn medal_from_raw(raw: &str) -> Option<Medal> {
    [Medal::None, Medal::Bronze, Medal::Silver, Medal::Gold].into_iter().find(|m| medal_raw(*m) == raw)
}

fn reference(date: Date) -> f64 {
    date.time_interval_since_reference_date()
}

pub struct LibraryIndex {
    connection: Connection,
}

const COLUMNS: &str = "path, imported_at, captured_at, fingerprint, label, film_id, film_name, \
    film_order, film_stock, film_shot_date, film_camera_body, manual_order, mode, pass_of, \
    leads_passes, trashed_at";

impl LibraryIndex {
    /// Lives in the caches folder, which the system may purge; `ONI_INDEX` gives a probe its own.
    pub fn default_path(caches: &Path) -> PathBuf {
        std::env::var_os("ONI_INDEX").map(PathBuf::from).unwrap_or_else(|| caches.join("index.sqlite"))
    }

    /// Opens or creates the index at `path` (`:memory:` for one in memory); `None` when SQLite refuses.
    pub fn open(path: &Path) -> Option<LibraryIndex> {
        let memory = path.as_os_str() == ":memory:";
        if !memory && let Some(folder) = path.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        let connection = if memory { Connection::open_in_memory() } else { Connection::open(path) }.ok()?;
        // WAL lets background thumbnail writes proceed without blocking gallery reads.
        let _ = connection.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()));
        let _ = connection.execute_batch("PRAGMA synchronous = NORMAL");
        let index = LibraryIndex { connection };
        index.prepare_schema().ok()?;
        // A busy database is respelt at the next open; meanwhile each write names both spellings.
        let _ = spelling::respell(&index.connection);
        Some(index)
    }

    fn prepare_schema(&self) -> rusqlite::Result<()> {
        let version: i32 = self.connection.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap_or(-1);
        if version != SCHEMA_VERSION {
            self.connection.execute_batch("DROP TABLE IF EXISTS photos")?;
            self.connection.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))?;
        }
        self.connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS photos (
                path TEXT PRIMARY KEY NOT NULL,
                imported_at REAL NOT NULL,
                captured_at REAL,
                fingerprint TEXT NOT NULL,
                label TEXT NOT NULL DEFAULT 'none',
                film_id TEXT,
                film_name TEXT,
                film_order REAL,
                film_stock TEXT,
                film_shot_date REAL,
                film_camera_body TEXT,
                manual_order REAL,
                mode TEXT,
                pass_of TEXT,
                leads_passes INTEGER NOT NULL DEFAULT 0,
                trashed_at REAL
            );
            CREATE INDEX IF NOT EXISTS photos_imported_at ON photos(imported_at);
            CREATE INDEX IF NOT EXISTS photos_captured_at ON photos(captured_at);
            CREATE INDEX IF NOT EXISTS photos_fingerprint ON photos(fingerprint);
            CREATE INDEX IF NOT EXISTS photos_label ON photos(label);",
        )
    }

    // MARK: - Writing

    pub fn upsert(&self, entry: &Entry) -> rusqlite::Result<()> {
        let film = entry.film.as_ref();
        self.connection
            .prepare_cached(&format!(
                "INSERT INTO photos ({COLUMNS})
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(path) DO UPDATE SET
                    imported_at = excluded.imported_at,
                    captured_at = excluded.captured_at,
                    fingerprint = excluded.fingerprint,
                    label = excluded.label,
                    film_id = excluded.film_id,
                    film_name = excluded.film_name,
                    film_order = excluded.film_order,
                    film_stock = excluded.film_stock,
                    film_shot_date = excluded.film_shot_date,
                    film_camera_body = excluded.film_camera_body,
                    manual_order = excluded.manual_order,
                    mode = excluded.mode,
                    pass_of = excluded.pass_of,
                    leads_passes = excluded.leads_passes,
                    trashed_at = excluded.trashed_at"
            ))?
            .execute(params![
                normal(&entry.path),
                reference(entry.imported_at),
                entry.captured_at.map(reference),
                entry.fingerprint,
                medal_raw(entry.label),
                film.map(|f| f.id.to_string()),
                film.map(|f| f.name.clone()),
                film.and_then(|f| f.manual_order),
                film.and_then(|f| f.stock.clone()),
                film.and_then(|f| f.shot_date).map(reference),
                film.and_then(|f| f.camera_body.clone()),
                entry.manual_order,
                entry.mode.map(ConversionMode::raw_value),
                entry.pass_of,
                i64::from(entry.leads_passes),
                entry.trashed_at.map(reference),
            ])?;
        Ok(())
    }

    pub fn set_trashed(&self, date: Option<Date>, path: &str) -> rusqlite::Result<()> {
        self.connection
            .execute("UPDATE photos SET trashed_at = ? WHERE path IN (?, ?)", params![date.map(reference), path, normal(path)])?;
        Ok(())
    }

    pub fn set_passes(&self, pass_of: Option<&str>, leads: bool, path: &str) -> rusqlite::Result<()> {
        self.connection.execute(
            "UPDATE photos SET pass_of = ?, leads_passes = ? WHERE path IN (?, ?)",
            params![pass_of, i64::from(leads), path, normal(path)],
        )?;
        Ok(())
    }

    /// A `None` roll is the whole of ungrouping.
    pub fn set_film(&self, film: Option<&Film>, path: &str) -> rusqlite::Result<()> {
        self.connection.execute(
            "UPDATE photos SET film_id = ?, film_name = ?, film_order = ?, film_stock = ?, \
             film_shot_date = ?, film_camera_body = ? WHERE path IN (?, ?)",
            params![
                film.map(|f| f.id.to_string()),
                film.map(|f| f.name.clone()),
                film.and_then(|f| f.manual_order),
                film.and_then(|f| f.stock.clone()),
                film.and_then(|f| f.shot_date).map(reference),
                film.and_then(|f| f.camera_body.clone()),
                path,
                normal(path)
            ],
        )?;
        Ok(())
    }

    pub fn set_label(&self, label: Medal, path: &str) -> rusqlite::Result<()> {
        self.connection.execute("UPDATE photos SET label = ? WHERE path IN (?, ?)", params![medal_raw(label), path, normal(path)])?;
        Ok(())
    }

    pub fn set_manual_order(&self, order: Option<f64>, path: &str) -> rusqlite::Result<()> {
        self.connection.execute("UPDATE photos SET manual_order = ? WHERE path IN (?, ?)", params![order, path, normal(path)])?;
        Ok(())
    }

    pub fn remove(&self, path: &str) -> rusqlite::Result<()> {
        self.connection.execute("DELETE FROM photos WHERE path IN (?, ?)", params![path, normal(path)])?;
        Ok(())
    }

    /// One transaction: without it each insert pays its own `fsync`, and a rebuild of ten
    /// thousand frames takes minutes instead of under a second.
    pub fn rebuild(&self, entries: &[Entry]) -> rusqlite::Result<()> {
        self.connection.execute_batch("BEGIN IMMEDIATE")?;
        let written = (|| {
            self.connection.execute_batch("DELETE FROM photos")?;
            for entry in entries {
                self.upsert(entry)?;
            }
            Ok(())
        })();
        match written {
            Ok(()) => self.connection.execute_batch("COMMIT"),
            Err(e) => {
                let _ = self.connection.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    // MARK: - Reading

    /// Capture time ascending, an undated frame on its import time, the order `Film::day_start` cuts on.
    pub fn all(&self) -> Vec<Entry> {
        let sql = format!("SELECT {COLUMNS} FROM photos ORDER BY COALESCE(captured_at, imported_at) ASC, path ASC");
        let Ok(mut statement) = self.connection.prepare_cached(&sql) else { return Vec::new() };
        let Ok(rows) = statement.query_map([], entry) else { return Vec::new() };
        rows.filter_map(Result::ok).collect()
    }

    /// The ONI Trash's rows alone, in `all`'s order, so the trash is read without the whole index.
    pub fn in_trash(&self) -> Vec<Entry> {
        let sql = format!("SELECT {COLUMNS} FROM photos WHERE trashed_at IS NOT NULL ORDER BY COALESCE(captured_at, imported_at) ASC, path ASC");
        let Ok(mut statement) = self.connection.prepare_cached(&sql) else { return Vec::new() };
        let Ok(rows) = statement.query_map([], entry) else { return Vec::new() };
        rows.filter_map(Result::ok).collect()
    }

    /// The ONI Trash as `trashed(&all())` lists it, whichever folder holds each photo, read
    /// without the whole index: a pass whose lead the index holds is left to its lead.
    pub fn trashed_frames(&self) -> Vec<Entry> {
        let rows = self.in_trash();
        let held = |lead: &str| rows.iter().any(|r| r.path == lead) || self.row(lead).is_some();
        let picked: Vec<&Entry> = rows
            .iter()
            .filter(|e| e.pass_of.as_ref().is_none_or(|lead| !held(&crate::paths::join(&crate::paths::parent(&e.path), lead))))
            .collect();
        latest_first(picked.into_iter().cloned().collect())
    }

    /// The row at `path`, in either spelling the index may hold it under.
    pub fn row(&self, path: &str) -> Option<Entry> {
        let sql = format!("SELECT {COLUMNS} FROM photos WHERE path IN (?, ?) LIMIT 1");
        let mut statement = self.connection.prepare_cached(&sql).ok()?;
        statement.query_row(params![path, normal(path)], entry).optional().ok().flatten()
    }

    /// Every row of one fingerprint, in `all`'s order: the copies of one file.
    pub fn with_fingerprint(&self, fingerprint: &str) -> Vec<Entry> {
        let sql = format!("SELECT {COLUMNS} FROM photos WHERE fingerprint = ? ORDER BY COALESCE(captured_at, imported_at) ASC, path ASC");
        let Ok(mut statement) = self.connection.prepare_cached(&sql) else { return Vec::new() };
        let Ok(rows) = statement.query_map([fingerprint], entry) else { return Vec::new() };
        rows.filter_map(Result::ok).collect()
    }

    /// What the gallery draws: every member but the ONI Trash's and the passes a lead composes.
    pub fn shown(&self) -> Vec<Entry> {
        shown(&self.all())
    }

    pub fn count(&self) -> usize {
        self.connection
            .query_row("SELECT COUNT(*) FROM photos", [], |r| r.get::<_, i64>(0))
            .optional()
            .ok()
            .flatten()
            .unwrap_or(0) as usize
    }
}

fn entry(row: &Row<'_>) -> rusqlite::Result<Entry> {
    let date = |i: usize| -> rusqlite::Result<Option<Date>> {
        Ok(row.get::<_, Option<f64>>(i)?.map(Date::since_reference_date))
    };
    // Both columns or neither: a row holding an id it cannot name would draw an empty heading.
    let film = match row.get::<_, Option<String>>(5)?.and_then(|text| Id::parse(&text)) {
        Some(id) => Some(Film {
            id,
            name: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
            manual_order: row.get(7)?,
            stock: row.get(8)?,
            shot_date: date(9)?,
            camera_body: row.get(10)?,
        }),
        None => None,
    };
    Ok(Entry {
        path: row.get(0)?,
        imported_at: Date::since_reference_date(row.get(1)?),
        captured_at: date(2)?,
        fingerprint: row.get(3)?,
        label: row.get::<_, Option<String>>(4)?.as_deref().and_then(medal_from_raw).unwrap_or_default(),
        film,
        manual_order: row.get(11)?,
        mode: row.get::<_, Option<String>>(12)?.as_deref().and_then(ConversionMode::from_raw),
        pass_of: row.get(13)?,
        leads_passes: row.get::<_, i64>(14)? != 0,
        trashed_at: date(15)?,
    })
}

/// Every row but the ONI Trash's and the passes whose lead the library holds; a pass whose lead
/// is gone shows again, so no file vanishes for want of its frame.
pub fn shown(rows: &[Entry]) -> Vec<Entry> {
    let picked: Vec<&Entry> = rows.iter().filter(|e| e.trashed_at.is_none()).collect();
    frames(&picked, rows)
}

/// The ONI Trash's rows, a triplet as its lead, latest deletion first.
pub fn trashed(rows: &[Entry]) -> Vec<Entry> {
    let picked: Vec<&Entry> = rows.iter().filter(|e| e.trashed_at.is_some()).collect();
    latest_first(frames(&picked, rows))
}

fn latest_first(mut out: Vec<Entry>) -> Vec<Entry> {
    let at = |e: &Entry| e.trashed_at.map(crate::seconds).unwrap_or(crate::DISTANT_PAST_SECONDS);
    out.sort_by(|a, b| at(b).total_cmp(&at(a)));
    out
}

fn frames(picked: &[&Entry], rows: &[Entry]) -> Vec<Entry> {
    let held: HashSet<&str> = rows.iter().map(|e| e.path.as_str()).collect();
    picked
        .iter()
        .filter(|entry| match &entry.pass_of {
            None => true,
            Some(lead) => !held.contains(crate::paths::join(&crate::paths::parent(&entry.path), lead).as_str()),
        })
        .map(|e| (*e).clone())
        .collect()
}

mod spelling;

#[cfg(test)]
mod tests;
