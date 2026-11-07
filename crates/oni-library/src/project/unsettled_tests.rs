//! The pair 1.5.6 leaves when a photo joined while its tile loaded is dragged elsewhere once read: Trip's
//! placeholder and Portfolio's print at one path, before that file is read again, as after each switch of build.

use super::shipped_tests::{row, shipped, shown, written_back};
use super::*;
use crate::index::LibraryIndex;
use crate::library::WAITING_MARK;
use crate::project_standing::{self, Standing};
use crate::scratch::Scratch;
use crate::merge;
use std::collections::HashSet;
use std::path::Path;

const A: &str = "/Roll 9/a.NEF";
const B: &str = "/Roll 9/b.NEF";
const C: &str = "/Roll 9/c.NEF";

/// Trip's placeholder, named before 1.5.6 renamed its folder, which moved its recorded path alone.
fn mark() -> String {
    format!("{WAITING_MARK}/Roll 9 scans/a.NEF")
}

/// 1.5.6's file: Portfolio, older, holds b then a by their prints; Trip holds a's placeholder, then c.
fn pair(support: &Path) -> shipped::ProjectsFile {
    let made = |name: &str, at: f64, texts: &[(&str, &str)]| shipped::Project {
        id: Id::new(),
        name: name.into(),
        description: String::new(),
        created_at: Date::since_reference_date(at),
        manual_order: None,
        members: texts.iter().map(|(t, _)| t.to_string()).collect(),
        paths: Some(texts.iter().map(|(t, p)| (t.to_string(), p.to_string())).collect()),
    };
    let w = mark();
    let file = shipped::ProjectsFile { version: 1, projects: vec![made("Portfolio", 8e8, &[("1-b", B), ("1-a", A)]), made("Trip", 9e8, &[(&w, A), ("1-c", C)])] };
    std::fs::write(file_path(support), serde_json::to_vec(&file).unwrap()).unwrap();
    file
}

/// Where the members stand while Roll 9 is unread: each a missing tile at its path.
fn standing(projects: &[Project], index: &LibraryIndex) -> Standing {
    project_standing::standing(projects, &[], index, &Found::new())
}

fn index(rows: &[(&str, &str)]) -> LibraryIndex {
    let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
    for (path, print) in rows {
        index.upsert(&row(path, print)).unwrap();
    }
    index
}

fn picked(paths: &[&str]) -> HashSet<String> {
    paths.iter().map(|p| p.to_string()).collect()
}

fn members(projects: &[Project]) -> Vec<Vec<Member>> {
    projects.iter().map(|p| p.members.clone()).collect()
}

/// The placeholder a member at its place, as every read made it before it was held apart.
fn as_before(projects: Vec<Project>) -> Vec<Project> {
    let lay = |mut p: Project| {
        for u in std::mem::take(&mut p.unsettled) {
            p.members.insert(u.at.min(p.members.len()), Member::new(Some(&u.path), None));
        }
        p
    };
    projects.into_iter().map(lay).collect()
}

fn of(path: &str, print: &str) -> Member {
    Member::new(Some(path), Some(print))
}

/// a stands in Portfolio alone, so b dropped past it there lands there, and Trip gains nothing.
#[test]
fn a_drop_beside_the_pair_lands_in_the_project_its_line_shows() {
    let scratch = Scratch::new("projects-pair-drop");
    pair(scratch.path());
    let unread = index(&[]);
    let mut read = load(scratch.path()).projects;
    let st = standing(&read, &unread);
    let reach = Reach::new(&st.rows, &st.found);
    let at_a: Vec<String> = grouped(&read, &reach).into_iter().filter(|g| g.entries.iter().any(|e| e.path == A)).map(|g| g.project.name).collect();
    assert_eq!(at_a, ["Portfolio"], "one tile for a, where its print stands");
    assert!(reorder_drop(&mut read, &reach, &picked(&[B]), A, true));
    assert_eq!(members(&read), [vec![of(A, "1-a"), of(B, "1-b")], vec![of(C, "1-c")]], "b past a in Portfolio, Trip as it was");
    assert_eq!(read[1].unsettled.len(), 1);
    // Twin: the placeholder a member, as read before, holds a for Trip, and b leaves Portfolio for it.
    let mut before = as_before(load(scratch.path()).projects);
    let st = standing(&before, &unread);
    assert!(reorder_drop(&mut before, &Reach::new(&st.rows, &st.found), &picked(&[B]), A, true));
    assert!(before[1].paths().contains(&B) && !before[0].paths().contains(&B));
}

/// An undo of a step in Trip lends its placeholder no print, so the save leaves a in Portfolio
/// whatever the dates; the file read at last then settles it.
#[test]
fn an_undo_in_the_placeholders_project_leaves_it_apart() {
    let scratch = Scratch::new("projects-pair-undo");
    let support = scratch.path();
    pair(support);
    let read = load(support).projects;
    let mut ours = read.clone();
    let st = standing(&ours, &index(&[]));
    let reach = Reach::new(&st.rows, &st.found);
    let before = rosters(&ours);
    assert_eq!(remove_from(&mut ours, &reach, &picked(&[C]), read[1].id), 1);
    let after = rosters(&ours);
    let base = merge::save_projects(support, &read, &ours).unwrap();
    let mut undone = base.clone();
    regroup(&mut undone, &after, &before);
    assert_eq!(members(&undone), [vec![of(B, "1-b"), of(A, "1-a")], vec![of(C, "1-c")]], "c back in Trip, a in Portfolio");
    assert_eq!(undone[1].unsettled, read[1].unsettled, "the placeholder apart, lent no print");
    let saved = merge::save_projects(support, &base, &undone).unwrap();
    assert_eq!(load(support).projects, saved, "written and read back alike");
    for (carried, trip) in [("1-a", vec![of(C, "1-c")]), ("2-a", vec![of(A, "2-a"), of(C, "1-c")])] {
        let mut learned = saved.clone();
        assert!(project_standing::learn(&mut learned, &[], &index(&[(A, carried)])));
        assert_eq!(members(&learned), [vec![of(B, "1-b"), of(A, "1-a")], trip], "{carried} at a");
    }
    // Twin: the placeholder a member, as read before: the undo lends it Portfolio's print, and the
    // save's one project per photo keeps a in the newer Trip only.
    let mut old = as_before(read.clone());
    let st = standing(&old, &index(&[]));
    let reach = Reach::new(&st.rows, &st.found);
    let (before, mut removed) = (rosters(&old), old.clone());
    remove_from(&mut removed, &reach, &picked(&[C]), read[1].id);
    regroup(&mut old, &rosters(&removed), &before);
    assert_eq!(old[1].members[0], of(A, "1-a"));
    assert!(!exclusive(&old)[0].paths().contains(&A));
}

