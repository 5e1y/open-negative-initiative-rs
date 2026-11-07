//! The file as the shipped 1.5.6 build reads and writes it, its types and lookups taken from
//! eac39604: every member it can show, and none lost once it writes the file back without its field.

use super::*;
use crate::index::{Entry, LibraryIndex};
use crate::project_standing;
use crate::scratch::Scratch;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

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

/// 1.5.6's `grouped` for one project: each member at its recorded path, else at the first row of its print.
pub(super) fn shown<'a>(project: &shipped::Project, rows: &'a [Entry]) -> Vec<&'a Entry> {
    let (mut by_print, mut by_path): (HashMap<&str, &Entry>, HashMap<&str, &Entry>) = (HashMap::new(), HashMap::new());
    for e in rows {
        by_print.entry(e.fingerprint.as_str()).or_insert(e);
        by_path.entry(e.path.as_str()).or_insert(e);
    }
    let recorded = |m: &String| project.paths.as_ref().and_then(|p| p.get(m)).and_then(|path| by_path.get(path.as_str()));
    project.members.iter().filter_map(|m| recorded(m).or_else(|| by_print.get(m.as_str())).copied()).collect()
}

/// 1.5.6's save: `located` on each project, then the file written as version 1 by its own types.
pub(super) fn written_back(support: &Path, rows: &[Entry]) {
    let text = std::fs::read(file_path(support)).unwrap();
    let mut file: shipped::ProjectsFile = serde_json::from_slice(&text).expect("1.5.6 parses the file");
    for project in &mut file.projects {
        let resolved: HashMap<String, String> = shown(project, rows).into_iter().rev().map(|e| (e.fingerprint.clone(), e.path.clone())).collect();
        let kept = |m: &String| resolved.get(m).cloned().or_else(|| project.paths.as_ref().and_then(|p| p.get(m)).cloned());
        let paths: BTreeMap<String, String> = project.members.iter().filter_map(|m| Some((m.clone(), kept(m)?))).collect();
        project.paths = (!paths.is_empty()).then_some(paths);
    }
    file.version = 1;
    std::fs::write(file_path(support), serde_json::to_vec(&file).unwrap()).unwrap();
}

/// Two copies of one file in Client, a Swift print, a photo whose print is not read yet: 1.5.6 parses
/// the file, shows what it can find, and writing it back loses none of them.
#[test]
fn the_shipped_build_reads_the_file_and_its_write_back_loses_no_member() {
    let scratch = Scratch::new("projects-shipped");
    let support = scratch.path();
    let (r10, r8, b, c) = ("/r10/a.NEF", "/r8/a.NEF", "/r8/b.NEF", "/r9/c.NEF");
    let rows = [row(r10, "1-a"), row(r8, "1-a"), row(b, "1-b"), row(c, "1-c")];
    let client = Project::new("Client", stamp(), vec![Member::new(Some(r10), Some("1-a")), Member::new(Some(r8), Some("1-a")), Member::new(None, Some("1-s"))]);
    let other = Project::new("Other", Date::since_reference_date(9e8), vec![Member::new(Some(b), Some("1-b")), Member::new(Some(c), None)]);
    let ours = vec![client, other];
    save(support, &ProjectsFile { version: FORMAT, projects: ours.clone() }).unwrap();
    assert_eq!(load(support).projects, ours, "this build reads its exact members back");

    let text = std::fs::read(file_path(support)).unwrap();
    let file: shipped::ProjectsFile = serde_json::from_slice(&text).expect("1.5.6 parses the file");
    let seen: Vec<Vec<&str>> = file.projects.iter().map(|p| shown(p, &rows).into_iter().map(|e| e.path.as_str()).collect()).collect();
    assert_eq!(seen, [vec![r10, r8], vec![b, c]], "both copies and the unread photo show in 1.5.6");
    let texts: Vec<&String> = file.projects.iter().flat_map(|p| &p.members).collect();
    assert_eq!(texts.iter().collect::<std::collections::HashSet<_>>().len(), texts.len(), "1.5.6's exclusive merges no two");

    written_back(support, &rows);
    let back = load(support).projects;
    assert_eq!(back.iter().map(|p| p.members.len()).collect::<Vec<_>>(), [3, 2], "every member kept");
    assert_eq!(back[0].members[1], Member::new(Some(r8), None), "the second copy apart, by its path");
    let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
    for r in &rows {
        index.upsert(r).unwrap();
    }
    let mut learned = back.clone();
    assert!(learned_off(&mut learned, &index));
    assert_eq!(learned[0].members, ours[0].members, "its print learned again off its file");

    // Twin: the second copy named by its print, as 1.5.6 names members, is lost in the write-back.
    let mut file: serde_json::Value = serde_json::from_slice(&text).unwrap();
    file["projects"][0]["members"][1] = "1-a".into();
    std::fs::write(file_path(support), serde_json::to_vec(&file).unwrap()).unwrap();
    written_back(support, &rows);
    assert_eq!(load(support).projects[0].members.len(), 2);
}

