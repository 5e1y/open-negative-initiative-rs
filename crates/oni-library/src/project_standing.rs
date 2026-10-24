//! A project's members wherever they lie, as Swift's `standing`: the index's rows beyond the folder
//! opened, and a member no row holds laid at its recorded path, so a project never shrinks unseen.

use crate::index::{Entry, LibraryIndex};
use crate::paths;
use crate::project::Project;
use std::collections::HashSet;

/// The rows `project::grouped` needs beyond `held`: a trashed member stays hidden yet a member, and
/// one the index lacks stands at its recorded path, its original looked at behind.
pub fn standing(projects: &[Project], held: &[Entry], index: &LibraryIndex) -> Vec<Entry> {
    let held_paths: HashSet<&str> = held.iter().map(|e| e.path.as_str()).collect();
    let held_prints: HashSet<&str> = held.iter().map(|e| e.fingerprint.as_str()).collect();
    let mut out: Vec<Entry> = Vec::new();
    let mut taken: HashSet<String> = HashSet::new();
    let mut add = |e: Entry, out: &mut Vec<Entry>| {
        if !held_paths.contains(e.path.as_str()) && taken.insert(e.path.clone()) {
            out.push(e);
        }
    };
    for project in projects {
        for member in &project.members {
            let recorded = project.paths.as_ref().and_then(|p| p.get(member));
            if recorded.is_some_and(|p| held_paths.contains(p.as_str())) {
                continue;
            }
            let at = recorded.and_then(|p| index.row(p));
            if let Some(row) = at.clone().filter(|r| shown(r, index)) {
                add(row, &mut out);
                continue;
            }
            if held_prints.contains(member.as_str()) {
                continue;
            }
            let copies = index.with_fingerprint(member);
            if let Some(row) = copies.iter().find(|r| shown(r, index)) {
                add(row.clone(), &mut out);
            } else if let (None, true, Some(path)) = (&at, copies.is_empty(), recorded) {
                add(Entry::new(path, crate::distant_past(), member), &mut out);
            }
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
    use crate::project;
    use oni_settings::Date;
    use std::collections::BTreeMap;
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
        let mut trip = project::Project::new("Trip", Date::since_reference_date(8e8), vec!["fa1".into(), "fa2".into(), "fb1".into(), "fgone".into()]);
        trip.paths = Some(BTreeMap::from([("fgone".to_string(), gone.to_string())]));
        let projects = [trip];

        // B opened: its row is held, A's come from the index, the trashed one stays hidden.
        let held = vec![row(b1, "fb1")];
        let found = standing(&projects, &held, &index);
        let paths: Vec<&str> = found.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, [a1, gone], "A's photo from the index, the unindexed member at its recorded path");
        let reach: Vec<Entry> = held.iter().cloned().chain(found).collect();
        let listed: Vec<String> = project::grouped(&projects, &reach)[0].entries.iter().map(|e| e.path.clone()).collect();
        assert_eq!(listed, [a1, b1, gone], "every member but the trashed one, whatever folder is open");
        assert_eq!(projects[0].members.len(), 4, "the trashed photo stays a member");

        // Twin: resolved against the opened folder's rows alone, the project shrinks to B.
        let scoped: Vec<String> = project::grouped(&projects, &held)[0].entries.iter().map(|e| e.path.clone()).collect();
        assert_eq!(scoped, [b1]);

        // Restored, the photo joins its project again from the index.
        index.set_trashed(None, a2).unwrap();
        let back: Vec<String> = standing(&projects, &held, &index).into_iter().map(|e| e.path).collect();
        assert_eq!(back, [a1, a2, gone]);
    }
}
