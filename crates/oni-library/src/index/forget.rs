//! The rows under a folder, read and forgotten as one: Remove from Library takes a folder gone out
//! of the cache alone, its sidecars and originals never touched.

use super::{COLUMNS, Entry, LibraryIndex, entry};
use crate::paths::{holds, normal};
use std::collections::HashSet;

/// The `LIKE` pattern every path at `folder` or under it matches: its ASCII head, which `LIKE` folds
/// as a case-blind volume does and every composition spells alike; `holds` then trims the rest.
pub(super) fn head_pattern(folder: &str) -> String {
    let folder = normal(folder);
    let head = &folder[..folder.find(|c: char| !c.is_ascii()).unwrap_or(folder.len())];
    let mut pattern: String = head.chars().flat_map(|c| if matches!(c, '\\' | '%' | '_') { vec!['\\', c] } else { vec![c] }).collect();
    pattern.push('%');
    pattern
}

impl LibraryIndex {
    /// Every row at `folder` or under it, in `all`'s order, whichever spelling holds it; the rows of
    /// other folders are left in SQLite, never read into rows.
    pub fn under(&self, folder: &str) -> Vec<Entry> {
        let sql = format!("SELECT {COLUMNS} FROM photos WHERE path LIKE ? ESCAPE '\\' ORDER BY COALESCE(captured_at, imported_at) ASC, path ASC");
        let Ok(mut statement) = self.connection.prepare_cached(&sql) else { return Vec::new() };
        let Ok(rows) = statement.query_map([head_pattern(folder)], entry) else { return Vec::new() };
        rows.filter_map(Result::ok).filter(|e| holds(folder, &e.path)).collect()
    }

    /// The paths of `under`'s rows alone.
    pub fn paths_under(&self, folder: &str) -> HashSet<String> {
        let Ok(mut statement) = self.connection.prepare_cached("SELECT path FROM photos WHERE path LIKE ? ESCAPE '\\'") else { return HashSet::new() };
        let Ok(rows) = statement.query_map([head_pattern(folder)], |r| r.get::<_, String>(0)) else { return HashSet::new() };
        rows.filter_map(Result::ok).filter(|p| holds(folder, p)).collect()
    }

    /// Every path the index holds, no row read whole.
    pub fn paths(&self) -> HashSet<String> {
        let Ok(mut statement) = self.connection.prepare_cached("SELECT path FROM photos") else { return HashSet::new() };
        let Ok(rows) = statement.query_map([], |r| r.get::<_, String>(0)) else { return HashSet::new() };
        rows.filter_map(Result::ok).collect()
    }

    /// The paths whose fingerprint starts with `mark`.
    pub fn marked(&self, mark: &str) -> Vec<String> {
        let Ok(mut statement) = self.connection.prepare_cached("SELECT path FROM photos WHERE substr(fingerprint, 1, length(?1)) = ?1") else { return Vec::new() };
        let Ok(rows) = statement.query_map([mark], |r| r.get::<_, String>(0)) else { return Vec::new() };
        rows.filter_map(Result::ok).collect()
    }

    /// The rows under `folder` taken out of the index in one transaction, all or none; the rows
    /// taken, so their thumbnails can go once no other row claims their fingerprint.
    pub fn forget(&self, folder: &str) -> rusqlite::Result<Vec<Entry>> {
        let gone = self.under(folder);
        self.in_one(|| gone.iter().try_for_each(|e| self.remove(&e.path)))?;
        Ok(gone)
    }

    /// The rows at `paths` that `still_gone` confirms inside the transaction, taken out all or none:
    /// a file back between the look and the write keeps its row. The rows taken.
    pub fn forget_rows(&self, paths: &[String], still_gone: &dyn Fn(&str) -> bool) -> rusqlite::Result<Vec<Entry>> {
        let mut gone = Vec::new();
        self.in_one(|| {
            for path in paths {
                if let Some(row) = self.row(path).filter(|_| still_gone(path)) {
                    self.remove(path)?;
                    gone.push(row);
                }
            }
            Ok(())
        })?;
        Ok(gone)
    }

    /// Whether any row holds `fingerprint`, as a thumbnail kept for a copy elsewhere is.
    pub fn claims(&self, fingerprint: &str) -> bool {
        !self.with_fingerprint(fingerprint).is_empty()
    }

