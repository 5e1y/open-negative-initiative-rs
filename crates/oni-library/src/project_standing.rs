//! A project's members wherever they lie, as Swift's `standing`, found again by their prints where
//! their paths went stale, so a project never shrinks unseen; each read looks again, writing nothing.

use crate::index::{Entry, LibraryIndex};
use crate::library::WAITING_MARK;
use crate::paths;
use crate::project::{Found, Member, Project};
use std::collections::{HashMap, HashSet};

/// The rows `project::grouped` needs beyond the opened folder's, and where each member its path no
/// longer shows stands this read.
#[derive(Debug, Default, PartialEq)]
pub struct Standing {
    pub rows: Vec<Entry>,
    pub found: Found,
}

/// A member stands at its path while the row there carries its print, a trashed one hidden; one gone,
/// or whose path holds another file, takes the first free row with its print, else stands missing.
pub fn standing(projects: &[Project], held: &[Entry], index: &LibraryIndex) -> Standing {
    let held_at: HashMap<&str, &Entry> = held.iter().map(|e| (e.path.as_str(), e)).collect();
    let mut out = Standing::default();
    let mut added: HashSet<String> = HashSet::new();
    let mut add = |e: Entry, out: &mut Standing| {
        if !held_at.contains_key(e.path.as_str()) && added.insert(e.path.clone()) {
            out.rows.push(e);
        }
    };
    let mut claimed: HashSet<String> = HashSet::new();
    let mut looked: Vec<(&Member, bool)> = Vec::new();
    for member in projects.iter().flat_map(|p| &p.members) {
        let Some(path) = member.path.as_deref() else {
            looked.push((member, false));
            continue;
        };
        match held_at.get(path).map(|e| (*e).clone()).or_else(|| index.row(path)) {
            Some(row) if member.agrees(&row.fingerprint) => {
                claimed.insert(row.path.clone());
                if row.path != path {
                    out.found.insert(member.clone(), row.path.clone());
                }
                if shown(&row, index) {
                    add(row, &mut out);
                }
            }
            Some(_) => looked.push((member, false)),
            None => looked.push((member, true)),
        }
    }
    for (member, gone) in looked {
        let free: Vec<Entry> = member
            .print
            .as_deref()
            .map(|print| index.with_fingerprint(print))
            .unwrap_or_default()
            .into_iter()
            .filter(|r| shown(r, index) && !claimed.contains(&r.path))
            .collect();
        let same = free.iter().position(|r| member.path.as_deref().is_some_and(|p| paths::same(p, &r.path)));
        match same.or((!free.is_empty()).then_some(0)) {
            Some(at) => {
                let row = free[at].clone();
                claimed.insert(row.path.clone());
                out.found.insert(member.clone(), row.path.clone());
                add(row, &mut out);
            }
            None => {
                if let Some(path) = member.path.as_deref().filter(|_| gone) {
                    add(Entry::new(path, crate::distant_past(), &format!("{WAITING_MARK}{path}")), &mut out);
                }
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
    use crate::project::{self, Reach};
    use oni_settings::Date;
    use std::path::Path;

    fn row(path: &str, fingerprint: &str) -> Entry {
        Entry { captured_at: Some(Date::since_reference_date(8e8)), ..Entry::new(path, Date::since_reference_date(8e8), fingerprint) }
    }

    fn of(path: &str, print: &str) -> Member {
        Member::new(Some(path), Some(print))
    }

    fn shown(projects: &[Project], held: &[Entry], standing: &Standing) -> Vec<Vec<String>> {
        let reach: Vec<Entry> = held.iter().cloned().chain(standing.rows.iter().cloned()).collect();
        project::grouped(projects, &Reach::new(&reach, &standing.found)).into_iter().map(|g| g.entries.into_iter().map(|e| e.path).collect()).collect()
    }

    #[test]
    fn a_project_resolves_beyond_the_opened_folder_and_never_shows_its_trashed_member() {
        let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
        let (a1, a2, b1, gone) = ("/A/1.RW2", "/A/2.RW2", "/B/1.RW2", "/B/gone.RW2");
        for (path, print) in [(a1, "fa1"), (a2, "fa2"), (b1, "fb1"), ("/C/copy-of-2.RW2", "fa2")] {
            index.upsert(&row(path, print)).unwrap();
        }
        index.set_trashed(Some(Date::since_reference_date(8.1e8)), a2).unwrap();
        let members = vec![of(a1, "fa1"), of(a2, "fa2"), of(b1, "fb1"), Member::new(Some(gone), None)];
        let projects = [Project::new("Trip", Date::since_reference_date(8e8), members)];

        // B opened: its row is held, A's come from the index, the trashed one stays hidden.
        let held = vec![row(b1, "fb1")];
        let found = standing(&projects, &held, &index);
        let paths: Vec<&str> = found.rows.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, [a1, gone], "A's photo from the index, the unindexed member at its path");
        assert!(found.found.is_empty(), "a trashed member is never looked for on its copy");
        assert_eq!(shown(&projects, &held, &found), [[a1, b1, gone]], "every member but the trashed one, whatever folder is open");
        assert_eq!(projects[0].members.len(), 4, "the trashed photo stays a member");

        // Twin: resolved against the opened folder's rows alone, the project shrinks to B.
        assert_eq!(shown(&projects, &held, &Standing::default()), [[b1]]);

        // Restored, the photo joins its project again from the index.
        index.set_trashed(None, a2).unwrap();
        let back: Vec<String> = standing(&projects, &held, &index).rows.into_iter().map(|e| e.path).collect();
        assert_eq!(back, [a1, a2, gone]);
    }

    /// A Swift file's members are prints: each is found at the first row carrying it that no member
    /// stands at, so two copies of one file never stand for one member twice; nothing is written.
    #[test]
    fn a_swift_print_is_found_at_a_row_no_member_holds() {
        let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
        let (r8, r10, other) = ("/Roll 8/a.NEF", "/Roll 10/a.NEF", "/Roll 8/b.NEF");
        for (path, print) in [(r10, "1-no-date-a"), (r8, "1-no-date-a"), (other, "1-no-date-b")] {
            index.upsert(&row(path, print)).unwrap();
        }
        let swift = Project::new("Swift", Date::since_reference_date(8e8), vec![Member::new(None, Some("1-no-date-a")), Member::new(None, Some("1-no-date-a2"))]);
        let kept = Project::new("Kept", Date::since_reference_date(8e8), vec![of(r10, "1-no-date-a")]);
        let projects = [swift, kept];
        let found = standing(&projects, &[], &index);
        assert_eq!(found.found, Found::from([(Member::new(None, Some("1-no-date-a")), r8.to_string())]), "the copy no member stands at");
        assert_eq!(shown(&projects, &[], &found), [vec![r8], vec![r10]]);
        assert!(projects[0].members.iter().all(|m| m.path.is_none()), "the choice is the read's, never the file's");
        // Twin: with no member spared, the first row carrying the print is Kept's own.
        let first = index.with_fingerprint("1-no-date-a").into_iter().map(|e| e.path).next();
        assert_ne!(first.as_deref(), Some(r8));
    }

    /// An old Swift project's print whose original sits in the ONI Trash while a backup copy is
    /// indexed: the copy shows, nothing is committed, and the original shows once the copy goes.
    #[test]
    fn a_print_whose_original_is_trashed_shows_its_copy_for_this_read_only() {
        let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
        let (original, backup) = ("/Roll 8/a.NEF", "/Backup/a.NEF");
        for path in [original, backup] {
            index.upsert(&row(path, "1-a")).unwrap();
        }
        let day = Some(Date::since_reference_date(8.1e8));
        index.set_trashed(day, original).unwrap();
        let swift = [Project::new("Swift", Date::since_reference_date(8e8), vec![Member::new(None, Some("1-a"))])];
        assert_eq!(shown(&swift, &[], &standing(&swift, &[], &index)), [[backup]], "the copy shows meanwhile");
        index.set_trashed(None, original).unwrap();
        index.set_trashed(day, backup).unwrap();
        assert_eq!(shown(&swift, &[], &standing(&swift, &[], &index)), [[original]], "the original back, the copy gone");
        // Twin: the first read's choice written as the member holds it on the copy for good.
        let committed = [Project::new("Committed", Date::since_reference_date(8e8), vec![of(backup, "1-a")])];
        assert!(shown(&committed, &[], &standing(&committed, &[], &index))[0].is_empty());
    }

    /// The print tells another file at a member's path, as a Locate onto copies leaves one: that
    /// file never stands for it, the member's copy elsewhere does, and its own path is tried first.
    #[test]
    fn a_member_whose_path_holds_another_file_is_found_by_its_print() {
        let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
        for (path, print) in [("/Roll 10/a.NEF", "1-other"), ("/Roll 10/b.NEF", "1-b"), ("/Copies/b.NEF", "1-b"), ("/Copies/a.NEF", "1-a")] {
            index.upsert(&row(path, print)).unwrap();
        }
        let trip = [Project::new("Trip", Date::since_reference_date(8e8), vec![of("/Roll 10/a.NEF", "1-a"), of("/roll 10/B.NEF", "1-b")])];
        let found = standing(&trip, &[], &index);
        let same_file = if paths::ignores_case("/") { "/Roll 10/b.NEF" } else { "/Copies/b.NEF" };
        assert_eq!(shown(&trip, &[], &found), [["/Copies/a.NEF", same_file]]);
        // Twin: a member by its path alone takes the other file standing there.
        let blind = [Project::new("Blind", Date::since_reference_date(8e8), vec![Member::new(Some("/Roll 10/a.NEF"), None)])];
        assert_eq!(shown(&blind, &[], &standing(&blind, &[], &index)), [["/Roll 10/a.NEF"]]);
    }
}
