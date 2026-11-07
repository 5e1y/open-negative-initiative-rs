//! 1.5.6's placeholder beside a print of the same photo, as 1.5.6 leaves it when a photo that joined while
//! its tile loaded is filed elsewhere once read: the photo shows once, at the print, the placeholder kept as given.

use super::shipped_tests::{learned_off, row, shipped, shown, written_back};
use super::*;
use crate::index::LibraryIndex;
use crate::library::WAITING_MARK;
use crate::merge;
use crate::project_standing::{self, Standing};
use crate::scratch::Scratch;
use std::collections::HashSet;
use std::path::Path;

const A: &str = "/Roll 9/a.NEF";
const B: &str = "/Roll 9/b.NEF";
const C: &str = "/Roll 9/c.NEF";
const D: &str = "/Roll 9/d.NEF";

/// Trip's placeholder, named before 1.5.6 renamed its folder, which moved its recorded path alone.
fn mark() -> String {
    format!("{WAITING_MARK}/Roll 9 scans/a.NEF")
}

fn made(name: &str, at: f64, texts: &[(&str, &str)]) -> shipped::Project {
    shipped::Project {
        id: Id::new(),
        name: name.into(),
        description: String::new(),
        created_at: Date::since_reference_date(at),
        manual_order: None,
        members: texts.iter().map(|(t, _)| t.to_string()).collect(),
        paths: Some(texts.iter().map(|(t, p)| (t.to_string(), p.to_string())).collect()),
    }
}

fn wrote(support: &Path, projects: Vec<shipped::Project>) -> shipped::ProjectsFile {
    let file = shipped::ProjectsFile { version: 1, projects };
    std::fs::write(file_path(support), serde_json::to_vec(&file).unwrap()).unwrap();
    file
}

/// 1.5.6's file: Portfolio, older, holds b then a by their prints, a dragged there once read; Trip holds
/// a's placeholder, then c.
fn pair(support: &Path) -> shipped::ProjectsFile {
    let w = mark();
    wrote(support, vec![made("Portfolio", 8e8, &[("1-b", B), ("1-a", A)]), made("Trip", 9e8, &[(&w, A), ("1-c", C)])])
}

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

/// Roll 9 as read: each file carrying the print 1.5.6 recorded.
fn read_roll() -> LibraryIndex {
    index(&[(A, "1-a"), (B, "1-b"), (C, "1-c"), (D, "1-d")])
}

fn picked(paths: &[&str]) -> HashSet<String> {
    paths.iter().map(|p| p.to_string()).collect()
}

fn members(projects: &[Project]) -> Vec<Vec<Member>> {
    projects.iter().map(|p| p.members.clone()).collect()
}

fn of(path: &str, print: &str) -> Member {
    Member::new(Some(path), Some(print))
}

/// The projects showing `path`, by name, with the print of the row each shows there.
fn showing(projects: &[Project], index: &LibraryIndex, path: &str) -> Vec<(String, String)> {
    let st = standing(projects, index);
    let groups = grouped(projects, &Reach::new(&st.rows, &st.found));
    groups.into_iter().flat_map(|g| g.entries.into_iter().filter(|e| e.path == path).map(move |e| (g.project.name.clone(), e.fingerprint))).collect()
}

/// The placeholder a member with no print, as #1550 to #1563 read it.
fn printless(projects: &[Project]) -> Vec<Project> {
    let lay = |m: &Member| if m.placeholder().is_some() { Member::new(m.path.as_deref(), None) } else { m.clone() };
    projects.iter().map(|p| Project { members: p.members.iter().map(lay).collect(), ..p.clone() }).collect()
}

