//! The ONI Trash: a deleted photo is marked in its sidecar and hidden, its file never touched,
//! moved or removed. The index caches the mark.

use crate::film_group::{self, FilmGroup, FilmKey};
use crate::folders;
use crate::index::Entry;
use crate::paths::{self, holds};
use crate::refusal;
use oni_i18n::{tr, trn};
use oni_settings::{Date, sidecar};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outcome {
    pub done: Vec<String>,
    /// Sidecars written by a newer version, which this one never rewrites.
    pub refused: Vec<String>,
    /// Originals not on disk, their disk away or the file moved: no sidecar is written beside nothing.
    pub unreachable: Vec<String>,
    /// Each photo whose sidecar the disk refused, with the cause.
    pub failed: Vec<(String, String)>,
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
            Ok(false) | Err(sidecar::Error::Newer) => out.refused.push(path.clone()),
            Err(sidecar::Error::Unreachable) => out.unreachable.push(path.clone()),
            Err(e) => out.failed.push((path.clone(), refusal::said(&e))),
        }
    }
    out
}

/// Names at most this many failed files, then counts the rest.
const NAMED: usize = 3;

/// Every cause of a mark that did not land, each with its count; `None` when every photo was marked.
pub fn report(outcome: &Outcome, restoring: bool) -> Option<String> {
    let head = |count: usize| {
        if restoring {
            trn("One photo was not restored", "{n} photos were not restored", count as u64)
        } else {
            trn("One photo was not moved to the ONI Trash", "{n} photos were not moved to the ONI Trash", count as u64)
        }
    };
    let mut said = Vec::new();
    if !outcome.refused.is_empty() {
        let head = head(outcome.refused.len());
        said.push(tr!("{head}: a newer version of ONI wrote their settings, and this one never rewrites them.", head = head));
    }
    if !outcome.unreachable.is_empty() {
        let count = outcome.unreachable.len();
        // "Nothing was changed" holds only when nothing else was.
        let why = if outcome.done.is_empty() { refusal::why_gone(count) } else { refusal::why_unreachable(count) };
        said.push(tr!("{head}: {why}", head = head(count), why = why));
    }
    if !outcome.failed.is_empty() {
        let mut files: Vec<String> = outcome
            .failed
            .iter()
            .take(NAMED)
            .map(|(path, cause)| {
                let name = paths::name(&sidecar::url_for(Path::new(path)).to_string_lossy());
                tr!("{file} could not be written ({cause})", file = name, cause = cause)
            })
            .collect();
        if outcome.failed.len() > NAMED {
            let rest = (outcome.failed.len() - NAMED) as u64;
            files.push(trn("and {n} other", "and {n} others", rest));
        }
        let head = head(outcome.failed.len());
        said.push(tr!("{head}: {files}.", head = head, files = files.join(", ")));
    }
    if !said.is_empty() && !outcome.done.is_empty() {
        let done = outcome.done.len() as u64;
        said.push(if restoring {
            trn("The other photo is back in its folder.", "The other {n} photos are back in their folders.", done)
        } else {
            trn("The other photo went to the ONI Trash.", "The other {n} photos went to the ONI Trash.", done)
        });
    }
    (!said.is_empty()).then(|| said.join(" "))
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

/// A folder by its path from the home or its volume; a Windows drive by its letter, `D: › Scans/One`.
pub fn heading(folder: &str, home: &str) -> String {
    if holds(home, folder) {
        let tail = if folder == home { String::new() } else { format!("/{}", paths::relative(folder, home)) };
        return format!("{}{tail}", paths::name(home));
    }
    if let Some(on_volume) = folder.strip_prefix("/Volumes/") {
        return on_volume.to_owned();
    }
    if folder.get(..3).is_some_and(crate::navigator::is_drive_root) {
        let rest = folder[3..].trim_matches('/');
        return if rest.is_empty() { folder[..2].to_owned() } else { format!("{} › {rest}", &folder[..2]) };
    }
    folder.trim_start_matches('/').to_owned()
}

#[cfg(test)]
mod round_trip;

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
        assert_eq!(heading("D:/Scans/One", "C:/Users/a"), "D: › Scans/One");
        assert_eq!(heading("D:/", "C:/Users/a"), "D:");
        // Twin: a folder whose name only starts like a drive keeps its path.
        assert_eq!(heading("/D:x/One", "/Users/a"), "D:x/One");
    }

    #[test]
    fn a_partial_restore_says_what_came_back() {
        let unreachable = vec!["/gone/a".to_owned()];
        let partial = Outcome { done: vec!["/r/b".into(), "/r/c".into()], unreachable: unreachable.clone(), ..Outcome::default() };
        let said = report(&partial, true).unwrap_or_default();
        assert_eq!(said, "One photo was not restored: its original cannot be found, its disk disconnected or the file moved. The other 2 photos are back in their folders.");
        // Twin: when nothing was restored, "Nothing was changed" is true and stays.
        let none = report(&Outcome { unreachable, ..Outcome::default() }, true).unwrap_or_default();
        assert!(none.ends_with("Nothing was changed.") && !none.contains("The other"), "{none}");
    }

    #[test]
    fn a_refused_mark_is_said_and_a_clean_one_is_silent() {
        let refused = Outcome { refused: vec!["/a".into()], ..Outcome::default() };
        assert!(report(&refused, false).unwrap().starts_with("One photo was not moved to the ONI Trash"));
        // Twin: a gesture that marked everything says nothing.
        assert_eq!(report(&Outcome { done: vec!["/a".into()], ..Outcome::default() }, false), None);
    }
}
