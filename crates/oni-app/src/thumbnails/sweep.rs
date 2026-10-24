//! `Thumbnails.removeDead`: the files no row claims, taken out of the durable folder once a launch,
//! off the interface thread; a disk away keeps its rows, hence its tiles.

use super::directory;
use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, SystemTime};

/// Written this recently, a file may be another app's tile for a row it is writing now.
pub const FRESH: Duration = Duration::from_secs(10 * 60);
/// The files one launch removes at most; the rest go at the next.
pub const SWEPT: usize = 5000;

/// The folder's own files that `claimed` does not name, removed: tiles, and the halves a write cut
/// short left; none newer than `FRESH`, at most `budget`. How many went.
pub fn sweep(support: &Path, claimed: &HashSet<String>, budget: usize) -> usize {
    let Ok(listing) = std::fs::read_dir(directory(support)) else { return 0 };
    let now = SystemTime::now();
    let mut removed = 0;
    for listed in listing.flatten() {
        if removed >= budget {
            break;
        }
        let name = listed.file_name().to_string_lossy().into_owned();
        let tile = name.ends_with(".jpg") && !name.starts_with('.');
        let half = name.ends_with(".jpg.part") || (name.starts_with('.') && name.ends_with(".tmp"));
        if !(tile || half) || claimed.contains(&name) {
            continue;
        }
        let Ok(metadata) = listed.metadata() else { continue };
        let old = metadata.modified().ok().and_then(|m| now.duration_since(m).ok()).is_some_and(|age| age >= FRESH);
        let thumbnail = directory(support).join(&name);
        if metadata.file_type().is_file() && old && std::fs::remove_file(&thumbnail).is_ok() {
            removed += 1;
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::{FRESH, SWEPT, sweep};
    use std::time::{Duration, SystemTime};

    fn aged(path: &Path) {
        let then = SystemTime::now() - FRESH - Duration::from_secs(60);
        std::fs::File::options().write(true).open(path).unwrap().set_modified(then).unwrap();
    }

    #[test]
    fn a_thumbnail_no_row_claims_is_swept() {
        let support = std::env::temp_dir().join(format!("oni-app-sweep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&support);
        let data = jpeg(&vec![90u8; 8 * 8 * 4], 8, 8).unwrap();
        for print in ["1-no-date-a", "2-no-date-b", "3-no-date-c"] {
            persist(&support, print, &data).unwrap();
        }
        let folder = directory(&support);
        let (half, other) = (folder.join("4-no-date-d.jpg.part"), folder.join("notes.txt"));
        std::fs::write(&half, b"cut").unwrap();
        std::fs::write(&other, b"not a tile").unwrap();
        for name in ["1-no-date-a.jpg", "2-no-date-b.jpg", "4-no-date-d.jpg.part", "notes.txt"] {
            aged(&folder.join(name));
        }
        let claimed: HashSet<String> = claims("1-no-date-a", "1-no-date-a").into_iter().collect();
        assert_eq!(sweep(&support, &claimed, SWEPT), 2, "the unclaimed tile and the cut write go");
        assert!(is_held(&support, "1-no-date-a") && !is_held(&support, "2-no-date-b") && !half.exists());
        assert!(other.exists(), "a file the folder does not make stays");
        // Twin: a tile written moments ago, unclaimed, stays for the row another app is writing.
        assert!(is_held(&support, "3-no-date-c"));
        // Twin: an old tile a row claims stays, as the first one did.
        persist(&support, "5-no-date-e", &data).unwrap();
        aged(&file(&support, "5-no-date-e"));
        let both: HashSet<String> = claimed.into_iter().chain(claims("5-no-date-e", "5-no-date-e")).collect();
        assert_eq!(sweep(&support, &both, SWEPT), 0);
        assert!(is_held(&support, "5-no-date-e"));
        let _ = std::fs::remove_dir_all(&support);
    }
}