/// Version 2 wrote members as objects: 1.5.6 could not parse it and set the file aside. A version 2
/// build reading this format took a second copy's path text for a print, and wrote it so.
#[test]
fn the_former_object_members_are_what_the_shipped_build_cannot_parse() {
    let scratch = Scratch::new("projects-objects");
    let support = scratch.path();
    let members = serde_json::json!([{"path": "/r/a.NEF", "print": "1-a"}, {"path": "/r8/a.NEF", "print": "/r8/a.NEF"}]);
    let objects = serde_json::json!({"version": 2, "projects": [{"id": Id::new().to_string(), "name": "Trip", "description": "", "createdAt": 8e8, "members": members}]});
    std::fs::write(file_path(support), serde_json::to_vec(&objects).unwrap()).unwrap();
    assert!(serde_json::from_slice::<shipped::ProjectsFile>(&std::fs::read(file_path(support)).unwrap()).is_err());
    let read = load(support).projects;
    assert_eq!(read[0].members, [Member::new(Some("/r/a.NEF"), Some("1-a")), Member::new(Some("/r8/a.NEF"), None)], "this build reads it, a path never a print");
    // Twin: the path taken as its print disagrees with every file, the photo's own included.
    assert!(!Member::new(Some("/r8/a.NEF"), Some("/r8/a.NEF")).agrees("1-a"));
    let mut renamed = read.clone();
    renamed[0].name = "Porto".into();
    crate::merge::save_projects(support, &read, &renamed).unwrap();
    let text = std::fs::read(file_path(support)).unwrap();
    assert!(serde_json::from_slice::<shipped::ProjectsFile>(&text).is_ok(), "the next write is 1.5.6's to read");
    assert!(former_path(support, 2).exists(), "the object file kept beside, once");
}

/// 1.5.6 resolves a stale print on the other copy and records its path: the print gives that path
/// up, so both members stay, one found by its print, one at its path.
#[test]
fn a_print_recorded_on_its_copys_path_keeps_both_members() {
    let scratch = Scratch::new("projects-swapped");
    let support = scratch.path();
    let (gone, copy) = ("/r10/a.NEF", "/r8/a.NEF");
    let client = Project::new("Client", stamp(), vec![Member::new(Some(gone), Some("1-a")), Member::new(Some(copy), Some("1-a"))]);
    save(support, &ProjectsFile { version: FORMAT, projects: vec![client] }).unwrap();
    written_back(support, &[row(copy, "1-a")]);
    let text: shipped::ProjectsFile = serde_json::from_slice(&std::fs::read(file_path(support)).unwrap()).unwrap();
    assert_eq!(text.projects[0].paths.as_ref().unwrap()["1-a"], copy, "1.5.6 recorded the copy's path for the print");
    assert_eq!(load(support).projects[0].members, [Member::new(None, Some("1-a")), Member::new(Some(copy), None)]);
    // Twin: the print keeping the recorded path makes the two one photo.
    assert_eq!(distinct(vec![Member::new(Some(copy), Some("1-a")), Member::new(Some(copy), None)]).len(), 1);
}

