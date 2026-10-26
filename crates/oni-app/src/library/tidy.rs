//! Once a launch, after the first read and off the interface thread: the rows of files gone from a
//! disk that answers taken out of the index, then the thumbnails no row claims swept.

use crate::thumbnails;
use oni_library::index::{LibraryIndex, prune};
use oni_library::navigator::Computer;
use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, SystemTime};

/// How long an index must have lived before its rows decide the sweep: one the system purged and
/// rebuilt claims only what was read since, and would sweep a library's tiles.
pub const SETTLED: Duration = Duration::from_secs(24 * 60 * 60);

/// What a tidy did: rows taken out, thumbnails swept.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Tidied {
    pub pruned: usize,
    pub swept: usize,
}

/// Prunes the dead rows (`prune::dead`, `kept` sparing the folders Locate and Forget answer for),
/// then sweeps the tiles no row left claims when `sweeping` (the index has `settled`). Reads the disk.
pub fn tidy(index: &LibraryIndex, support: &Path, computer: &Computer, kept: &[String], sweeping: bool) -> Tidied {
    let paths: Vec<String> = index.all().into_iter().map(|e| e.path).collect();
    let dead = prune::dead(&paths, computer, kept);
    let pruned = index.forget_rows(&dead, &prune::still_gone).map_or(0, |gone| gone.len());
    let swept = if sweeping && index.count() > 0 {
        let copies = index.copies();
        let claimed: HashSet<String> =
            index.all().iter().flat_map(|e| thumbnails::claims(&e.fingerprint, &thumbnails::key(&e.fingerprint, &e.path, &copies))).collect();
        thumbnails::sweep(support, &claimed, thumbnails::SWEPT)
    } else {
        0
    };
    Tidied { pruned, swept }
}

/// Whether the index's rows were last thrown away more than `SETTLED` ago, a schema drop included;
/// the file's birth stands in for an index never dated, and unknown is not settled.
pub fn settled(index: &LibraryIndex, index_path: &Path) -> bool {
    let made = index.rebuilt().or_else(|| std::fs::metadata(index_path).and_then(|m| m.created()).ok());
    made.and_then(|m| SystemTime::now().duration_since(m).ok()).is_some_and(|age| age >= SETTLED)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_library::Entry;
    use oni_settings::Date;

    #[test]
    fn a_launch_prunes_the_dead_rows_and_sweeps_only_on_a_settled_index() {
        let root = std::env::temp_dir().join(format!("oni-app-tidy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("Home/Roll")).unwrap();
        std::fs::write(root.join("Home/Roll/A.NEF"), b"a").unwrap();
        let p = |rel: &str| oni_library::paths::canonical(&root.join(rel));
        let at = root.join("Caches/index.sqlite");
        let index = LibraryIndex::open(&at).unwrap();
        let (a, b) = (p("Home/Roll/A.NEF"), oni_library::paths::join(&p("Home/Roll"), "B.NEF"));
        for (path, print) in [(&a, "1-no-date-a"), (&b, "1-no-date-b")] {
            index.upsert(&Entry::new(path, Date::since_reference_date(8e8), print)).unwrap();
        }
        let support = root.join("Support");
        let data = thumbnails::jpeg(&vec![90u8; 8 * 8 * 4], 8, 8).unwrap();
        for print in ["1-no-date-a", "1-no-date-b"] {
            thumbnails::persist(&support, print, &data).unwrap();
            let old = SystemTime::now() - thumbnails::FRESH - Duration::from_secs(60);
            std::fs::File::options().write(true).open(thumbnails::file(&support, print)).unwrap().set_modified(old).unwrap();
        }
        let computer = Computer { home: p("Home"), volumes: p("Volumes") };
        assert_eq!(tidy(&index, &support, &computer, &[], false), Tidied { pruned: 1, swept: 0 }, "an index just made sweeps nothing");
        assert!(index.row(&a).is_some() && index.row(&b).is_none());
        assert!(thumbnails::is_held(&support, "1-no-date-b"), "its tile waits for a settled index");
        assert!(!settled(&index, &at), "an index made now has not settled");
        // Twin: settled, the tile of the row gone is swept and the live one kept.
        assert_eq!(tidy(&index, &support, &computer, &[], true), Tidied { pruned: 0, swept: 1 });
        assert!(thumbnails::is_held(&support, "1-no-date-a") && !thumbnails::is_held(&support, "1-no-date-b"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_sweep_waits_on_the_rebuild_date_not_the_file_birth() {
        let root = std::env::temp_dir().join(format!("oni-app-rebuilt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let at = root.join("index.sqlite");
        let index = LibraryIndex::open(&at).unwrap();
        index.note_rebuilt(SystemTime::now() - 2 * SETTLED).unwrap();
        assert!(settled(&index, &at), "rows kept since long ago settle, whatever the file's birth");
        // Twin: dated now, as a schema drop dates it, the index claims too little to sweep on.
        index.note_rebuilt(SystemTime::now()).unwrap();
        assert!(!settled(&index, &at));
        let _ = std::fs::remove_dir_all(&root);
    }
}
