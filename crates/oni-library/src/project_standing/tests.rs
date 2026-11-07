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
    let found = standing(&projects, &held, &index, &Found::new());
    let paths: Vec<&str> = found.rows.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(paths, [a1, gone], "A's photo from the index, the unindexed member at its path");
    assert!(found.found.is_empty(), "a trashed member is never looked for on its copy");
    assert_eq!(shown(&projects, &held, &found), [[a1, b1, gone]], "every member but the trashed one, whatever folder is open");
    assert_eq!(projects[0].members.len(), 4, "the trashed photo stays a member");

    // Twin: resolved against the opened folder's rows alone, the project shrinks to B.
    assert_eq!(shown(&projects, &held, &Standing::default()), [[b1]]);

    // Restored, the photo joins its project again from the index.
    index.set_trashed(None, a2).unwrap();
    let back: Vec<String> = standing(&projects, &held, &index, &Found::new()).rows.into_iter().map(|e| e.path).collect();
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
    let found = standing(&projects, &[], &index, &Found::new());
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
    assert_eq!(shown(&swift, &[], &standing(&swift, &[], &index, &Found::new())), [[backup]], "the copy shows meanwhile");
    index.set_trashed(None, original).unwrap();
    index.set_trashed(day, backup).unwrap();
    assert_eq!(shown(&swift, &[], &standing(&swift, &[], &index, &Found::new())), [[original]], "the original back, the copy gone");
    // Twin: the first read's choice written as the member holds it on the copy for good.
    let committed = [Project::new("Committed", Date::since_reference_date(8e8), vec![of(backup, "1-a")])];
    assert!(shown(&committed, &[], &standing(&committed, &[], &index, &Found::new()))[0].is_empty());
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
    let found = standing(&trip, &[], &index, &Found::new());
    let same_file = if paths::ignores_case("/") { "/Roll 10/b.NEF" } else { "/Copies/b.NEF" };
    assert_eq!(shown(&trip, &[], &found), [["/Copies/a.NEF", same_file]]);
    // Twin: a member by its path alone takes the other file standing there.
    let blind = [Project::new("Blind", Date::since_reference_date(8e8), vec![Member::new(Some("/Roll 10/a.NEF"), None)])];
    assert_eq!(shown(&blind, &[], &standing(&blind, &[], &index, &Found::new())), [["/Roll 10/a.NEF"]]);
}

fn dated(path: &str, print: &str, seconds: f64) -> Entry {
    Entry { captured_at: Some(Date::since_reference_date(seconds)), ..Entry::new(path, Date::since_reference_date(seconds), print) }
}

/// Frame 12 rescanned over its file, and a Swift print whose only copy another project holds: each
/// stands missing in its project, under the file that took its place, and Remove from Project takes it.
#[test]
fn a_member_no_file_shows_stands_missing_where_it_can_be_removed() {
    let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
    let (twelve, kept) = ("/Roll 8/12.tif", "/Roll 8/13.tif");
    for (path, print) in [(twelve, "1-rescan"), (kept, "1-13")] {
        index.upsert(&row(path, print)).unwrap();
    }
    let trip = Project::new("Trip", Date::since_reference_date(8e8), vec![of(twelve, "1-12"), Member::new(None, Some("1-13"))]);
    let other = Project::new("Other", Date::since_reference_date(8e8), vec![of(kept, "1-13")]);
    let mut projects = vec![trip, other];
    let found = standing(&projects, &[], &index, &Found::new());
    let lost = ["/Roll 8/12.tif/12.tif", "/Roll 8/13.tif/13.tif"];
    assert_eq!(shown(&projects, &[], &found), [lost.to_vec(), vec![kept]], "each under the file now holding its place");
    assert!(found.rows.iter().filter(|e| lost.contains(&e.path.as_str())).all(|e| Member::of(e, None).print.is_none()), "no print of a file it is not");
    // Twin: by the rows alone, as before, Trip shows nothing at all.
    assert_eq!(shown(&projects, &[], &Standing { rows: found.rows.clone(), found: Found::new() })[0], Vec::<String>::new());
    let reach: Vec<Entry> = found.rows.clone();
    let id = projects[0].id;
    assert_eq!(project::remove_from(&mut projects, &Reach::new(&reach, &found.found), &HashSet::from([lost[0].to_string()]), id), [lost[0]]);
    assert_eq!(projects[0].members, [Member::new(None, Some("1-13"))], "the rescanned frame's member gone, nothing else");
    // A stand-in moved to another project carries its member whole, never the stand-in's place.
    let moved = Member::of(&found.rows.iter().find(|e| e.path == lost[1]).unwrap().clone(), Some(&projects[0].members[0]));
    assert_eq!(moved, Member::new(None, Some("1-13")));
}