/// A file a newer build wrote is read as far as this one can, never set aside and never written.
#[test]
fn a_file_from_a_newer_build_is_never_set_aside_or_written() {
    let scratch = Scratch::new("projects-newer");
    let support = scratch.path();
    let id = Id::new();
    let newer = serde_json::json!({"version": FORMAT + 1, "projects": [{"id": id.to_string(), "name": "Trip", "description": "", "createdAt": 8e8, "members": ["1-a"], "more": 1}]});
    std::fs::write(file_path(support), serde_json::to_vec(&newer).unwrap()).unwrap();
    let original = std::fs::read(file_path(support)).unwrap();
    let read = load(support).projects;
    assert_eq!(read[0].members, [Member::new(None, Some("1-a"))], "read as far as this build can");
    let mut renamed = read.clone();
    renamed[0].name = "Porto".into();
    assert!(crate::merge::save_projects(support, &read, &renamed).is_err(), "the write refused");
    assert_eq!(std::fs::read(file_path(support)).unwrap(), original);
    // A newer file this build cannot parse stays where it is.
    let unknown = serde_json::json!({"version": FORMAT + 1, "projects": {"by": "id"}});
    std::fs::write(file_path(support), serde_json::to_vec(&unknown).unwrap()).unwrap();
    assert!(load(support).projects.is_empty());
    assert!(file_path(support).exists(), "never set aside");
    // Twin: the same text from a known version is set aside as unreadable.
    let known = serde_json::json!({"version": 1, "projects": {"by": "id"}});
    std::fs::write(file_path(support), serde_json::to_vec(&known).unwrap()).unwrap();
    assert!(load(support).projects.is_empty());
    assert!(!file_path(support).exists());
}

/// 1.5.6 names a photo added while its tile loaded by its placeholder, its path beside: alone at its file
/// once read, it takes that file's print and is written as it, the text 1.5.6's gestures match.
#[test]
fn a_placeholder_alone_at_its_read_file_takes_its_print_and_is_written_so() {
    let scratch = Scratch::new("projects-placeholder");
    let support = scratch.path();
    let (a, b) = ("/r/a.NEF", "/r/b.NEF");
    let waiting = format!("{}{a}", crate::library::WAITING_MARK);
    let recorded = BTreeMap::from([(waiting.clone(), a.to_owned()), ("1-b".to_owned(), b.to_owned())]);
    let trip = shipped::Project { id: Id::new(), name: "Trip".into(), description: String::new(), created_at: stamp(), manual_order: None, members: vec![waiting.clone(), "1-b".into()], paths: Some(recorded) };
    let file = shipped::ProjectsFile { version: 1, projects: vec![trip.clone()] };
    std::fs::write(file_path(support), serde_json::to_vec(&file).unwrap()).unwrap();
    let read = load(support).projects;
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
    assert_eq!(crate::merge::save_projects(support, &read, &learned).unwrap(), learned);
    let text: shipped::ProjectsFile = serde_json::from_slice(&std::fs::read(file_path(support)).unwrap()).unwrap();
    assert_eq!((&text.projects[0].members, text.projects[0].paths.as_ref().unwrap().get("1-a")), (&vec!["1-a".to_string(), "1-b".into()], Some(&a.to_string())));
    assert!(text.projects[0].members.contains(&rows[0].fingerprint), "1.5.6 matches a in Trip by the print its tile carries");
    // A project 1.5.6 deleted since stays deleted when this side only learned its placeholder's print.
    assert!(crate::merge::projects(&read, &learned, &[]).is_empty());
    // Twin: with that print not forgiven, the learning reads as a change and brings the project back.
    assert_ne!(Project { members: printed(&read[0].members, &learned[0].members), ..read[0].clone() }, learned[0]);
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
            std::fs::write(file_path(support), serde_json::to_vec(&file).unwrap()).unwrap();
            let read = load(support).projects;
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
            crate::merge::save_projects(support, &read, &renamed).unwrap();
            assert_eq!(members(&load(support).projects), kept, "the next save keeps both");
        }
    }
    // Twin: once Trip's member has learned 1-a, the dates alone give a to the newer Trip.
    let printed = |name: &str, at: f64| Project::new(name, Date::since_reference_date(at), vec![Member::new(Some(a), Some("1-a"))]);
    assert!(exclusive(&[printed("Trip", 9e8), printed("Portfolio", 8e8)])[1].members.is_empty());
}