/// Review 4: a dragged to Portfolio after 1.5.6 put it in the newer Trip while its tile loaded stays in
/// Portfolio once read, shows in both before, and nothing is learned, so the launch writes nothing.
#[test]
fn a_later_move_to_portfolio_stands_over_the_placeholder() {
    let scratch = Scratch::new("projects-pair-later");
    pair(scratch.path());
    let read = load(scratch.path()).projects;
    assert_eq!(members(&read), [vec![of(B, "1-b"), of(A, "1-a")], vec![of(A, &mark()), of(C, "1-c")]], "both kept, the placeholder as given");
    let waits = format!("{WAITING_MARK}{A}");
    for (index, shown) in [(index(&[]), vec!["Trip", "Portfolio"]), (read_roll(), vec!["Portfolio"])] {
        let print = index.row(A).map_or(waits.clone(), |r| r.fingerprint);
        assert_eq!(showing(&read, &index, A), shown.into_iter().map(|name| (name.to_string(), print.clone())).collect::<Vec<_>>());
        let mut learned = read.clone();
        assert!(!learned_off(&mut learned, &index), "a placeholder beside a print learns nothing");
    }
    // Twin: read as a member with no print, the newer Trip keeps a and Portfolio loses it.
    assert!(!exclusive(&printless(&read))[0].paths().contains(&A));
}

/// Portfolio's print recorded where a no longer lies, its file moved outside ONI to where Trip's
/// placeholder names: the print is found there, so the photo shows once, in Portfolio.
#[test]
fn a_print_found_at_the_placeholders_file_stands_for_it() {
    let scratch = Scratch::new("projects-pair-found");
    let w = mark();
    wrote(scratch.path(), vec![made("Portfolio", 8e8, &[("1-a", "/Roll 9 old/a.NEF")]), made("Trip", 9e8, &[(&w, A)])]);
    let read = load(scratch.path()).projects;
    let roll = read_roll();
    assert_eq!(showing(&read, &roll, A), [("Portfolio".to_string(), "1-a".to_string())]);
    // Twin: a member claiming that file, as one with no print does, leaves Portfolio's print a missing tile.
    assert_eq!(showing(&printless(&read), &roll, A), [("Trip".to_string(), "1-a".to_string())]);
}

/// Two placeholders of one text are one member, kept by the newest project as 1.5.6 keeps it, wherever
/// each was recorded; two texts naming one file show it once, in the newest project.
#[test]
fn two_placeholders_at_one_file_show_it_once() {
    let scratch = Scratch::new("projects-pair-twice");
    let (w, moved) = (mark(), "/Roll 9b/a.NEF");
    wrote(scratch.path(), vec![made("Client", 7e8, &[(&w, moved)]), made("Trip", 9e8, &[(&w, A)])]);
    assert_eq!(members(&load(scratch.path()).projects), [vec![], vec![of(A, &w)]], "one text, one member, the newest's");
    let other = format!("{WAITING_MARK}{A}");
    let both = |client: f64| {
        wrote(scratch.path(), vec![made("Client", client, &[(&other, A)]), made("Trip", 9e8, &[(&w, A)])]);
        showing(&load(scratch.path()).projects, &read_roll(), A)
    };
    assert_eq!(both(7e8), [("Trip".to_string(), "1-a".to_string())]);
    // Twin: Client made last shows it there, so the order the check reads is the projects' dates.
    assert_eq!(both(9.5e8), [("Client".to_string(), "1-a".to_string())]);
    // Twin: weighed by their paths, as other members are, the one text stays in both.
    let at = |name: &str, path: &str, when: f64| Project::new(name, Date::since_reference_date(when), vec![Member::new(Some(path), None)]);
    assert_eq!(exclusive(&[at("Client", moved, 7e8), at("Trip", A, 9e8)])[0].members.len(), 1);
}

