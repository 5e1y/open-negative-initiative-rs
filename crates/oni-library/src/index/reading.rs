//! The index read back: every row, the ONI Trash's, one path's, one fingerprint's, and the count.

use super::*;

impl LibraryIndex {
    /// Capture time ascending, an undated frame on its import time, the path breaking a tie.
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
        frames::latest_first(picked.into_iter().cloned().collect())
    }

    /// The row at `path`, keyed in the archive's spelling.
    pub fn row(&self, path: &str) -> Option<Entry> {
        let sql = format!("SELECT {COLUMNS} FROM photos WHERE path = ? LIMIT 1");
        let mut statement = self.connection.prepare_cached(&sql).ok()?;
        statement.query_row(params![normal(path)], entry).optional().ok().flatten()
    }

    /// Every row of one fingerprint, in `all`'s order: the copies of one file.
    pub fn with_fingerprint(&self, fingerprint: &str) -> Vec<Entry> {
        let sql = format!("SELECT {COLUMNS} FROM photos WHERE fingerprint = ? ORDER BY COALESCE(captured_at, imported_at) ASC, path ASC");
        let Ok(mut statement) = self.connection.prepare_cached(&sql) else { return Vec::new() };
        let Ok(rows) = statement.query_map([fingerprint], entry) else { return Vec::new() };
        rows.filter_map(Result::ok).collect()
    }

    /// What the gallery draws: every member but the ONI Trash's, the contact sheets and the passes a lead composes.
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

pub(super) fn entry(row: &Row<'_>) -> rusqlite::Result<Entry> {
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
            lens: row.get(21)?,
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
        sheet: row.get::<_, i64>(16)? != 0,
        kept_camera: row.get(17)?,
        kept_lens: row.get(18)?,
        marker_body: row.get(19)?,
        marker_lens: row.get(22)?,
        sidecar_format: row.get(20)?,
    })
}
