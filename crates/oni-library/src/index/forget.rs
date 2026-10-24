//! The rows under a folder, read and forgotten as one: Remove from Library takes a folder gone out
//! of the cache alone, its sidecars and originals never touched.

use super::{Entry, LibraryIndex};
use crate::paths::holds;

impl LibraryIndex {
    /// Every row at `folder` or under it, in `all`'s order, whichever spelling holds it.
    pub fn under(&self, folder: &str) -> Vec<Entry> {
        self.all().into_iter().filter(|e| holds(folder, &e.path)).collect()
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
