//! The ONI Trash: a deleted photo is marked in its sidecar and hidden, its file never touched,
//! moved or removed. The index caches the mark.

use crate::film_group::{self, FilmGroup, FilmKey};
use crate::folders;
use crate::index::Entry;
use crate::paths::{self, holds};
use oni_settings::{Date, sidecar};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outcome {
    pub done: Vec<String>,
    /// Sidecars written by a newer version, which this one never rewrites.
    pub refused: Vec<String>,
    pub failure: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Folder {
    pub path: String,
    pub count: usize,
    pub latest: Date,
}

/// Writes the mark, or clears it on `None`, in each file's sidecar and nowhere else. `mosaic` says
/// whether a file decodes through the RAW engine's mosaic, which an old sidecar's migration reads.
pub fn mark(files: &[String], date: Option<Date>, mosaic: &dyn Fn(&Path) -> bool) -> Outcome {
    let mut out = Outcome::default();
    for path in files {
        let original = Path::new(path);
        match sidecar::set_trashed(date, original, mosaic(original)) {
            Ok(true) => out.done.push(path.clone()),
            Ok(false) => out.refused.push(path.clone()),
            Err(e) => out.failure = Some(e.to_string()),
        }
    }
    out
}

pub fn report(outcome: &Outcome, restoring: bool) -> Option<String> {
    let verb = if restoring { "restored" } else { "moved to the ONI Trash" };
    if !outcome.refused.is_empty() {
        let count = outcome.refused.len();
        let head =
            if count == 1 { format!("One photo was not {verb}") } else { format!("{count} photos were not {verb}") };
        return Some(format!("{head}: a newer version of ONI wrote their settings, and this one never rewrites them."));
    }
    outcome.failure.as_ref().map(|f| format!("Some photos were not {verb}: {f}"))
}

/// The folders holding trashed photos, latest deletion first, each counting its own photos.
pub fn folders(trashed: &[Entry]) -> Vec<Folder> {
    let mut by_folder: HashMap<String, Vec<&Entry>> = HashMap::new();
    for entry in trashed {
        by_folder.entry(folders::parent(&entry.path)).or_default().push(entry);
    }
    let mut out: Vec<Folder> = by_folder
        .into_iter()
        .map(|(path, frames)| Folder {
            path,
            count: frames.len(),
            latest: frames
                .iter()
                .filter_map(|e| e.trashed_at)
                .max_by(|a, b| crate::seconds(*a).total_cmp(&crate::seconds(*b)))
                .unwrap_or(crate::distant_past()),
        })
        .collect();
    out.sort_by(|a, b| {
        if a.latest != b.latest {
            crate::seconds(b.latest).total_cmp(&crate::seconds(a.latest))
        } else {
            paths::standard_order(&paths::name(&a.path), &paths::name(&b.path))
        }
    });
    out
}

/// What a folder row's Restore brings back: its own frames, never a subfolder's.
pub fn frames(folder: &str, trashed: &[Entry]) -> HashSet<String> {
    trashed.iter().map(|e| e.path.clone()).filter(|p| folders::parent(p) == folder).collect()
}

pub fn groups(trashed: &[Entry], home: &str) -> Vec<FilmGroup> {
    let mut by_folder: HashMap<String, Vec<Entry>> = HashMap::new();
    for entry in trashed {
        by_folder.entry(folders::parent(&entry.path)).or_default().push(entry.clone());
    }
    folders(trashed)
        .into_iter()
        .map(|folder| {
            let mut frames = by_folder.remove(&folder.path).unwrap_or_default();
            let start = frames
                .iter()
                .map(film_group::capture)
                .min_by(|a, b| crate::seconds(*a).total_cmp(&crate::seconds(*b)))
                .unwrap_or(crate::distant_past());
            frames.sort_by(folders::shot);
            FilmGroup {
                id: FilmKey::Folder(folder.path.clone()),
                film: None,
                start,
                entries: frames,
                heading: Some(heading(&folder.path, home)),
            }
        })
        .collect()
}

/// A folder by its path from the home or its volume.
pub fn heading(folder: &str, home: &str) -> String {
    if holds(home, folder) {
        let tail = if folder == home { String::new() } else { format!("/{}", paths::relative(folder, home)) };
        return format!("{}{tail}", paths::name(home));
    }
    if let Some(on_volume) = folder.strip_prefix("/Volumes/") {
        return on_volume.to_owned();
    }
    folder.trim_start_matches('/').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(path: &str, trashed: Date) -> Entry {
        let mut e = Entry::new(path, Date::since_reference_date(8e8), path);
        e.trashed_at = Some(trashed);
        e
    }

    #[test]
    fn the_trash_lists_its_folders_latest_deletion_first() {
        let older = Date::since_reference_date(812_000_000.0);
        let when = Date::since_reference_date(812_345_678.0);
        let rows = [
            row("/Users/a/Scans/One/1.RW2", older),
            row("/Volumes/T7/Two/2.RW2", when),
            row("/Users/a/Scans/One/3.RW2", older),
        ];
        let listed = folders(&rows);
        assert_eq!(
            listed.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(),
            ["/Volumes/T7/Two", "/Users/a/Scans/One"]
        );
        assert_eq!(listed.iter().map(|f| f.count).collect::<Vec<_>>(), [1, 2]);
        let mut nested = rows.to_vec();
        nested.push(row("/Users/a/Scans/One/Sub/4.RW2", when));
        assert_eq!(
            frames("/Users/a/Scans/One", &nested),
            HashSet::from(["/Users/a/Scans/One/1.RW2".to_owned(), "/Users/a/Scans/One/3.RW2".to_owned()])
        );
        let headings: Vec<String> = groups(&rows, "/Users/a").iter().map(FilmGroup::display_title).collect();
        assert_eq!(headings, ["T7/Two", "a/Scans/One"]);
        // Twin: read from another home, the same folder heads from the root.
        assert_eq!(heading("/Users/a/Scans/One", "/Users/b"), "Users/a/Scans/One");
    }

    #[test]
    fn a_refused_mark_is_said_and_a_clean_one_is_silent() {
        let refused = Outcome { refused: vec!["/a".into()], ..Outcome::default() };
        assert!(report(&refused, false).unwrap().starts_with("One photo was not moved to the ONI Trash"));
        // Twin: a gesture that marked everything says nothing.
        assert_eq!(report(&Outcome { done: vec!["/a".into()], ..Outcome::default() }, false), None);
    }
}
