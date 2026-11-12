//! `projects.json` as the shipped 1.5.6 build wrote it, its types taken from eac39604, read by the
//! import: every member it named, placeholders and copies included.

use super::*;
use crate::index::{Entry, LibraryIndex};
use crate::project_standing;
use crate::scratch::Scratch;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// 1.5.6's `Project` and `ProjectsFile`, field for field.
pub(super) mod shipped {
    use oni_settings::{Date, Id};
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Project {
        pub id: Id,
        pub name: String,
        pub description: String,
        #[serde(rename = "createdAt", with = "crate::reference_date")]
        pub created_at: Date,
        #[serde(rename = "manualOrder", skip_serializing_if = "Option::is_none", default)]
        pub manual_order: Option<f64>,
        pub members: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        pub paths: Option<BTreeMap<String, String>>,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct ProjectsFile {
        pub version: i64,
        pub projects: Vec<Project>,
    }
}

fn stamp() -> Date {
    Date::since_reference_date(8e8)
}

pub(super) fn row(path: &str, print: &str) -> Entry {
    Entry { captured_at: Some(stamp()), ..Entry::new(path, stamp(), print) }
}

/// `project_standing::learn` on a first read of `index`, nothing open.
pub(super) fn learned_off(projects: &mut [Project], index: &LibraryIndex) -> bool {
    let stood = project_standing::standing(projects, &[], index, &Found::new());
    project_standing::learn(projects, &[], index, &stood)
}

/// Where builds before `State/` kept the projects.
pub(super) fn old_path(support: &Path) -> PathBuf {
    support.join("projects.json")
}

/// The projects an old file holds, as the import reads them.
pub(super) fn old(support: &Path) -> Vec<Project> {
    crate::migrate::projects::read(&std::fs::read(old_path(support)).unwrap()).expect("the import reads the file")
}

/// Version 2 wrote members as objects: 1.5.6 could not parse it and set the file aside. A version 2
/// build reading this format took a second copy's path text for a print, and wrote it so.
#[test]
fn the_former_object_members_are_what_the_shipped_build_cannot_parse() {
    let scratch = Scratch::new("projects-objects");
    let support = scratch.path();
    let members = serde_json::json!([{"path": "/r/a.NEF", "print": "1-a"}, {"path": "/r8/a.NEF", "print": "/r8/a.NEF"}]);
    let objects = serde_json::json!({"version": 2, "projects": [{"id": Id::new().to_string(), "name": "Trip", "description": "", "createdAt": 8e8, "members": members}]});
    std::fs::write(old_path(support), serde_json::to_vec(&objects).unwrap()).unwrap();
    assert!(serde_json::from_slice::<shipped::ProjectsFile>(&std::fs::read(old_path(support)).unwrap()).is_err());
    let read = old(support);
    assert_eq!(read[0].members, [Member::new(Some("/r/a.NEF"), Some("1-a")), Member::new(Some("/r8/a.NEF"), None)], "the import reads it, a path never a print");
    // Twin: the path taken as its print disagrees with every file, the photo's own included.
    assert!(!Member::new(Some("/r8/a.NEF"), Some("/r8/a.NEF")).agrees("1-a"));
}

/// 1.5.6 resolves a stale print on the other copy and records its path: the print gives that path
/// up, so both members stay, one found by its print, one at its path.
#[test]
fn a_print_recorded_on_its_copys_path_keeps_both_members() {
    let scratch = Scratch::new("projects-swapped");
    let support = scratch.path();
    let copy = "/r8/a.NEF";
    // A format 4 file naming the copy by its path, rewritten by 1.5.6, which found the print on the copy.
    let recorded = BTreeMap::from([("1-a".to_owned(), copy.to_owned()), (copy.to_owned(), copy.to_owned())]);
    let members = vec!["1-a".into(), copy.into()];
    let client = shipped::Project { id: Id::new(), name: "Client".into(), description: String::new(), created_at: stamp(), manual_order: None, members, paths: Some(recorded) };
    std::fs::write(old_path(support), serde_json::to_vec(&shipped::ProjectsFile { version: 1, projects: vec![client] }).unwrap()).unwrap();
    assert_eq!(old(support)[0].members, [Member::new(None, Some("1-a")), Member::new(Some(copy), None)]);
    // Twin: the print keeping the recorded path makes the two one photo.
    assert_eq!(distinct(vec![Member::new(Some(copy), Some("1-a")), Member::new(Some(copy), None)]).len(), 1);
}

/// A file a newer build wrote is read as far as this one can, never set aside and never written.
#[test]
fn a_file_from_a_newer_build_is_never_set_aside_or_written() {
    let scratch = Scratch::new("projects-newer");
    let support = scratch.path();
    std::fs::create_dir_all(file_path(support).parent().unwrap()).unwrap();
    let trip = serde_json::json!({"id": Id::new().to_string(), "name": "Trip", "description": "", "createdAt": stamp().iso8601(), "members": [{"print": "1-a"}], "more": 1});
    let newer = serde_json::json!({"version": FORMAT + 1, "projects": [trip]});
    std::fs::write(file_path(support), serde_json::to_vec(&newer).unwrap()).unwrap();
    let original = std::fs::read(file_path(support)).unwrap();
    let mut read = load(support);
    assert_eq!(read.projects[0].members, [Member::new(None, Some("1-a"))], "read as far as this build can");
    read.projects[0].name = "Porto".into();
    assert!(save(support, &read).is_err(), "the write refused");
    assert_eq!(std::fs::read(file_path(support)).unwrap(), original);
    // A newer file this build cannot parse stays where it is.
    let unknown = serde_json::json!({"version": FORMAT + 1, "projects": {"by": "id"}});
    std::fs::write(file_path(support), serde_json::to_vec(&unknown).unwrap()).unwrap();
    assert!(load(support).projects.is_empty() && load(support).is_newer());
    assert!(file_path(support).exists(), "never set aside");
    // Twin: the same text at this version is set aside as unreadable.
    let known = serde_json::json!({"version": FORMAT, "projects": {"by": "id"}});
    std::fs::write(file_path(support), serde_json::to_vec(&known).unwrap()).unwrap();
    assert!(load(support).projects.is_empty());
    assert!(!file_path(support).exists());
}

/// 1.5.6 names a photo added while its tile loaded by its placeholder, its path beside: alone at its file
/// once read, it takes that file's print and is written with it.
#[test]
fn a_placeholder_alone_at_its_read_file_takes_its_print_and_is_written_so() {
    let scratch = Scratch::new("projects-placeholder");
    let support = scratch.path();
    let (a, b) = ("/r/a.NEF", "/r/b.NEF");
    let waiting = format!("{}{a}", crate::library::WAITING_MARK);
    let recorded = BTreeMap::from([(waiting.clone(), a.to_owned()), ("1-b".to_owned(), b.to_owned())]);
    let trip = shipped::Project { id: Id::new(), name: "Trip".into(), description: String::new(), created_at: stamp(), manual_order: None, members: vec![waiting.clone(), "1-b".into()], paths: Some(recorded) };
    let file = shipped::ProjectsFile { version: 1, projects: vec![trip.clone()] };
    std::fs::write(old_path(support), serde_json::to_vec(&file).unwrap()).unwrap();
    let read = old(support);
    assert_eq!(read[0].members, [Member::new(Some(a), Some(&waiting)), Member::new(Some(b), Some("1-b"))], "the placeholder is its text");
    let mut waited = read.clone();
    assert!(!learned_off(&mut waited, &LibraryIndex::open(Path::new(":memory:")).unwrap()), "its file unread, it learns nothing");
    let rows = [row(a, "1-a"), row(b, "1-b")];
    let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
    rows.iter().for_each(|r| index.upsert(r).unwrap());
    let mut learned = read.clone();
    assert!(learned_off(&mut learned, &index));
    assert_eq!(learned[0].members, [Member::new(Some(a), Some("1-a")), Member::new(Some(b), Some("1-b"))]);
    let found = project_standing::standing(&learned, &[], &index, &Found::new());
    assert!(found.found.is_empty() && found.rows.iter().all(|e| Member::of(e, None).print.is_some()), "each stands at its own file");
    save(support, &ProjectsFile { version: FORMAT, projects: learned.clone() }).unwrap();
    assert_eq!(load(support).projects[0].members, learned[0].members, "written with the print learned");
    // Twin: written as given, the text is one no read tile's print matches.
    assert!(!trip.members.contains(&rows[0].fingerprint));
}

/// 1.5.6 leaves by text, so Trip's placeholder for a and Portfolio's print 1-a both held it: whatever the
/// dates, the load keeps both, the file at a tells once read which one shows it, and unread both do.
#[test]
fn the_file_at_its_place_tells_a_placeholder_from_a_print() {
    let scratch = Scratch::new("projects-placeholder-moved");
    let support = scratch.path();
    let a = "/r/a.NEF";
    let waiting = format!("{}{a}", crate::library::WAITING_MARK);
    let made = |name: &str, at: f64, text: &str| shipped::Project {
        id: Id::new(),
        name: name.into(),
        description: String::new(),
        created_at: Date::since_reference_date(at),
        manual_order: None,
        members: vec![text.into()],
        paths: Some(BTreeMap::from([(text.to_owned(), a.to_owned())])),
    };
    let members = |projects: &[Project]| projects.iter().map(|p| p.members.clone()).collect::<Vec<_>>();
    let both = vec![vec![Member::new(Some(a), Some(&waiting))], vec![Member::new(Some(a), Some("1-a"))]];
    // Portfolio joined a once its tile was read; or held 1-a before a was scanned again, Trip naming the new
    // scan, whose print Trip's placeholder learns; or a not read yet, shown in both.
    for (carried, holding) in [(Some("1-a"), vec!["Portfolio"]), (Some("2-a"), vec!["Trip"]), (None, vec!["Portfolio", "Trip"])] {
        let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
        if let Some(print) = carried {
            index.upsert(&row(a, print)).unwrap();
        }
        let kept = match carried {
            Some("2-a") => vec![vec![Member::new(Some(a), Some("2-a"))], both[1].clone()],
            _ => both.clone(),
        };
        for (trip, portfolio) in [(9e8, 8e8), (8e8, 9e8)] {
            let file = shipped::ProjectsFile { version: 1, projects: vec![made("Trip", trip, &waiting), made("Portfolio", portfolio, "1-a")] };
            std::fs::write(old_path(support), serde_json::to_vec(&file).unwrap()).unwrap();
            let read = old(support);
            assert_eq!(members(&read), both, "the file alone tells nothing: both kept");
            let mut now = read.clone();
            assert_eq!(learned_off(&mut now, &index), kept != both, "{carried:?}: a print learned alone at its file");
            assert_eq!(members(&now), kept);
            let standing = project_standing::standing(&now, &[], &index, &Found::new());
            let mut at_a: Vec<String> = grouped(&now, &Reach::new(&standing.rows, &standing.found)).into_iter().filter(|g| g.entries.iter().any(|e| e.path == a)).map(|g| g.project.name).collect();
            at_a.sort();
            assert_eq!(at_a, holding, "{carried:?} at a");
            let mut renamed = now.clone();
            renamed[0].name = "Tour".into();
            save(support, &ProjectsFile { version: FORMAT, projects: renamed }).unwrap();
            assert_eq!(members(&load(support).projects), kept, "the next save keeps both");
        }
    }
    // Twin: once Trip's member has learned 1-a, the dates alone give a to the newer Trip.
    let printed = |name: &str, at: f64| Project::new(name, Date::since_reference_date(at), vec![Member::new(Some(a), Some("1-a"))]);
    assert!(exclusive(&[printed("Trip", 9e8), printed("Portfolio", 8e8)])[1].members.is_empty());
}
