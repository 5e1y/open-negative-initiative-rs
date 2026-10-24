//! The rows a launch takes out of the cache: a file gone from a folder the boot disk still lists. A
//! volume's rows wait for its folder's read: a disk away, or another under its name, keeps them all.

use crate::navigator::{Computer, is_drive_root};
use crate::paths::{self, holds};
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;

/// Each path whose file is gone from a disk that answers. A folder gone keeps its rows when it is
/// `kept` or under one (the opened folder, a favourite), so Locate and Forget still find them.
pub fn dead(paths: &[String], computer: &Computer, kept: &[String]) -> Vec<String> {
    let mut looks = Looks::default();
    paths.iter().filter(|p| looks.dead(p, computer, kept)).cloned().collect()
}

/// Whether `path` is still gone, the look `forget_rows` repeats inside its transaction.
pub fn still_gone(path: &str) -> bool {
    crate::presence::looked(path);
    std::fs::symlink_metadata(path).is_err_and(|e| e.kind() == ErrorKind::NotFound)
}

/// What the disk said of each folder looked at once, so a folder of thousands is listed once.
#[derive(Default)]
struct Looks {
    /// `Some(true)` a folder that lists, `Some(false)` one gone, `None` one the disk refuses.
    folders: HashMap<String, Option<bool>>,
}

impl Looks {
    fn dead(&mut self, path: &str, computer: &Computer, kept: &[String]) -> bool {
        // A volume, a drive or a share may be another disk under the same name: its rows wait for its read.
        if computer.volumes.is_empty() || computer.is_on_volume(path) || path.starts_with("//") {
            return false;
        }
        if !still_gone(path) {
            return false;
        }
        let parent = paths::parent(path);
        match self.folder(&parent) {
            Some(true) => true,
            None => false,
            // Its folder gone too: the nearest folder above must list, and be no disk's root.
            Some(false) => {
                if kept.iter().any(|k| holds(k, path)) {
                    return false;
                }
                let mut at = parent;
                loop {
                    let up = paths::parent(&at);
                    if up == at || up == "/" || is_drive_root(&up) || up.is_empty() {
                        return false;
                    }
                    match self.folder(&up) {
                        Some(true) => return true,
                        None => return false,
                        Some(false) => at = up,
                    }
                }
            }
        }
    }

    fn folder(&mut self, folder: &str) -> Option<bool> {
        if let Some(known) = self.folders.get(folder) {
            return *known;
        }
        crate::presence::looked(folder);
        let answer = match std::fs::read_dir(Path::new(folder)) {
            Ok(_) => Some(true),
            Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => Some(false),
            Err(_) => None,
        };
        self.folders.insert(folder.to_owned(), answer);
        answer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    #[test]
    fn a_dead_row_on_a_present_disk_is_pruned_and_one_away_is_kept() {
        let scratch = Scratch::new("prune");
        let root = paths::canonical(scratch.path());
        let computer = Computer { home: paths::join(&root, "Home"), volumes: paths::join(&root, "Volumes") };
        let at = |rel: &str| paths::join(&root, rel);
        for folder in ["Home/Scans/Roll 1", "Home/Downloads", "Volumes/T7/Film"] {
            std::fs::create_dir_all(at(folder)).unwrap();
        }
        for file in ["Home/Scans/Roll 1/A.NEF", "Volumes/T7/Film/C.NEF"] {
            std::fs::write(at(file), b"raw").unwrap();
        }
        let rows: Vec<String> = [
            "Home/Scans/Roll 1/A.NEF",     // there
            "Home/Scans/Roll 1/B.NEF",     // deleted from a folder that lists
            "Home/Downloads/svg/icon.svg", // its folder deleted, the one above lists
            "Home/Gone/Roll 9/D.NEF",      // a favourite gone: Locate and Forget need its rows
            "Volumes/T7/Film/C.NEF",       // there, on a volume
            "Volumes/T7/Film/E.NEF",       // gone from a volume mounted: its folder's read decides
            "Volumes/T8/Film/F.NEF",       // a volume away
        ]
        .iter()
        .map(|r| at(r))
        .collect();
        let kept = [at("Home/Gone/Roll 9")];
        let pruned = dead(&rows, &computer, &kept);
        assert_eq!(pruned, [at("Home/Scans/Roll 1/B.NEF"), at("Home/Downloads/svg/icon.svg")]);
        // Twin: with no volume folder known every path reads as the boot disk's, and the rows on the
        // volumes would go too; a volume's rows are spared only by knowing it is one.
        let blind = Computer { home: computer.home.clone(), volumes: "/nowhere".into() };
        let would = dead(&rows, &blind, &kept);
        assert!(would.contains(&at("Volumes/T7/Film/E.NEF")) && would.contains(&at("Volumes/T8/Film/F.NEF")), "{would:?}");
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_the_disk_refuses_keeps_its_rows() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = Scratch::new("prune-denied");
        let root = paths::canonical(scratch.path());
        let computer = Computer { home: root.clone(), volumes: paths::join(&root, "Volumes") };
        let locked = paths::join(&root, "Locked");
        std::fs::create_dir_all(&locked).unwrap();
        let row = paths::join(&locked, "A.NEF");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        let refused = dead(std::slice::from_ref(&row), &computer, &[]);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(refused.is_empty(), "a folder that refuses says nothing of its files");
        // Twin: open again, the same folder lists and the file is plainly gone.
        assert_eq!(dead(std::slice::from_ref(&row), &computer, &[]), [row]);
    }
}
