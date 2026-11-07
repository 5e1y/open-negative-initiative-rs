use super::*;
use crate::index::{Entry, LibraryIndex};
use crate::scratch::Scratch;
use crate::{merge, project_standing};
use std::path::Path;

fn stamp() -> Date {
    Date::since_reference_date(8e8)
}

fn former(support: &Path, id: Id, members: &[&str], paths: &[(&str, &str)]) {
    let paths: serde_json::Map<String, serde_json::Value> = paths.iter().map(|(k, v)| (k.to_string(), (*v).into())).collect();
    let file = serde_json::json!({"version": 1, "projects": [{"id": id.to_string(), "name": "Trip", "description": "", "createdAt": 8e8, "members": members, "paths": paths}]});
    std::fs::write(file_path(support), serde_json::to_vec(&file).unwrap()).unwrap();
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
    let text = std::fs::read_to_string(file_path(scratch.path())).unwrap();
    assert!(text.contains("\"version\" : 2") && text.contains("\"print\" : \"fa\""), "{text}");
    assert!(text.contains("\"createdAt\" : 800000000") && text.contains("\"manualOrder\" : 3"));
    assert!(!former_path(scratch.path(), 2).exists(), "a file in this format is never set aside");
    // Twin: the sidecar's ISO spelling of the same date is not what a project writes.
    assert!(!text.contains(&stamp().iso8601()));
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
    let read = load(scratch.path()).projects;
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

/// Read and laid, a former file is never written; the first change writes this format and keeps
/// the former file beside it, once, byte for byte.
#[test]
fn only_a_change_writes_and_the_former_file_is_kept_beside_it_once() {
    let scratch = Scratch::new("projects-kept");
    let support = scratch.path();
    former(support, Id::new(), &["1-no-date-a"], &[("1-no-date-a", "/r/a.NEF")]);
    let original = std::fs::read(file_path(support)).unwrap();
    let read = load(support).projects;
    let written = merge::save_projects(support, &read, &read).unwrap();
    assert_eq!(written, read);
    assert_eq!(std::fs::read(file_path(support)).unwrap(), original, "nothing changed, nothing written");
    assert!(!former_path(support, 1).exists());

    let mut ours = read.clone();
    ours[0].name = "Lisbon".into();
    let read = merge::save_projects(support, &read, &ours).unwrap();
    assert_eq!(std::fs::read(former_path(support, 1)).unwrap(), original, "the former file kept beside");
    assert_eq!(load(support), ProjectsFile { version: FORMAT, projects: read.clone() });
    assert_eq!(load(support).projects[0].members[0], Member::new(Some("/r/a.NEF"), Some("1-no-date-a")), "its print written beside its path");
    // A second former file never takes the first one's place.
    former(support, Id::new(), &["1-no-date-z"], &[]);
    let again = load(support).projects;
    let mut renamed = again.clone();
    renamed[0].name = "Porto".into();
    merge::save_projects(support, &again, &renamed).unwrap();
    assert_eq!(std::fs::read(former_path(support, 1)).unwrap(), original, "kept once");
    // Twin: a change written in this format sets nothing aside.
    std::fs::remove_file(former_path(support, 1)).unwrap();
    let mut third = load(support).projects;
    third[0].description = "Coast".into();
    merge::save_projects(support, &renamed, &third).unwrap();
    assert!(!former_path(support, 1).exists() && !former_path(support, 2).exists());
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
    let original = std::fs::read(file_path(scratch.path())).unwrap();
    let projects = load(scratch.path()).projects;
    let standing = project_standing::standing(&projects, &[], &index);
    let shown: Vec<String> = grouped(&projects, &Reach::new(&standing.rows, &standing.found))[0].entries.iter().map(|e| e.path.clone()).collect();
    assert_eq!(shown, [moved, accented, windows], "the stale, the accented and the backslashed found; b's place holds another file");
    assert_eq!(projects[0].members.len(), 4, "b stays a member, its print kept for a later read");
    assert_eq!(std::fs::read(file_path(scratch.path())).unwrap(), original, "reading writes nothing");
    // Twin: by their recorded paths alone, only the other file at b's place would show.
    let blind: Vec<Project> = projects.iter().map(|p| Project { members: p.members.iter().map(|m| Member { print: None, ..m.clone() }).collect(), ..p.clone() }).collect();
    let rows: Vec<Entry> = index.all();
    let by_path: Vec<String> = grouped(&blind, &Reach::rows(&rows))[0].entries.iter().map(|e| e.path.clone()).collect();
    assert_eq!(by_path, if cfg!(target_os = "macos") { vec![accented, other] } else { vec![other] });
}
