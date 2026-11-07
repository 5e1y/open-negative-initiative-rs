//! A project's members wherever they lie, as Swift's `standing`: the index's rows beyond the folder
//! opened, and a member no row holds laid at its path, so a project never shrinks unseen.

use crate::index::{Entry, LibraryIndex};
use crate::library::WAITING_MARK;
use crate::paths;
use crate::project::{self, Project};
use std::collections::HashSet;

/// The rows `project::grouped` needs beyond the opened folder's, and each Swift member found: its
/// print, then the path of the row carrying it, which the member becomes.
#[derive(Debug, Default, PartialEq)]
pub struct Standing {
    pub rows: Vec<Entry>,
    pub found: Vec<(String, String)>,
}

/// A trashed member stays hidden yet a member, and one the index lacks stands at its path, its
/// original looked at behind. A Swift print takes the first row shown carrying it no member holds.
pub fn standing(projects: &[Project], held: &[Entry], index: &LibraryIndex) -> Standing {
    let held_paths: HashSet<&str> = held.iter().map(|e| e.path.as_str()).collect();
    let members: HashSet<&str> = projects.iter().flat_map(|p| &p.members).map(String::as_str).collect();
    let mut out = Standing::default();
    let mut taken: HashSet<String> = HashSet::new();
    let mut add = |e: Entry, out: &mut Standing| {
        if !held_paths.contains(e.path.as_str()) && taken.insert(e.path.clone()) {
            out.rows.push(e);
        }
    };
    for member in projects.iter().flat_map(|p| &p.members) {
        if held_paths.contains(member.as_str()) {
            continue;
        }
        if project::is_print(member) {
            let claimed = |path: &str| members.contains(path) || out.found.iter().any(|(_, p)| p == path);
            if let Some(row) = index.with_fingerprint(member).into_iter().find(|r| shown(r, index) && !claimed(&r.path)) {
                out.found.push((member.clone(), row.path.clone()));
                add(row, &mut out);
            }
            continue;
        }
        match index.row(member) {
            Some(row) if shown(&row, index) => add(row, &mut out),
            Some(_) => {}
            None => add(Entry::new(member, crate::distant_past(), &format!("{WAITING_MARK}{member}")), &mut out),
        }
    }
    out
}

/// `LibraryIndex.shown`'s cut on one row: out of the ONI Trash, and no pass whose lead is held.
fn shown(row: &Entry, index: &LibraryIndex) -> bool {
    row.trashed_at.is_none()
        && row.pass_of.as_ref().is_none_or(|lead| index.row(&paths::join(&paths::parent(&row.path), lead)).is_none())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::Date;
    use std::path::Path;

    fn row(path: &str, fingerprint: &str) -> Entry {
        Entry::new(path, Date::since_reference_date(8e8), fingerprint)
    }

    #[test]
    fn a_project_resolves_beyond_the_opened_folder_and_never_shows_its_trashed_member() {
        let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
        let (a1, a2, b1, gone) = ("/A/1.RW2", "/A/2.RW2", "/B/1.RW2", "/B/gone.RW2");
        for (path, print) in [(a1, "fa1"), (a2, "fa2"), (b1, "fb1")] {
            index.upsert(&row(path, print)).unwrap();
        }
        index.set_trashed(Some(Date::since_reference_date(8.1e8)), a2).unwrap();
        let trip = Project::new("Trip", Date::since_reference_date(8e8), vec![a1.into(), a2.into(), b1.into(), gone.into()]);
        let projects = [trip];

        // B opened: its row is held, A's come from the index, the trashed one stays hidden.
        let held = vec![row(b1, "fb1")];
        let found = standing(&projects, &held, &index);
        let paths: Vec<&str> = found.rows.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, [a1, gone], "A's photo from the index, the unindexed member at its path");
        assert!(found.found.is_empty(), "no path is ever looked for by its print");
        let reach: Vec<Entry> = held.iter().cloned().chain(found.rows).collect();
        let listed: Vec<String> = project::grouped(&projects, &reach)[0].entries.iter().map(|e| e.path.clone()).collect();
        assert_eq!(listed, [a1, b1, gone], "every member but the trashed one, whatever folder is open");
        assert_eq!(projects[0].members.len(), 4, "the trashed photo stays a member");

        // Twin: resolved against the opened folder's rows alone, the project shrinks to B.
        let scoped: Vec<String> = project::grouped(&projects, &held)[0].entries.iter().map(|e| e.path.clone()).collect();
        assert_eq!(scoped, [b1]);

        // Restored, the photo joins its project again from the index.
        index.set_trashed(None, a2).unwrap();
        let back: Vec<String> = standing(&projects, &held, &index).rows.into_iter().map(|e| e.path).collect();
        assert_eq!(back, [a1, a2, gone]);
    }

    /// A Swift file's members are prints: each is found at the first row carrying it that no member
    /// holds, so two copies of one file never stand for one member twice.
    #[test]
    fn a_swift_print_is_found_at_a_row_no_member_holds() {
        let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
        let (r8, r10, other) = ("/Roll 8/a.NEF", "/Roll 10/a.NEF", "/Roll 8/b.NEF");
        for (path, print) in [(r10, "1-no-date-a"), (r8, "1-no-date-a"), (other, "1-no-date-b")] {
            index.upsert(&row(path, print)).unwrap();
        }
        let swift = Project::new("Swift", Date::since_reference_date(8e8), vec!["1-no-date-a".into(), "1-no-date-a2".into()]);
        let kept = Project::new("Kept", Date::since_reference_date(8e8), vec![r10.into()]);
        let projects = [swift, kept];
        let found = standing(&projects, &[], &index);
        assert_eq!(found.found, [("1-no-date-a".to_string(), r8.to_string())], "the copy no member holds; the unknown print stays");
        assert!(!found.rows.iter().any(|e| e.path.starts_with("1-no-date")), "a print is never laid as a path");
        let followed = project::moved(&projects, &found.found);
        assert_eq!(followed[0].members, [r8, "1-no-date-a2"]);
        // Twin: with no member spared, the first row carrying the print is Kept's own.
        let first = index.with_fingerprint("1-no-date-a").into_iter().map(|e| e.path).next();
        assert_ne!(first.as_deref(), Some(r8));
    }
}