/// A member lost from its place stands on the copy nearest it: its own file name, then its own
/// folder, before the earliest; the copy one read chose stays its own while it stays free.
#[test]
fn a_lost_member_stands_on_its_nearest_copy_and_keeps_it() {
    let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
    let (earliest, folder, named) = ("/A Backup/zz.tif", "/Roll 8/renamed.tif", "/Elsewhere/12.tif");
    for (path, at) in [(earliest, 7e8), (folder, 8e8), (named, 9e8)] {
        index.upsert(&dated(path, "1-12", at)).unwrap();
    }
    let member = of("/Roll 8/12.tif", "1-12");
    let trip = [Project::new("Trip", Date::since_reference_date(8e8), vec![member.clone()])];
    let first = standing(&trip, &[], &index, &Found::new());
    assert_eq!(first.found[&member], named, "its own file name first");
    index.set_trashed(Some(Date::since_reference_date(9.5e8)), named).unwrap();
    let second = standing(&trip, &[], &index, &first.found);
    assert_eq!(second.found[&member], folder, "then its own folder");
    index.set_trashed(None, named).unwrap();
    assert_eq!(standing(&trip, &[], &index, &second.found).found[&member], folder, "the copy chosen stays while free");
    assert_eq!(standing(&trip, &[], &index, &Found::new()).found[&member], named, "a read with nothing chosen ranks again");
    // Twin: by date then path, as before, the backup copy wins.
    assert_eq!(index.with_fingerprint("1-12")[0].path, earliest);
}

/// A photo added while its tile was being read joined without a print: once its file is read it
/// takes the print, so a later read finds it by that print once it moves; a member's own print stays.
#[test]
fn a_member_without_a_print_learns_it_from_its_file() {
    let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
    let (a, b) = ("/Roll/a.tif", "/Roll/b.tif");
    for (path, print) in [(a, "1-a"), (b, "1-other")] {
        index.upsert(&row(path, print)).unwrap();
    }
    let mut trip = vec![Project::new("Trip", Date::since_reference_date(8e8), vec![Member::new(Some(a), None), of(b, "1-b")])];
    let unlearned = trip.clone();
    let held = [row(a, "waiting-/Roll/a.tif")];
    let stood = standing(&trip, &held, &index, &Found::new());
    assert!(!learn(&mut trip, &held, &index, &stood), "a tile still being read teaches nothing");
    let stood = standing(&trip, &[], &index, &Found::new());
    assert!(learn(&mut trip, &[], &index, &stood));
    assert_eq!(trip[0].members, [of(a, "1-a"), of(b, "1-b")], "b keeps its print over the other file at its place");
    let stood = standing(&trip, &[], &index, &Found::new());
    assert!(!learn(&mut trip, &[], &index, &stood), "learnt once");
    // Moved outside ONI, the photo is found by its print.
    let moved = "/Roll moved/a.tif";
    index.upsert(&row(moved, "1-a")).unwrap();
    index.remove(a).unwrap();
    assert_eq!(shown(&trip, &[], &standing(&trip, &[], &index, &Found::new()))[0][0], moved);
    // Twin: without its print, the moved photo stands missing at its old place.
    assert_eq!(shown(&unlearned, &[], &standing(&unlearned, &[], &index, &Found::new()))[0][0], a);
}
