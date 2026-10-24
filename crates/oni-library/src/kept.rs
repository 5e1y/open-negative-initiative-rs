//! The last shape of an opened folder, its subfolders and rolls, kept among the caches: a folder whose
//! disk is away is laid as it was last seen, roll headings and empty folders included.

use crate::roots::Survey;
use oni_settings::{Date, Film};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub folder: String,
    pub folders: Vec<String>,
    pub made: HashMap<String, Date>,
    pub rolls: HashMap<String, Film>,
}

/// The file `folder`'s shape is kept in under `dir`, named by a stable hash of its path.
pub fn file(dir: &Path, folder: &str) -> PathBuf {
    let hash = folder.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    dir.join(format!("{hash:016x}.json"))
}

/// Keeps a whole survey's shape of `folder`; a stopped or refused one keeps the last.
pub fn keep(dir: &Path, folder: &str, survey: &Survey) -> std::io::Result<()> {
    if survey.stopped || survey.unreadable.iter().any(|u| crate::paths::same(u, folder)) {
        return Ok(());
    }
    let shape = Shape { folder: folder.to_owned(), folders: survey.folders.clone(), made: survey.made.clone(), rolls: survey.rolls.clone() };
    let previous = std::fs::read(file(dir, folder)).ok().and_then(|d| serde_json::from_slice::<Shape>(&d).ok());
    if previous.as_ref() == Some(&shape) {
        return Ok(());
    }
    std::fs::create_dir_all(dir)?;
    crate::write_atomically(&file(dir, folder), &serde_json::to_vec(&shape).map_err(std::io::Error::other)?)
}

/// A survey whose folder the disk refused takes the shape last kept for it.
pub fn worn(dir: &Path, folder: &str, survey: &mut Survey) {
    if !survey.unreadable.iter().any(|u| crate::paths::same(u, folder)) {
        return;
    }
    let Some(shape) = std::fs::read(file(dir, folder)).ok().and_then(|d| serde_json::from_slice::<Shape>(&d).ok()) else { return };
    if shape.folder != folder {
        return;
    }
    survey.folders = shape.folders;
    survey.made = shape.made;
    survey.rolls = shape.rolls;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    #[test]
    fn a_folder_refused_wears_the_shape_it_was_last_seen_in() {
        let scratch = Scratch::new("kept");
        let dir = scratch.path().join("Surveys");
        let folder = "/Volumes/T7/Scans";
        let seen = Survey {
            folders: vec![format!("{folder}/Roll 12"), format!("{folder}/Empty")],
            rolls: [(format!("{folder}/Roll 12"), Film::new("Roll 12"))].into(),
            ..Survey::default()
        };
        keep(&dir, folder, &seen).unwrap();
        let mut away = Survey { unreadable: vec![folder.to_owned()], ..Survey::default() };
        worn(&dir, folder, &mut away);
        assert_eq!((away.folders.len(), away.rolls.len()), (2, 1));
        // Twin: a folder that reads keeps what its own walk found.
        let mut read = Survey::default();
        worn(&dir, folder, &mut read);
        assert!(read.folders.is_empty() && read.rolls.is_empty());
        // A refused survey never overwrites the shape kept.
        keep(&dir, folder, &away_bare(folder)).unwrap();
        let mut again = away_bare(folder);
        worn(&dir, folder, &mut again);
        assert_eq!(again.folders.len(), 2);
    }

    fn away_bare(folder: &str) -> Survey {
        Survey { unreadable: vec![folder.to_owned()], ..Survey::default() }
    }
}
