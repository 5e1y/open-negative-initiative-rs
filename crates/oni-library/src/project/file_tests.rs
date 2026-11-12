use super::shipped_tests::{old, old_path};
use super::*;
use crate::index::{Entry, LibraryIndex};
use crate::migrate::projects::is_path;
use crate::project_standing;
use crate::scratch::Scratch;
use std::path::Path;

fn stamp() -> Date {
    Date::since_reference_date(8e8)
}

fn former(support: &Path, id: Id, members: &[&str], paths: &[(&str, &str)]) {
    let paths: serde_json::Map<String, serde_json::Value> = paths.iter().map(|(k, v)| (k.to_string(), (*v).into())).collect();
    let file = serde_json::json!({"version": 1, "projects": [{"id": id.to_string(), "name": "Trip", "description": "", "createdAt": 8e8, "members": members, "paths": paths}]});
    std::fs::write(old_path(support), serde_json::to_vec(&file).unwrap()).unwrap();
}

#[test]
fn a_file_round_trips_through_the_disk_saying_its_format() {
    let scratch = Scratch::new("projects");
    let mut iceland = Project::new("Iceland", stamp(), vec![Member::new(Some("/r/a.NEF"), Some("fa")), Member::new(None, Some("fb"))]);
    iceland.description = "Two weeks, June".into();
    let mut portraits = Project::new("Portraits", stamp(), vec![]);
    portraits.manual_order = Some(3.0);
    let file = ProjectsFile { version: FORMAT, projects: vec![iceland, portraits] };
    save(scratch.path(), &file).unwrap();
    assert_eq!(load(scratch.path()), file);
    let text = std::fs::read_to_string(scratch.path().join("State/projects.json")).unwrap();
    assert!(text.contains(&format!("\"version\": {FORMAT}")) && text.contains("\"print\": \"fa\""), "{text}");
    assert!(text.contains(&format!("\"createdAt\": \"{}\"", stamp().iso8601())) && text.contains("\"manualOrder\": 3.0"), "{text}");
    // Twin: the seconds since 2001 the old file kept are not what this one writes.
    assert!(!text.contains("800000000"));
}

/// A version 1 file read: each print beside its recorded path spelt as the index spells it, a path
/// kept alone, a Swift print; a backslashed path is a path, never a print.
#[test]
fn a_former_file_reads_each_member_with_its_path_and_its_print() {
    let scratch = Scratch::new("projects-former");
    let id = Id::new();
    let decomposed = "/r8/E\u{301}te\u{301}.NEF";
    former(
        scratch.path(),
        id,
        &["1-no-date-a", "1-no-date-b", "1-no-date-c", "/r9/z.NEF", "2026:10:31/x-y", "1-no-date-a"],
        &[("1-no-date-a", decomposed), ("1-no-date-c", r"C:\Scans\c.NEF")],
    );
    let read = old(scratch.path());
    let composed = if cfg!(target_os = "macos") { "/r8/\u{c9}t\u{e9}.NEF" } else { decomposed };
    let backslashed = if cfg!(windows) { "C:/Scans/c.NEF" } else { r"C:\Scans\c.NEF" };
    assert_eq!(
        read[0].members,
        [
            Member::new(Some(composed), Some("1-no-date-a")),
            Member::new(None, Some("1-no-date-b")),
            Member::new(Some(backslashed), Some("1-no-date-c")),
            Member::new(Some("/r9/z.NEF"), None),
            Member::new(None, Some("2026:10:31/x-y")),
        ],
        "each photo once, a print holding a slash still a print"
    );
    // Twin: a slash taken for a path, the date-stamped print would be lost as a path.
    assert!(!is_path("2026:10:31/x-y") && is_path(r"C:\Scans\c.NEF") && is_path("/r9/z.NEF"));
}

/// Builds before #1550 took 1.5.6's placeholder for a print and wrote it as an exact member, with its
/// path or without: each reads as that placeholder at its path, and the next write keeps both.
#[test]
fn an_exact_member_holding_a_placeholder_stands_at_its_path() {
    let scratch = Scratch::new("projects-exact-placeholder");
    let support = scratch.path();
    let (a, b) = ("/r/a.NEF", "/r/b.NEF");
    let (wa, wb) = (format!("{}{a}", crate::library::WAITING_MARK), format!("{}{b}", crate::library::WAITING_MARK));
    let paths: serde_json::Map<String, serde_json::Value> = [(wb.clone(), b.into())].into_iter().collect();
    let exact = serde_json::json!([{"print": wa}, {"path": b, "print": wb}]);
    let project = serde_json::json!({"id": Id::new().to_string(), "name": "Trip", "description": "", "createdAt": 8e8, "members": [wa, wb], "paths": paths, "exactMembers": exact});
    std::fs::write(old_path(support), serde_json::to_vec(&serde_json::json!({"version": 3, "projects": [project]})).unwrap()).unwrap();
    let read = old(support);
    let standing = [Member::new(Some(a), Some(&wa)), Member::new(Some(b), Some(&wb))];
    assert_eq!(read[0].members, standing, "each its placeholder, at its path");
    let mut renamed = read.clone();
    renamed[0].name = "Porto".into();
    save(support, &ProjectsFile { version: FORMAT, projects: renamed }).unwrap();
    assert_eq!(load(support).projects[0].members, standing, "the next write keeps both");
    // Twin: a member left with neither path nor print, as a's was, is dropped by that write.
    save(support, &ProjectsFile { version: FORMAT, projects: vec![Project::new("Lost", stamp(), vec![Member::new(None, None)])] }).unwrap();
    assert!(load(support).projects[0].members.is_empty());
}