    /// The fingerprints two rows or more hold: copies of one file, whose tiles are told apart.
    pub fn copies(&self) -> std::collections::HashSet<String> {
        let Ok(mut statement) = self.connection.prepare_cached("SELECT fingerprint FROM photos GROUP BY fingerprint HAVING COUNT(*) > 1") else {
            return Default::default();
        };
        let Ok(rows) = statement.query_map([], |r| r.get::<_, String>(0)) else { return Default::default() };
        rows.filter_map(Result::ok).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::Date;

    fn index(paths: &[(&str, &str)]) -> LibraryIndex {
        let index = LibraryIndex::open(std::path::Path::new(":memory:")).unwrap();
        for (path, print) in paths {
            index.upsert(&Entry::new(path, Date::since_reference_date(8e8), print)).unwrap();
        }
        index
    }

    #[test]
    fn a_folder_forgotten_takes_its_rows_and_leaves_its_sibling_and_a_copy() {
        let index = index(&[("/r/Roll/A.tif", "a"), ("/r/Roll/Sub/B.tif", "b"), ("/r/Rolled/C.tif", "c"), ("/s/A copy.tif", "a")]);
        let gone: Vec<String> = index.forget("/r/Roll").unwrap().into_iter().map(|e| e.path).collect();
        assert_eq!(gone, ["/r/Roll/A.tif", "/r/Roll/Sub/B.tif"]);
        let left: Vec<String> = index.all().into_iter().map(|e| e.path).collect();
        assert_eq!(left, ["/r/Rolled/C.tif", "/s/A copy.tif"]);
        assert!(index.claims("a"), "the copy elsewhere still claims its print");
        assert!(!index.claims("b"));
        // Twin: a folder named like the start of another forgets nothing of it.
        assert!(index.forget("/r/Rol").unwrap().is_empty());
        assert_eq!(index.count(), 2);
    }

    /// A folder's rows read by their head alone answer as the whole index read and trimmed did,
    /// accents, `%` and `_` in the names included.
    #[test]
    fn a_folder_s_rows_are_its_head_s_trimmed() {
        let index = index(&[
            ("/r/\u{c9}t\u{e9} 50%_off/a.tif", "a"),
            ("/r/\u{c9}t\u{e9} 50%_off/Sub/b.tif", "b"),
            ("/r/\u{c9}t\u{e9} 50%_offX/c.tif", "c"),
            ("/r/a_b/d.tif", "d"),
            ("/r/aXb/e.tif", "e"),
            ("/s/f.tif", "f"),
        ]);
        for folder in ["/r/\u{c9}t\u{e9} 50%_off", "/r/a_b", "/r", "/s", "/t"] {
            let whole: Vec<String> = index.all().into_iter().filter(|e| holds(folder, &e.path)).map(|e| e.path).collect();
            let head: Vec<String> = index.under(folder).into_iter().map(|e| e.path).collect();
            assert_eq!(head, whole, "{folder}");
            assert_eq!(index.paths_under(folder), whole.into_iter().collect::<HashSet<_>>(), "{folder}");
        }
        assert_eq!(index.under("/r/a_b").len(), 1, "`_` matches itself alone");
        assert_eq!(head_pattern("/r/a_b%"), "/r/a\\_b\\%%");
        assert_eq!(head_pattern("/r/\u{c9}t\u{e9}"), "/r/%", "the head stops before the accent");
        // Twin: the bare `LIKE` of `_` would take the sibling `aXb` too.
        let bare: i64 = index.connection.query_row("SELECT COUNT(*) FROM photos WHERE path LIKE '/r/a_b/%'", [], |r| r.get(0)).unwrap();
        assert_eq!(bare, 2);
    }

    #[test]
    fn rows_forgotten_are_looked_at_again_inside_the_write() {
        let index = index(&[("/r/A.tif", "a"), ("/r/B.tif", "b"), ("/s/A.tif", "a")]);
        assert_eq!(index.copies(), std::collections::HashSet::from(["a".to_owned()]));
        let paths = ["/r/A.tif".to_owned(), "/r/B.tif".to_owned(), "/r/never.tif".to_owned()];
        let gone = index.forget_rows(&paths, &|p| p != "/r/B.tif").unwrap();
        assert_eq!(gone.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(), ["/r/A.tif"]);
        assert!(index.row("/r/B.tif").is_some(), "a file back before the write keeps its row");
        assert!(index.copies().is_empty(), "one row left holds the print");
        // Twin: without the look again, the row of the file back would go too.
        assert_eq!(index.forget_rows(&paths, &|_| true).unwrap().len(), 1);
        assert!(index.row("/r/B.tif").is_none());
    }
}