/// Review 5: Portfolio held 12.tif by its print before it was scanned again, and 1.5.6 named the new scan
/// in Trip while its tile loaded: the new scan shows in Trip, Portfolio's print as a missing tile.
#[test]
fn a_rescan_shows_in_trip_and_the_stale_print_as_a_missing_tile() {
    let scratch = Scratch::new("projects-pair-rescan");
    let twelve = "/Roll 9/12.tif";
    let w = format!("{WAITING_MARK}{twelve}");
    wrote(scratch.path(), vec![made("Portfolio", 8e8, &[("1-12", twelve)]), made("Trip", 9e8, &[(&w, twelve)])]);
    let read = load(scratch.path()).projects;
    let rescanned = index(&[(twelve, "2-12")]);
    assert_eq!(showing(&read, &rescanned, twelve), [("Trip".to_string(), "2-12".to_string())]);
    let st = standing(&read, &rescanned);
    let portfolio = &grouped(&read, &Reach::new(&st.rows, &st.found))[1];
    assert_eq!(portfolio.project.name, "Portfolio");
    assert_eq!(portfolio.entries.iter().map(|e| (e.path.as_str(), Member::of(e, None).print)).collect::<Vec<_>>(), [("/Roll 9/12.tif/12.tif", None)], "a missing tile");
    // Unread, nothing proves Portfolio's print there: both show it, so neither prompt says the other keeps it.
    let unread = index(&[]);
    assert_eq!(showing(&read, &unread, twelve), [("Trip".to_string(), w.clone()), ("Portfolio".to_string(), w.clone())]);
    let st = standing(&read, &unread);
    assert_eq!(shadowed(&read, &Reach::new(&st.rows, &st.found)), [0, 0]);
    // Twin: by the row each member holds alone, as before, Portfolio's print hides Trip's photo unread.
    assert_eq!(holders(&read, &Reach::new(&st.rows, &st.found)).get(twelve), Some(&(0, 0)));
    // Twin: a print standing at its path whatever its file carries would take the new scan from Trip.
    let blind: Vec<Project> = read.iter().map(|p| Project { members: p.members.iter().map(|m| if m.placeholder().is_some() { m.clone() } else { Member::new(m.path.as_deref(), None) }).collect(), ..p.clone() }).collect();
    assert_eq!(showing(&blind, &rescanned, twelve), [("Portfolio".to_string(), "2-12".to_string())]);
}

