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
        self.connection.execute_batch("BEGIN IMMEDIATE")?;
        let removed = gone.iter().try_for_each(|e| self.remove(&e.path));
        match removed {
            Ok(()) => self.connection.execute_batch("COMMIT")?,
            Err(e) => {
                let _ = self.connection.execute_batch("ROLLBACK");
                return Err(e);
            }
        }
        Ok(gone)
    }

    /// Whether any row holds `fingerprint`, as a thumbnail kept for a copy elsewhere is.
    pub fn claims(&self, fingerprint: &str) -> bool {
        !self.with_fingerprint(fingerprint).is_empty()
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
}