/// A folder renamed in 1.5.6 moves the paths recorded beside its placeholders, never their texts: a
/// version 1 text and an exact member each read at the path recorded, not at the one after the mark.
#[test]
fn a_placeholder_recorded_elsewhere_than_its_mark_stands_where_recorded() {
    let scratch = Scratch::new("projects-renamed-placeholder");
    let support = scratch.path();
    let mark = |file: &str| format!("{}/Roll 8/{file}", crate::library::WAITING_MARK);
    let (wa, wb) = (mark("a.NEF"), mark("b.NEF"));
    let (a, b) = ("/Roll 8b/a.NEF", "/Roll 8b/b.NEF");
    let project = |name: &str, text: &str, path: &str, exact: Option<serde_json::Value>| {
        let paths: serde_json::Map<String, serde_json::Value> = [(text.to_owned(), path.into())].into_iter().collect();
        let mut p = serde_json::json!({"id": Id::new().to_string(), "name": name, "description": "", "createdAt": 8e8, "members": [text], "paths": paths});
        if let Some(exact) = exact {
            p["exactMembers"] = exact;
        }
        p
    };
    let former = project("Trip", &wa, a, None);
    let exact = project("Client", &wb, b, Some(serde_json::json!([{"path": b, "print": wb}])));
    std::fs::write(old_path(support), serde_json::to_vec(&serde_json::json!({"version": 3, "projects": [former, exact]})).unwrap()).unwrap();
    let read: Vec<Vec<Member>> = old(support).into_iter().map(|p| p.members).collect();
    assert_eq!(read, [vec![Member::new(Some(a), Some(&wa))], vec![Member::new(Some(b), Some(&wb))]], "each at its recorded path");
    // Twin: the path after each mark is the folder's old name.
    let after = |w: &str| w.strip_prefix(crate::library::WAITING_MARK).map(str::to_owned);
    assert_eq!((after(&wa), after(&wb)), (Some("/Roll 8/a.NEF".into()), Some("/Roll 8/b.NEF".into())));
}

/// A former file whose recorded paths went stale, decomposed or backslashed: every member found
/// again by its print, the file untouched by the reading.
#[test]
fn a_former_files_stale_and_respelt_paths_are_found_by_their_prints() {
    let scratch = Scratch::new("projects-stale");
    let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
    let row = |path: &str, print: &str| Entry { captured_at: Some(stamp()), ..Entry::new(path, stamp(), print) };
    let (moved, accented, windows, other) = ("/Roll 8 moved/a.NEF", "/Roll 8/\u{c9}t\u{e9}.NEF", "/Scans/c.NEF", "/Roll 8/b.NEF");
    for (path, print) in [(moved, "1-a"), (accented, "1-e"), (windows, "1-c"), (other, "1-other")] {
        index.upsert(&row(path, print)).unwrap();
    }
    former(scratch.path(), Id::new(), &["1-a", "1-e", "1-c", "1-b"], &[("1-a", "/Roll 8/a.NEF"), ("1-e", "/Roll 8/E\u{301}te\u{301}.NEF"), ("1-c", r"D:\Scans\c.NEF"), ("1-b", other)]);
    let original = std::fs::read(old_path(scratch.path())).unwrap();
    let projects = old(scratch.path());
    let standing = project_standing::standing(&projects, &[], &index, &Default::default());
    let shown: Vec<String> = grouped(&projects, &Reach::new(&standing.rows, &standing.found))[0].entries.iter().map(|e| e.path.clone()).collect();
    assert_eq!(shown, [moved, accented, windows, "/Roll 8/b.NEF/b.NEF"], "the stale, the accented and the backslashed found; b missing, as its place holds another file");
    assert_eq!(projects[0].members.len(), 4, "b stays a member, its print kept for a later read");
    assert_eq!(std::fs::read(old_path(scratch.path())).unwrap(), original, "reading writes nothing");
    // Twin: by their recorded paths alone, only the other file at b's place would show.
    let blind: Vec<Project> = projects.iter().map(|p| Project { members: p.members.iter().map(|m| Member { print: None, ..m.clone() }).collect(), ..p.clone() }).collect();
    let rows: Vec<Entry> = index.all();
    let by_path: Vec<String> = grouped(&blind, &Reach::rows(&rows))[0].entries.iter().map(|e| e.path.clone()).collect();
    assert_eq!(by_path, if cfg!(target_os = "macos") { vec![accented, other] } else { vec![other] });
}
