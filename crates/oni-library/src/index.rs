//! A cache, disposable without loss: rebuildable from the sidecars, which stay the only truth.
//! Bumping the schema version throws the table away and rebuilds it, never a migration.

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
                entry.path,
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
            .execute("UPDATE photos SET trashed_at = ? WHERE path = ?", params![date.map(reference), path])?;
        Ok(())
    }

    pub fn set_passes(&self, pass_of: Option<&str>, leads: bool, path: &str) -> rusqlite::Result<()> {
        self.connection.execute(
            "UPDATE photos SET pass_of = ?, leads_passes = ? WHERE path = ?",
            params![pass_of, i64::from(leads), path],
        )?;
        Ok(())
    }

    /// A `None` roll is the whole of ungrouping.
    pub fn set_film(&self, film: Option<&Film>, path: &str) -> rusqlite::Result<()> {
        self.connection.execute(
            "UPDATE photos SET film_id = ?, film_name = ?, film_order = ?, film_stock = ?, \
             film_shot_date = ?, film_camera_body = ? WHERE path = ?",
            params![
                film.map(|f| f.id.to_string()),
                film.map(|f| f.name.clone()),
                film.and_then(|f| f.manual_order),
                film.and_then(|f| f.stock.clone()),
                film.and_then(|f| f.shot_date).map(reference),
                film.and_then(|f| f.camera_body.clone()),
                path
            ],
        )?;
        Ok(())
    }

    pub fn set_label(&self, label: Medal, path: &str) -> rusqlite::Result<()> {
        self.connection.execute("UPDATE photos SET label = ? WHERE path = ?", params![medal_raw(label), path])?;
        Ok(())
    }

    pub fn set_manual_order(&self, order: Option<f64>, path: &str) -> rusqlite::Result<()> {
        self.connection.execute("UPDATE photos SET manual_order = ? WHERE path = ?", params![order, path])?;
        Ok(())
    }

    pub fn remove(&self, path: &str) -> rusqlite::Result<()> {
        self.connection.execute("DELETE FROM photos WHERE path = ?", params![path])?;
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
    let mut out = frames(&picked, rows);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    fn day1() -> Date {
        Date::since_reference_date(800_000_000.0)
    }

    fn at(base: Date, seconds: f64) -> Date {
        crate::later(base, seconds)
    }

    fn roll_entries() -> Vec<Entry> {
        let day1 = day1();
        let batch = at(day1, 10_800.0);
        let day2 = at(day1, 86_400.0);
        let mut b = Entry::new("/negs/b.RW2", batch, "b-100-x");
        b.captured_at = Some(at(day1, 7200.0));
        let mut a = Entry::new("/negs/a.RW2", batch, "a-100-x");
        a.captured_at = Some(at(day1, 3600.0));
        let undated = Entry::new("/negs/sansdate.RW2", batch, "s-100-x");
        let mut c = Entry::new("/negs/c.RW2", day2, "c-100-x");
        c.captured_at = Some(at(day2, 60.0));
        let mut relinked = Entry::new("/negs/relinked.RW2", at(day2, 3600.0), "r-100-x");
        relinked.captured_at = Some(at(day1, 1800.0));
        vec![b, a, undated, c, relinked]
    }

    fn names(index: &LibraryIndex) -> Vec<String> {
        index.all().iter().map(|e| crate::paths::name(&e.path)).collect()
    }

    #[test]
    fn the_index_orders_on_capture_and_rebuilds_whole() {
        let scratch = Scratch::new("index");
        let index = LibraryIndex::open(&scratch.path().join("index.sqlite")).unwrap();
        assert_eq!(index.count(), 0);
        let entries = roll_entries();
        for e in &entries {
            index.upsert(e).unwrap();
        }
        for e in &entries {
            index.upsert(e).unwrap();
        }
        assert_eq!(index.count(), 5, "a second import duplicates nothing");
        assert_eq!(names(&index), ["relinked.RW2", "a.RW2", "b.RW2", "sansdate.RW2", "c.RW2"]);
        index.remove("/negs/b.RW2").unwrap();
        assert_eq!(index.count(), 4);
        index.rebuild(&entries).unwrap();
        assert_eq!(index.all(), {
            let mut sorted = entries.clone();
            let key = |e: &Entry| crate::seconds(e.captured_at.unwrap_or(e.imported_at));
            sorted.sort_by(|a, b| key(a).total_cmp(&key(b)).then_with(|| a.path.cmp(&b.path)));
            sorted
        });

        index.set_label(Medal::Gold, "/negs/a.RW2").unwrap();
        let all = index.all();
        assert!(all.iter().all(|e| (e.label == Medal::Gold) == (e.path == "/negs/a.RW2")));

        let mut roll = Film::new("Portra 400");
        roll.stock = Some("kodak-portra".into());
        roll.shot_date = Some(Date::since_reference_date(700_000_000.0));
        index.set_film(Some(&roll), "/negs/a.RW2").unwrap();
        let joined = index.all().into_iter().find(|e| e.path == "/negs/a.RW2").unwrap();
        assert_eq!(joined.film.as_ref(), Some(&roll));
        // Twin: a namesake is another roll, so the id is what came back.
        assert_ne!(Some(&Film::new("Portra 400")), joined.film.as_ref());
        index.set_film(None, "/negs/a.RW2").unwrap();
        assert!(index.all().iter().all(|e| e.film.is_none()));

        let mut moded = entries.clone();
        moded[0].mode = Some(ConversionMode::Positive);
        moded[1].mode = Some(ConversionMode::Monochrome);
        index.rebuild(&moded).unwrap();
        let mode = |p: &str| index.all().into_iter().find(|e| e.path == p).unwrap().mode;
        assert_eq!(mode("/negs/b.RW2"), Some(ConversionMode::Positive));
        assert_eq!(mode("/negs/a.RW2"), Some(ConversionMode::Monochrome));
        assert_eq!(mode("/negs/c.RW2"), None);
    }

    #[test]
    fn a_triplet_draws_as_its_lead_and_its_passes_show_once_it_is_gone() {
        let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
        let mut lead = Entry::new("/rgb/0821.ARW", day1(), "a");
        lead.leads_passes = true;
        let pass = |p: &str, f: &str| {
            let mut e = Entry::new(p, day1(), f);
            e.pass_of = Some("0821.ARW".into());
            e
        };
        for e in [lead, pass("/rgb/0822.ARW", "b"), pass("/rgb/0823.ARW", "c"), pass("/other/0821-like.ARW", "d")] {
            index.upsert(&e).unwrap();
        }
        let drawn: HashSet<String> = index.shown().into_iter().map(|e| e.path).collect();
        assert_eq!(drawn, HashSet::from(["/other/0821-like.ARW".to_owned(), "/rgb/0821.ARW".to_owned()]));
        index.remove("/rgb/0821.ARW").unwrap();
        // Twin: with the lead gone, the passes show again.
        let drawn: HashSet<String> = index.shown().into_iter().map(|e| e.path).collect();
        assert_eq!(drawn.len(), 3);
    }

    /// A row with every column set to a value no other column holds.
    fn full_row() -> Entry {
        let mut film = Film::new("Roll #8");
        film.manual_order = Some(-4.5);
        film.stock = Some("fujifilm-fujicolor".into());
        film.shot_date = Some(Date::since_reference_date(700_000_001.0));
        film.camera_body = Some("Nikon F3".into());
        Entry {
            path: "/negs/DSC_6432.NEF".into(),
            imported_at: Date::since_reference_date(812_000_002.0),
            captured_at: Some(Date::since_reference_date(811_000_003.0)),
            fingerprint: "30005388-2026:09:14 16:46:19-58c5f1206c22f7cb".into(),
            label: Medal::Silver,
            film: Some(film),
            manual_order: Some(7.25),
            mode: Some(ConversionMode::Monochrome),
            pass_of: Some("DSC_6431.NEF".into()),
            leads_passes: true,
            trashed_at: Some(Date::since_reference_date(813_000_004.0)),
        }
    }

    #[test]
    fn every_column_reads_back_into_its_own_field() {
        let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
        let row = full_row();
        index.upsert(&row).unwrap();
        assert_eq!(index.all(), std::slice::from_ref(&row));
        // Twin: the same row with two same-typed columns crossed is another row, so a crossed read fails above.
        let mut crossed = row.clone();
        crossed.manual_order = row.film.as_ref().and_then(|f| f.manual_order);
        crossed.film.as_mut().unwrap().manual_order = row.manual_order;
        assert_ne!(crossed, row);
        let mut swapped = row.clone();
        swapped.film.as_mut().unwrap().stock = row.film.as_ref().unwrap().camera_body.clone();
        swapped.film.as_mut().unwrap().camera_body = row.film.as_ref().unwrap().stock.clone();
        assert_ne!(swapped, row);

        // Dates are stored as seconds since 2001, as the Swift index stores them.
        let (imported, trashed): (f64, f64) = index
            .connection
            .query_row("SELECT imported_at, trashed_at FROM photos", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!((imported, trashed), (812_000_002.0, 813_000_004.0));
        // Twin: the same instant in seconds since 1970 is not what the column holds.
        assert_ne!(imported, row.imported_at.unix_seconds());

        index.set_passes(None, false, &row.path).unwrap();
        index.set_trashed(None, &row.path).unwrap();
        index.set_manual_order(None, &row.path).unwrap();
        let cleared = index.all().remove(0);
        assert_eq!(
            (cleared.pass_of, cleared.leads_passes, cleared.trashed_at, cleared.manual_order),
            (None, false, None, None)
        );
        assert_eq!(cleared.film, row.film, "a write to one column leaves the roll alone");
    }

    #[test]
    fn a_schema_of_another_version_is_thrown_away() {
        let scratch = Scratch::new("index-version");
        let path = scratch.path().join("index.sqlite");
        {
            let index = LibraryIndex::open(&path).unwrap();
            index.upsert(&Entry::new("/a", day1(), "a")).unwrap();
        }
        // Twin: reopened at the same version, the row is still there.
        assert_eq!(LibraryIndex::open(&path).unwrap().count(), 1);
        Connection::open(&path).unwrap().execute_batch("PRAGMA user_version = 99").unwrap();
        assert_eq!(LibraryIndex::open(&path).unwrap().count(), 0);
    }
}