/// Any save before a is read writes Trip's placeholder as 1.5.6 gave it, which 1.5.6 shows as before;
/// rewritten by 1.5.6, Portfolio's print keeps its path and the pair reads back as it was.
#[test]
fn a_save_1_5_6_rewrites_keeps_the_pair_as_it_gave_it() {
    let scratch = Scratch::new("projects-pair-written");
    let support = scratch.path();
    let given = pair(support);
    let read = load(support).projects;
    let mut renamed = read.clone();
    renamed[0].name = "Folio".into();
    merge::save_projects(support, &read, &renamed).unwrap();
    let text: shipped::ProjectsFile = serde_json::from_slice(&std::fs::read(file_path(support)).unwrap()).unwrap();
    assert_eq!((&text.projects[1].members, &text.projects[1].paths), (&given.projects[1].members, &given.projects[1].paths), "Trip's texts as given");
    let rows = [row(A, "1-a"), row(B, "1-b"), row(C, "1-c")];
    let seen: Vec<&str> = shown(&text.projects[1], &rows).into_iter().map(|e| e.path.as_str()).collect();
    assert_eq!(seen, [A, C], "1.5.6 shows a in Trip as before");
    written_back(support, &rows);
    let back = load(support).projects;
    assert_eq!((members(&back), &back[1].unsettled), (members(&read), &read[1].unsettled), "the pair as it was, a's print at its path");
    // Twin: the placeholder a member, written as its bare path, takes a's path from Portfolio's print in 1.5.6's write.
    save(support, &ProjectsFile { version: FORMAT, projects: as_before(read) }).unwrap();
    written_back(support, &rows);
    assert_eq!(load(support).projects[0].members[1], Member::new(None, Some("1-a")));
}

/// What happens to a before its file is read leaves the decision to that file: carrying the print
/// held against the placeholder, a stays where the hand put it; another file joins Trip; none, it waits.
#[test]
fn the_file_read_at_last_settles_the_placeholder_whatever_moved_since() {
    let scratch = Scratch::new("projects-pair-settled");
    let support = scratch.path();
    pair(support);
    let read = load(support).projects;
    let reach_of = |projects: &[Project]| standing(projects, &index(&[]));
    // a moved to Client, then Roll 9 renamed in ONI: the placeholder's recorded path follows, its text stays.
    let mut moving = read.clone();
    let client = Project::new("Client", Date::since_reference_date(7e8), vec![]);
    let id = client.id;
    moving.push(client);
    let st = reach_of(&moving);
    add_to(&mut moving, &Reach::new(&st.rows, &st.found), &picked(&[A]), id);
    let moving = moved(&moving, &[("/Roll 9".into(), "/Roll 9b".into())]);
    assert_eq!((moving[1].unsettled[0].path.as_str(), &moving[1].unsettled[0].text), ("/Roll 9b/a.NEF", &mark()));
    let mut settled = moving.clone();
    assert!(project_standing::learn(&mut settled, &[], &index(&[("/Roll 9b/a.NEF", "1-a")])));
    assert_eq!(settled[2].paths(), ["/Roll 9b/a.NEF"], "a stays in Client");
    assert!(settled[1].unsettled.is_empty() && settled[1].paths() == ["/Roll 9b/c.NEF"], "Trip's placeholder dropped");
    // a removed from Portfolio: the same file leaves it in no project, a new scan there joins Trip.
    let mut removed = read.clone();
    let st = reach_of(&removed);
    remove_from(&mut removed, &Reach::new(&st.rows, &st.found), &picked(&[A]), read[0].id);
    for (carried, trip) in [("1-a", vec![C]), ("2-a", vec![A, C])] {
        let mut learned = removed.clone();
        assert!(project_standing::learn(&mut learned, &[], &index(&[(A, carried)])));
        assert_eq!((learned[0].paths(), learned[1].paths()), (vec![B], trip), "{carried} at a");
    }
    // Its file gone, or its disk away: it waits apart, a missing tile in Portfolio, written back as given.
    let mut waiting = read.clone();
    assert!(!project_standing::learn(&mut waiting, &[], &index(&[])));
    assert_eq!(waiting, read);
    let st = reach_of(&waiting);
    let missing = grouped(&waiting, &Reach::new(&st.rows, &st.found));
    assert!(missing.iter().any(|g| g.project.name == "Portfolio" && g.entries.iter().any(|e| e.path == A && e.fingerprint.starts_with(WAITING_MARK))));
    // Twin: weighed against the members alone, a's own file read after its removal brings it back into Trip.
    let mut forgot = removed.clone();
    forgot[1].unsettled[0].against.clear();
    assert!(project_standing::learn(&mut forgot, &[], &index(&[(A, "1-a")])));
    assert_eq!(forgot[1].paths(), [A, C]);
}
