//! The paths an index file lists, whatever its schema: the only list of a person's photos an older
//! build may have left, read without writing anything into it.

use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

/// Every `photos.path` of the index at `file`, opened read-only; none when it does not open.
pub fn listed(file: &Path) -> Vec<String> {
    let Ok(connection) = Connection::open_with_flags(file, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX) else { return Vec::new() };
    let Ok(mut statement) = connection.prepare("SELECT path FROM photos") else { return Vec::new() };
    statement.query_map([], |row| row.get::<_, String>(0)).map(|rows| rows.flatten().collect()).unwrap_or_default()
}

/// Every index file of the caches folder: `index.sqlite` and each `index-<schema>.sqlite`.
pub fn files(caches: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(caches) else { return Vec::new() };
    let mut out: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("index") && n.ends_with(".sqlite")))
        .collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_index_lists_its_paths_and_is_left_as_it_was() {
        let dir = std::env::temp_dir().join(format!("oni-index-listed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("index.sqlite");
        let c = Connection::open(&file).unwrap();
        c.execute_batch("CREATE TABLE photos (path TEXT); INSERT INTO photos VALUES ('/a/1.NEF'), ('/b/2.NEF');").unwrap();
        drop(c);
        let before = std::fs::read(&file).unwrap();
        assert_eq!(listed(&file), ["/a/1.NEF", "/b/2.NEF"]);
        assert_eq!(std::fs::read(&file).unwrap(), before, "read-only");
        std::fs::write(dir.join("index-10.sqlite"), b"").unwrap();
        std::fs::write(dir.join("thumbnails.db"), b"").unwrap();
        assert_eq!(files(&dir), [dir.join("index-10.sqlite"), file.clone()]);
        // Twin: a file that is no index lists nothing.
        assert!(listed(&dir.join("index-10.sqlite")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