/// Review 6: before Roll 9 is read, a shows in Portfolio and in Trip, so a drop beside it lands in neither;
/// once read, a shows once, in Portfolio, and b dropped past it lands there.
#[test]
fn a_drop_beside_the_pair_waits_for_its_file_then_lands_in_portfolio() {
    let scratch = Scratch::new("projects-pair-drop");
    pair(scratch.path());
    let unread = index(&[]);
    let mut read = load(scratch.path()).projects;
    assert_eq!(showing(&read, &unread, A).len(), 2, "a tile for a in each");
    let st = standing(&read, &unread);
    let reach = Reach::new(&st.rows, &st.found);
    assert_eq!(member_projects(&read, &reach, &picked(&[A])).iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Portfolio", "Trip"]);
    assert!(!reorder_drop(&mut read.clone(), &reach, &picked(&[B]), A, true), "no one project to land in");
    // Twin: by its holder alone, the drop would land in Portfolio though the line stood at Trip's tile too.
    assert_eq!(holders(&read, &reach).get(A), Some(&(0, 1)));
    let roll = read_roll();
    let st = standing(&read, &roll);
    assert!(reorder_drop(&mut read, &Reach::new(&st.rows, &st.found), &picked(&[B]), A, true));
    assert_eq!(members(&read), [vec![of(A, "1-a"), of(B, "1-b")], vec![of(A, &mark()), of(C, "1-c")]], "b past a in Portfolio, Trip as given");
}

/// Review 6: an undo of a step in Trip holds its placeholder as it was, lent no print, so a stays in Portfolio.
#[test]
fn an_undo_in_the_placeholders_project_holds_it_as_read() {
    let scratch = Scratch::new("projects-pair-undo");
    let support = scratch.path();
    pair(support);
    let read = load(support).projects;
    let mut ours = read.clone();
    let st = standing(&ours, &index(&[]));
    let reach = Reach::new(&st.rows, &st.found);
    let before = rosters(&ours);
    assert_eq!(remove_from(&mut ours, &reach, &picked(&[C]), read[1].id), [C]);
    let after = rosters(&ours);
    let base = merge::save_projects(support, &read, &ours).unwrap();
    let mut undone = base.clone();
    regroup(&mut undone, &after, &before);
    assert_eq!(members(&undone), members(&read), "c back in Trip, the placeholder as given");
    let saved = merge::save_projects(support, &base, &undone).unwrap();
    assert_eq!(load(support).projects, saved, "written and read back alike");
    assert_eq!(showing(&saved, &read_roll(), A), [("Portfolio".to_string(), "1-a".to_string())]);
    // Twin: as a member with no print, the undo lends it Portfolio's print and the newer Trip takes a.
    let mut old = printless(&read);
    let (before, mut removed) = (rosters(&old), old.clone());
    remove_from(&mut removed, &reach, &picked(&[C]), read[1].id);
    regroup(&mut old, &rosters(&removed), &before);
    assert_eq!(old[1].members[0], of(A, "1-a"));
    assert!(!exclusive(&old)[0].paths().contains(&A));
}

/// Review 7: a placeholder 1.5.6 added to Trip since this build read the file survives a save of a change
/// here to Trip, merged three ways, member by member: c removed, or Roll 9 renamed in ONI.
#[test]
fn a_merge_keeps_a_placeholder_1_5_6_added_since() {
    let scratch = Scratch::new("projects-pair-merge");
    let support = scratch.path();
    let given = pair(support);
    let base = load(support).projects;
    let mut ours = base.clone();
    ours[1].members.retain(|m| m.path.as_deref() != Some(C));
    let renamed = moved(&base, &[("/Roll 9".into(), "/Roll 9b".into())]);
    assert_eq!(renamed[1].members[0], of("/Roll 9b/a.NEF", &mark()), "the folder carries the recorded path, the text kept");
    let wd = format!("{WAITING_MARK}{D}");
    let mut theirs = given.projects.clone();
    theirs[1].members.push(wd.clone());
    theirs[1].paths.as_mut().unwrap().insert(wd.clone(), D.into());
    theirs.push(made("Client", 7e8, &[("1-d", D)]));
    let theirs = wrote(support, theirs);
    let read_theirs = read(&file_path(support)).unwrap().projects;
    let merged = merge::save_projects(support, &base, &ours).unwrap();
    assert_eq!(merged[1].members, [of(A, &mark()), of(D, &wd)], "c gone, 1.5.6's placeholder for d kept");
    assert_eq!(load(support).projects[1].members, merged[1].members);
    assert_eq!(showing(&merged, &read_roll(), D), [("Client".to_string(), "1-d".to_string())], "d shows once, at Client's print");
    wrote(support, theirs.projects);
    assert!(merge::save_projects(support, &base, &renamed).unwrap()[1].members.contains(&of(D, &wd)), "kept through the rename too");
    // Twin: Trip's members picked whole from this side, which changed them, as a field is, lose d's placeholder.
    let picked = if ours[1].members != base[1].members { &ours[1].members } else { &read_theirs[1].members };
    assert!(!picked.contains(&of(D, &wd)));
}

/// Any save writes each placeholder as 1.5.6 gave it and each print with its path; rewritten by 1.5.6,
/// the file reads back as it was.
#[test]
fn a_save_and_1_5_6_s_rewrite_keep_every_placeholder_text_and_print_path() {
    let scratch = Scratch::new("projects-pair-written");
    let support = scratch.path();
    let given = pair(support);
    let read = load(support).projects;
    let mut renamed = read.clone();
    renamed[0].name = "Folio".into();
    merge::save_projects(support, &read, &renamed).unwrap();
    let text: shipped::ProjectsFile = serde_json::from_slice(&std::fs::read(file_path(support)).unwrap()).expect("1.5.6 parses this format");
    assert_eq!(text.version, FORMAT);
    for (written, given) in text.projects.iter().zip(&given.projects) {
        assert_eq!((&written.members, &written.paths), (&given.members, &given.paths), "{}'s texts and paths as given", given.name);
    }
    assert_eq!(load(support).projects, renamed, "this build reads its exact members back alike");
    let rows = [row(A, "1-a"), row(B, "1-b"), row(C, "1-c")];
    assert_eq!(shown(&text.projects[1], &rows).into_iter().map(|e| e.path.as_str()).collect::<Vec<_>>(), [A, C], "1.5.6 shows a in Trip as before");
    written_back(support, &rows);
    assert_eq!(members(&load(support).projects), members(&read), "the pair as it was, a's print at its path");
    // Twin: the placeholder written as its bare path takes a's path from Portfolio's print in 1.5.6's write.
    save(support, &ProjectsFile { version: FORMAT, projects: printless(&read) }).unwrap();
    written_back(support, &rows);
    assert_eq!(load(support).projects[0].members[1], Member::new(None, Some("1-a")));
}

/// A photo taken out of its projects leaves with every member standing for it, the placeholder it hides
/// included, so none brings it back; one removed from its project alone shows where a placeholder holds it.
#[test]
fn adding_the_photo_to_client_takes_the_hidden_placeholder_with_it() {
    let scratch = Scratch::new("projects-pair-client");
    pair(scratch.path());
    let mut read = load(scratch.path()).projects;
    let client = Project::new("Client", Date::since_reference_date(7e8), vec![]);
    let id = client.id;
    read.push(client);
    let roll = read_roll();
    let st = standing(&read, &roll);
    let reach = Reach::new(&st.rows, &st.found);
    assert_eq!(leaving(&read, &reach, &picked(&[A]), id).iter().map(|(p, n)| (p.name.as_str(), *n)).collect::<Vec<_>>(), [("Portfolio", 1)]);
    let mut added = read.clone();
    assert_eq!(add_to(&mut added, &reach, &picked(&[A]), id), Joined { added: 1, already: 0 });
    assert_eq!(members(&added), [vec![of(B, "1-b")], vec![of(C, "1-c")], vec![of(A, "1-a")]], "Trip's placeholder left with it");
    let mut removed = added.clone();
    let gone = remove_from(&mut removed, &reach, &picked(&[A]), id);
    assert!(showing(&removed, &roll, A).is_empty(), "a in no project");
    assert_eq!(removed_said("Client", gone.len(), &shown_in(&removed, &reach, &picked(&[A]))), "One photo removed from \u{201c}Client\u{201d}.");
    // Removed from Portfolio alone, a shows in Trip, by its placeholder, as 1.5.6 shows it, and the notice says so.
    let mut left = read.clone();
    let gone = remove_from(&mut left, &reach, &picked(&[A]), read[0].id);
    assert_eq!(showing(&left, &roll, A), [("Trip".to_string(), "1-a".to_string())]);
    let said = removed_said("Portfolio", gone.len(), &shown_in(&left, &reach, &picked(&[A])));
    assert_eq!(said, "One photo removed from \u{201c}Portfolio\u{201d}. It now shows in \u{201c}Trip\u{201d}.");
    // Twin: the placeholder left in Trip, as a gesture on the member shown alone leaves it, brings a back.
    removed[1].members.insert(0, of(A, &mark()));
    assert_eq!(showing(&removed, &roll, A), [("Trip".to_string(), "1-a".to_string())]);
}

/// A project naming a twice, by a placeholder its print hides, loses both when a is removed from it,
/// and takes its hidden placeholder away when a joins it from Client, each place kept.
#[test]
fn a_gesture_in_one_project_takes_its_own_hidden_placeholder_with_the_photo() {
    let scratch = Scratch::new("projects-pair-own");
    let w = mark();
    wrote(scratch.path(), vec![made("Client", 7e8, &[("1-c", C)]), made("Portfolio", 8e8, &[(&w, A), ("1-b", B), ("1-a", A)])]);
    let read = load(scratch.path()).projects;
    let roll = read_roll();
    let st = standing(&read, &roll);
    let reach = Reach::new(&st.rows, &st.found);
    let mut removed = read.clone();
    assert_eq!(remove_from(&mut removed, &reach, &picked(&[A]), read[1].id), [A]);
    assert_eq!(removed[1].members, [of(B, "1-b")]);
    assert!(showing(&removed, &roll, A).is_empty(), "a in no project");
    // Twin: the member shown removed alone leaves a in Portfolio, by its placeholder.
    let mut alone = read.clone();
    alone[1].members.remove(2);
    assert_eq!(showing(&alone, &roll, A), [("Portfolio".to_string(), "1-a".to_string())]);
    // a held by Client, Portfolio's placeholder of it hidden: dropped before b in Portfolio, it joins once.
    let mut filed = vec![Project { members: vec![of(A, "1-a")], ..read[0].clone() }, Project { members: vec![of(A, &w), of(B, "1-b")], ..read[1].clone() }];
    let st = standing(&filed, &roll);
    assert!(reorder_drop(&mut filed, &Reach::new(&st.rows, &st.found), &picked(&[A]), B, false));
    assert_eq!(members(&filed), [vec![], vec![of(A, "1-a"), of(B, "1-b")]]);
}

/// A project naming a only by a placeholder Portfolio's print hides shows no photo, holds none in any
/// count or menu, and its delete prompt says its photo stands in another project.
#[test]
fn a_project_holding_only_a_hidden_placeholder_claims_no_photo_and_says_so() {
    let scratch = Scratch::new("projects-pair-alone");
    let w = mark();
    wrote(scratch.path(), vec![made("Portfolio", 8e8, &[("1-a", A)]), made("Trip", 9e8, &[(&w, A)])]);
    let read = load(scratch.path()).projects;
    let roll = read_roll();
    let st = standing(&read, &roll);
    let reach = Reach::new(&st.rows, &st.found);
    let trip = grouped(&read, &reach).into_iter().find(|g| g.project.name == "Trip").unwrap();
    assert!(trip.entries.is_empty(), "its caption reads 0, nothing to open");
    assert_eq!(holders(&read, &reach).get(A), Some(&(0, 0)));
    assert_eq!(member_projects(&read, &reach, &picked(&[A])).len(), 1);
    assert_eq!(shadowed(&read, &reach), [0, 1]);
    let said = delete_prompt(&read[1], 1).1;
    assert_eq!(said, "It shows no photo: the one it names stands in another project, which keeps it. Nothing else is affected.");
    assert_eq!(delete_prompt(&read[0], 0).1, "It stops grouping its 1 photo. The photos, their settings and every other project or roll they belong to are untouched.");
    // Twin: counted as a member, Trip's prompt claims a photo the person cannot see in it.
    assert!(delete_prompt(&read[1], 0).1.starts_with("It stops grouping its 1 photo."));
    // Its file unread, nothing proves Portfolio's print there: Trip shows a too, counts it and says so.
    let st = standing(&read, &index(&[]));
    let reach = Reach::new(&st.rows, &st.found);
    assert_eq!(grouped(&read, &reach).iter().map(|g| (g.project.name.as_str(), g.entries.len())).collect::<Vec<_>>(), [("Trip", 1), ("Portfolio", 1)]);
    assert_eq!(shadowed(&read, &reach), [0, 0]);
    assert_eq!(member_projects(&read, &reach, &picked(&[A])).len(), 2, "Remove from Project offers both");
    let mut out = read.clone();
    assert_eq!(remove_from(&mut out, &reach, &picked(&[A]), read[1].id), [A], "out of Trip, its own placeholder goes");
    assert!(out[1].members.is_empty() && out[0].members == read[0].members);
    // Twin: by the holders alone, as before, Trip holds no member at a to remove.
    assert!(holders(&read, &reach).values().all(|(p, _)| *p == 0));
}
