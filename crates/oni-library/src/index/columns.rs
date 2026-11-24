//! One column of a row written in place, its other columns left alone: a gesture's sidecar write, or
//! the roll its folder's marker names, cached so no count reads the disk.

use super::{LibraryIndex, medal_raw, reference};
use crate::paths::{self, normal};
use oni_settings::{Date, Film, Medal};
use rusqlite::params;

impl LibraryIndex {
    pub fn set_trashed(&self, date: Option<Date>, path: &str) -> rusqlite::Result<()> {
        self.connection
            .execute("UPDATE photos SET trashed_at = ? WHERE path = ?", params![date.map(reference), normal(path)])?;
        Ok(())
    }

    pub fn set_passes(&self, pass_of: Option<&str>, leads: bool, path: &str) -> rusqlite::Result<()> {
        self.connection.execute(
            "UPDATE photos SET pass_of = ?, leads_passes = ? WHERE path = ?",
            params![pass_of, i64::from(leads), normal(path)],
        )?;
        Ok(())
    }

    /// A `None` roll is the whole of ungrouping.
    pub fn set_film(&self, film: Option<&Film>, path: &str) -> rusqlite::Result<()> {
        self.connection.execute(
            "UPDATE photos SET film_id = ?, film_name = ?, film_order = ?, film_stock = ?, \
             film_shot_date = ?, film_camera_body = ?, film_lens = ? WHERE path = ?",
            params![
                film.map(|f| f.id.to_string()),
                film.map(|f| f.name.clone()),
                film.and_then(|f| f.manual_order),
                film.and_then(|f| f.stock.clone()),
                film.and_then(|f| f.shot_date).map(reference),
                film.and_then(|f| f.camera_body.clone()),
                film.and_then(|f| f.lens.clone()),
                normal(path)
            ],
        )?;
        Ok(())
    }

    pub fn set_label(&self, label: Medal, path: &str) -> rusqlite::Result<()> {
        self.connection.execute("UPDATE photos SET label = ? WHERE path = ?", params![medal_raw(label), normal(path)])?;
        Ok(())
    }

    /// The rows right inside `folder` take the roll its marker now names, `None` when it names none.
    pub fn set_marker(&self, folder: &str, film: Option<&Film>) -> rusqlite::Result<()> {
        let gear = marker_gear(film);
        let inside: Vec<(String, MarkerGear)> =
            self.paths_under(folder).into_iter().filter(|p| paths::same(&paths::parent(p), folder)).map(|p| (p, gear.clone())).collect();
        self.set_marker_gear(&inside)
    }

    /// Each row's marker body and lens, in one transaction.
    pub fn set_marker_gear(&self, rows: &[(String, MarkerGear)]) -> rusqlite::Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        self.in_one(|| {
            rows.iter().try_for_each(|(path, (body, lens))| {
                self.connection
                    .execute("UPDATE photos SET marker_body = ?, marker_lens = ? WHERE path = ?", params![body, lens, normal(path)])
                    .map(|_| ())
            })
        })
    }
}

/// A folder roll's body and lens as its rows cache them, both `None` without a roll, empty where it names none.
pub type MarkerGear = (Option<String>, Option<String>);

pub fn marker_gear(film: Option<&Film>) -> MarkerGear {
    (film.map(|f| f.camera_body.clone().unwrap_or_default()), film.map(|f| f.lens.clone().unwrap_or_default()))
}
