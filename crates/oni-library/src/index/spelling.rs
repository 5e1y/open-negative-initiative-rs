//! The index's keys in the archive's spelling: a row an earlier build keyed as the disk listed it,
//! decomposed or with `\`, takes the spelling the survey hands now, so no row goes for want of a match.

use crate::paths::normal;
use rusqlite::{Connection, params};

/// The columns a row may lack, which the row kept takes from its twin rather than lose them.
const NULLABLE: &[&str] = &["captured_at", "manual_order", "mode", "pass_of", "trashed_at"];

/// The roll's columns, taken together from the twin when the row kept names no roll.
const FILM: &[&str] = &["film_id", "film_name", "film_order", "film_stock", "film_shot_date", "film_camera_body"];

/// Respells every row whose key is not in the archive's spelling; when both spellings hold a row,
/// the one already respelt stays and takes what it lacks from the other. One transaction.
pub(super) fn respell(connection: &Connection) -> rusqlite::Result<()> {
    // Only a key holding a character past ASCII, or a `\`, can be spelt otherwise.
    let stale: Vec<(String, String)> = {
        let mut statement =
            connection.prepare("SELECT path FROM photos WHERE path GLOB '*[^ -~]*' OR instr(path, char(92)) > 0")?;
        let rows = statement.query_map([], |r| r.get::<_, String>(0))?;
        rows.filter_map(Result::ok)
            .filter_map(|path| {
                let spelt = normal(&path).into_owned();
                (spelt != path).then_some((path, spelt))
            })
            .collect()
    };
    if stale.is_empty() {
        return Ok(());
    }
    let mut set: Vec<String> = NULLABLE.iter().map(|c| format!("{c} = COALESCE({c}, (SELECT {c} FROM photos WHERE path = ?2))")).collect();
    set.extend(FILM.iter().map(|c| format!("{c} = CASE WHEN film_id IS NULL THEN (SELECT {c} FROM photos WHERE path = ?2) ELSE {c} END")));
    set.push("label = CASE WHEN label = 'none' THEN (SELECT label FROM photos WHERE path = ?2) ELSE label END".into());
    set.push("leads_passes = MAX(leads_passes, (SELECT leads_passes FROM photos WHERE path = ?2))".into());
    let merge = format!("UPDATE photos SET {} WHERE path = ?1", set.join(", "));
    connection.execute_batch("BEGIN IMMEDIATE")?;
    let done = (|| {
        for (old, spelt) in &stale {
            let held: i64 = connection.query_row("SELECT COUNT(*) FROM photos WHERE path = ?", [spelt], |r| r.get(0))?;
            if held == 0 {
                connection.execute("UPDATE photos SET path = ? WHERE path = ?", params![spelt, old])?;
            } else {
                connection.execute(&merge, params![spelt, old])?;
                connection.execute("DELETE FROM photos WHERE path = ?", [old])?;
            }
        }
        Ok(())
    })();
    match done {
        Ok(()) => connection.execute_batch("COMMIT"),
        Err(e) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}
