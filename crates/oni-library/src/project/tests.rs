use super::*;
use crate::index::Entry;
use std::collections::HashSet;

fn stamp() -> Date {
    Date::since_reference_date(8e8)
}

fn row(path: &str, fingerprint: &str) -> Entry {
    let mut e = Entry::new(path, stamp(), fingerprint);
    e.captured_at = Some(stamp());
    e
}

/// A member as a row of `rows` makes it, its print the row's.
fn of(path: &str) -> Member {
    Member::new(Some(path), Some(&format!("f{path}")))
}

fn members(paths: &[&str]) -> Vec<Member> {
    paths.iter().map(|p| of(p)).collect()
}

fn rows(paths: &[&str]) -> Vec<Entry> {
    paths.iter().map(|p| row(p, &format!("f{p}"))).collect()
}

fn picked(paths: &[&str]) -> HashSet<String> {
    paths.iter().map(|s| s.to_string()).collect()
}

fn shown(projects: &[Project], rows: &[Entry]) -> Vec<Vec<String>> {
    grouped(projects, &Reach::rows(rows)).into_iter().map(|g| g.entries.into_iter().map(|e| e.path).collect()).collect()
}

#[test]
fn identity_is_the_id_and_the_free_name_the_lowest_number() {
    let a = Project::new("Iceland", stamp(), vec![]);
    let mut b = a.clone();
    b.description = String::new();
    assert_eq!(a, b);
    assert_ne!(a, Project::new("Iceland", stamp(), vec![]));
    let taken = [Project::new("Project 1", stamp(), vec![]), Project::new("Project 3", stamp(), vec![])];
    assert_eq!(untitled(&taken), "Project 2");
    assert_eq!(untitled(&[]), "Project 1");
}

/// Two rolls holding the same file under the same name: a project keeps both, each by its path.
#[test]
fn two_copies_of_one_file_are_two_members_each_found_at_its_path() {
    let library = [row("/r10/a.NEF", "print-a"), row("/r8/a.NEF", "print-a"), row("/r8/b.NEF", "print-b")];
    let mut projects = vec![Project::new("Client", stamp(), vec![Member::of(&library[0], None)])];
    let id = projects[0].id;
    let done = add_to(&mut projects, &Reach::rows(&library), &picked(&["/r8/a.NEF", "/r8/b.NEF"]), id);
    assert_eq!(done, Joined { added: 2, already: 0 }, "the copy of a member joins it");
    assert_eq!(shown(&projects, &library), [["/r10/a.NEF", "/r8/a.NEF", "/r8/b.NEF"]]);
    assert_eq!(projects[0].members[1], Member::new(Some("/r8/a.NEF"), Some("print-a")), "each member keeps its print");
    // Adding them again changes nothing and says each was there.
    let before = projects.clone();
    assert_eq!(add_to(&mut projects, &Reach::rows(&library), &picked(&["/r8/a.NEF", "/r10/a.NEF"]), id), Joined { added: 0, already: 2 });
    assert_eq!(projects, before);
    // Twin: keyed by print alone, the copy would count as there and b join alone.
    let by_print: HashSet<&str> = library.iter().map(|e| e.fingerprint.as_str()).collect();
    assert_ne!(by_print.len(), projects[0].members.len());
}

/// The print tells another file standing at a member's path: that file is not the member.
#[test]
fn a_member_shows_only_on_the_file_its_print_names() {
    let library = [row("/r/a.NEF", "other"), row("/r/b.NEF", "fb")];
    let trip = Project::new("Trip", stamp(), vec![Member::new(Some("/r/a.NEF"), Some("fa")), Member::new(Some("/r/b.NEF"), Some("fb"))]);
    assert_eq!(shown(std::slice::from_ref(&trip), &library), [["/r/b.NEF"]]);
    // A row still being read cannot tell, so it stands.
    let reading = [row("/r/a.NEF", "waiting-/r/a.NEF")];
    assert_eq!(shown(std::slice::from_ref(&trip), &reading), [["/r/a.NEF"]]);
    // Twin: by its path alone the other file would be the member.
    let blind = Project::new("Blind", stamp(), vec![Member::new(Some("/r/a.NEF"), None)]);
    assert_eq!(shown(&[blind], &library), [["/r/a.NEF"]]);
}

#[test]
fn projects_order_newest_first_and_follow_a_move_made_in_oni() {
    let older = Project::new("Older", Date::since_reference_date(0.0), vec![]);
    let newer = Project::new("Newer", Date::since_reference_date(100.0), vec![]);
    let mut pinned = Project::new("Pinned", Date::since_reference_date(0.0), vec![]);
    pinned.manual_order = Some(200.0);
    let order: Vec<String> = grouped(&[older, newer, pinned], &Reach::rows(&[])).into_iter().map(|g| g.project.name).collect();
    assert_eq!(order, ["Pinned", "Newer", "Older"]);

    let library = rows(&["/negs/a.RW2", "/negs/b.RW2"]);
    let gap = Project::new("Gap", stamp(), members(&["/negs/b.RW2", "/negs/gone.RW2", "/negs/a.RW2"]));
    assert_eq!(shown(std::slice::from_ref(&gap), &library), [["/negs/b.RW2", "/negs/a.RW2"]]);
    assert_eq!(gap.members.len(), 3, "a member no row holds is never pruned by reading");

    let trip = Project::new("Trip", stamp(), members(&["/r/a.RW2", "/s/b.RW2"]));
    let carried = moved(std::slice::from_ref(&trip), &[("/r".into(), "/t/r".into())]);
    assert_eq!(carried[0].paths(), ["/t/r/a.RW2", "/s/b.RW2"]);
    assert_eq!(carried[0].members[0].print, trip.members[0].print, "the print stays with the photo");
    let file = moved(std::slice::from_ref(&trip), &[("/s/b.RW2".into(), "/s/B.RW2".into())]);
    assert_eq!(file[0].paths(), ["/r/a.RW2", "/s/B.RW2"], "a file moved alone is carried too");
    // A photo whose place holds another file stays where it was: the move before the folder's keeps it.
    let kept = moved(std::slice::from_ref(&trip), &[("/r/a.RW2".into(), "/r/a.RW2".into()), ("/r".into(), "/t/r".into())]);
    assert_eq!(kept[0].paths(), ["/r/a.RW2", "/s/b.RW2"]);
    // Twin: a prefix that is not a folder moves nothing.
    let sibling = moved(std::slice::from_ref(&trip), &[("/r/a".into(), "/x".into())]);
    assert_eq!(sibling[0].members, trip.members);
}

/// `/Volumes/T7 1`'s photos added, then the old `/Volumes/T7` located there: the old members land
/// on the new ones, and the project holds each photo once, every print kept.
#[test]
fn a_move_landing_on_a_member_leaves_one_photo() {
    let old = Member::new(Some("/Volumes/T7/Roll/a.NEF"), None);
    let new = Member::new(Some("/Volumes/T7 1/Roll/a.NEF"), Some("fa"));
    let other = Member::new(Some("/Volumes/T7/Roll/b.NEF"), Some("fb"));
    let trip = Project::new("Trip", stamp(), vec![old, other, new.clone()]);
    let carried = moved(std::slice::from_ref(&trip), &[("/Volumes/T7".into(), "/Volumes/T7 1".into())]);
    assert_eq!(carried[0].members, [new.clone(), Member::new(Some("/Volumes/T7 1/Roll/b.NEF"), Some("fb"))], "one a, its print kept");
    // Twin: two prints at one path are two photos, one of them stale, and both stay.
    let stale = Project::new("Stale", stamp(), vec![Member::new(Some("/Volumes/T7/Roll/a.NEF"), Some("fz")), new]);
    assert_eq!(moved(&[stale], &[("/Volumes/T7".into(), "/Volumes/T7 1".into())])[0].members.len(), 2);
}

#[test]
fn a_shared_member_stays_with_the_newest_project() {
    let first = Project::new("First", Date::since_reference_date(0.0), members(&["/r/a", "/r/shared"]));
    let second = Project::new("Second", Date::since_reference_date(100.0), members(&["/r/shared", "/r/b"]));
    let settled: Vec<Vec<Member>> = exclusive(&[first.clone(), second.clone()]).into_iter().map(|p| p.members).collect();
    assert_eq!(settled, [members(&["/r/a"]), members(&["/r/shared", "/r/b"])]);
    // Twin: listed the other way round, creation still decides, not position.
    let swapped: Vec<Vec<Member>> = exclusive(&[second, first]).into_iter().map(|p| p.members).collect();
    assert_eq!(swapped, [members(&["/r/shared", "/r/b"]), members(&["/r/a"])]);
}

#[test]
fn a_new_project_takes_the_picked_photos_in_library_order_and_none_makes_none() {
    let library = rows(&["/r/a", "/r/b", "/r/c", "/r/z"]);
    let mut projects = vec![Project::new("Project 1", stamp(), members(&["/r/z", "/r/c"]))];
    let id = new_from(&mut projects, &Reach::rows(&library), &picked(&["/r/c", "/r/a"]), stamp()).unwrap();
    let made = projects.iter().find(|p| p.id == id).unwrap().clone();
    assert_eq!((made.name.as_str(), made.members.clone()), ("Project 2", members(&["/r/a", "/r/c"])));
    assert_eq!(projects[0].members, members(&["/r/z"]), "c leaves the project it sat in");
    // Twin: the picked set's own order would put c first.
    assert_ne!(made.members, members(&["/r/c", "/r/a"]));
    // Nothing picked makes nothing; the empty tab's key makes an empty project, named.
    assert_eq!(new_from(&mut projects, &Reach::rows(&library), &HashSet::new(), stamp()), None);
    assert_eq!(projects.len(), 2);
    let empty = new_empty(&mut projects, stamp());
    let empty = projects.iter().find(|p| p.id == empty).unwrap();
    assert!(empty.members.is_empty() && empty.name == "Project 3");
}

/// "Add to Project" takes a photo out of the project it sat in, as a photo sits in one: the ask
/// names that project first, and an emptied project stays, never dropped without a word.
#[test]
fn adding_to_a_project_never_drops_another_silently() {
    let library = rows(&["/r/a", "/r/b", "/r/c", "/r/z"]);
    let reach = Reach::rows(&library);
    let first = Project::new("First", stamp(), members(&["/r/a"]));
    let second = Project::new("Second", stamp(), members(&["/r/b", "/r/c"]));
    let target = Project::new("Target", stamp(), members(&["/r/z"]));
    let (first_id, second_id, target_id) = (first.id, second.id, target.id);
    let mut projects = vec![first, second, target];
    let asked: Vec<(&str, usize)> = leaving(&projects, &reach, &picked(&["/r/a", "/r/b"]), target_id).into_iter().map(|(p, n)| (p.name.as_str(), n)).collect();
    assert_eq!(asked, [("First", 1), ("Second", 1)], "the ask names each project a photo leaves");
    assert!(leaving(&projects, &reach, &picked(&["/r/c"]), second_id).is_empty(), "twin: photos already there leave nothing");
    assert_eq!(add_to(&mut projects, &reach, &picked(&["/r/a", "/r/b"]), target_id), Joined { added: 2, already: 0 });
    let first = projects.iter().find(|p| p.id == first_id).expect("First stays, empty, with its name");
    assert!(first.members.is_empty() && first.name == "First");
    assert_eq!(projects.iter().find(|p| p.id == second_id).unwrap().members, members(&["/r/c"]));
    assert_eq!(projects.iter().find(|p| p.id == target_id).unwrap().members, members(&["/r/z", "/r/a", "/r/b"]));
    // Twin: the rule it replaces, emptied projects dropped, loses First and its name.
    let mut dropped = projects.clone();
    dropped.retain(|p| !p.members.is_empty());
    assert!(!dropped.iter().any(|p| p.id == first_id));
    assert_eq!(remove_from(&mut projects, &reach, &picked(&["/r/a", "/r/c"]), target_id).len(), 1, "only the member there is counted");
}

#[test]
fn a_drop_lands_before_its_target_members_first_then_the_joining_ones() {
    let library = rows(&["/r/a", "/r/b", "/r/c", "/r/d", "/r/x"]);
    let reach = Reach::rows(&library);
    let trip = Project::new("Trip", stamp(), members(&["/r/a", "/r/b", "/r/c", "/r/d"]));
    let other = Project::new("Other", stamp(), members(&["/r/x"]));
    let trip_id = trip.id;
    let mut projects = vec![other, trip];
    // d and x dragged before b: d already a member moves first, x joins after it and leaves Other.
    assert!(reorder_drop(&mut projects, &reach, &picked(&["/r/x", "/r/d"]), "/r/b", false));
    assert!(projects[0].name == "Other" && projects[0].members.is_empty(), "Other held only x and stays, empty");
    assert_eq!(projects[1].id, trip_id);
    assert_eq!(projects[1].members, members(&["/r/a", "/r/d", "/r/x", "/r/b", "/r/c"]));
    // Twin: the target dragged onto itself moves nothing.
    assert!(!reorder_drop(&mut projects, &reach, &picked(&["/r/b"]), "/r/b", false));
    // A drop on a photo in no project lands nowhere.
    assert!(!reorder_drop(&mut projects, &reach, &picked(&["/r/a"]), "/r/loose", false));
}

/// P holds a and b, Q holds c; X joins Q, then a is dragged before c. Undoing the drag puts a back
/// in P; undoing the add then takes X alone out of Q, whatever came since, never another member.
#[test]
fn an_undo_turns_back_what_its_step_changed_and_nothing_else() {
    let library = rows(&["/r/a", "/r/b", "/r/c", "/r/x"]);
    let reach = Reach::rows(&library);
    let p = Project::new("P", stamp(), members(&["/r/a", "/r/b"]));
    let q = Project::new("Q", stamp(), members(&["/r/c"]));
    let (p_id, q_id) = (p.id, q.id);
    let mut projects = vec![p, q];
    let changed = |before: &Rosters, after: &Rosters| -> (Rosters, Rosters) {
        let ids: Vec<Id> = after.iter().filter(|(id, list)| before.get(*id) != Some(*list)).map(|(id, _)| *id).collect();
        let keep = |r: &Rosters| r.iter().filter(|(id, _)| ids.contains(id)).map(|(id, l)| (*id, l.clone())).collect();
        (keep(before), keep(after))
    };
    let start = rosters(&projects);
    add_to(&mut projects, &reach, &picked(&["/r/x"]), q_id);
    let (add_from, add_to_) = changed(&start, &rosters(&projects));
    let added = rosters(&projects);
    assert!(reorder_drop(&mut projects, &reach, &picked(&["/r/a"]), "/r/c", false));
    let (drag_from, drag_to) = changed(&added, &rosters(&projects));
    assert_eq!(drag_from.len(), 2, "the drag's step names both projects it changed");
    let find = |projects: &[Project], id: Id| projects.iter().find(|p| p.id == id).unwrap().members.clone();
    regroup(&mut projects, &drag_to, &drag_from);
    assert_eq!((find(&projects, p_id), find(&projects, q_id)), (members(&["/r/a", "/r/b"]), members(&["/r/c", "/r/x"])));
    regroup(&mut projects, &add_to_, &add_from);
    assert_eq!((find(&projects, p_id), find(&projects, q_id)), (members(&["/r/a", "/r/b"]), members(&["/r/c"])));
    // Redo lays both again.
    regroup(&mut projects, &add_from, &add_to_);
    regroup(&mut projects, &drag_from, &drag_to);
    assert_eq!((find(&projects, p_id), find(&projects, q_id)), (members(&["/r/b"]), members(&["/r/a", "/r/c", "/r/x"])));
    // The drag never recorded, the add's undo turned back over it keeps a in Q.
    regroup(&mut projects, &add_to_, &add_from);
    assert_eq!(find(&projects, q_id), members(&["/r/a", "/r/c"]), "only X leaves");
    // Twin: Q's list from before the add laid whole, as a step's end once was, loses a from every project.
    let mut laid = projects.clone();
    laid.iter_mut().filter(|p| p.id == q_id).for_each(|p| p.members = add_from[&q_id].members.clone());
    assert!(!laid.iter().any(|p| p.members.contains(&of("/r/a"))));
}

/// New Project from a and b, which sat in P: its undo gives them back to P and takes the project
/// away, its redo makes it again, the same project; one holding a photo since stays.
#[test]
fn an_undo_of_new_project_takes_the_project_away_and_its_redo_brings_it_back() {
    let library = rows(&["/r/a", "/r/b", "/r/c"]);
    let reach = Reach::rows(&library);
    let p = Project::new("P", stamp(), members(&["/r/a", "/r/b"]));
    let p_id = p.id;
    let mut projects = vec![p];
    let before = rosters(&projects);
    let made = new_from(&mut projects, &reach, &picked(&["/r/a", "/r/b"]), stamp()).unwrap();
    let after = rosters(&projects);
    let step = |r: &Rosters, ids: &[Id]| -> Rosters { r.iter().filter(|(id, _)| ids.contains(id)).map(|(id, p)| (*id, p.clone())).collect() };
    let (from, to) = (step(&before, &[p_id, made]), step(&after, &[p_id, made]));
    regroup(&mut projects, &to, &from);
    assert_eq!(projects.len(), 1, "the project the step made is gone");
    assert_eq!(projects[0].members, members(&["/r/a", "/r/b"]));
    regroup(&mut projects, &from, &to);
    let back = projects.iter().find(|p| p.id == made).expect("the redo brings it back");
    assert_eq!((back.name.as_str(), back.members.clone()), ("Project 1", members(&["/r/a", "/r/b"])));
    assert!(projects.iter().find(|p| p.id == p_id).unwrap().members.is_empty());
    // Twin: a step holding the project at both ends, as its members alone did, leaves it empty.
    let mut both = from.clone();
    both.insert(made, Project { members: Vec::new(), ..to[&made].clone() });
    let mut left = projects.clone();
    regroup(&mut left, &to, &both);
    assert!(left.iter().any(|p| p.id == made && p.members.is_empty()));
    // A photo joining it since keeps it through the undo.
    add_to(&mut projects, &reach, &picked(&["/r/c"]), made);
    regroup(&mut projects, &to, &from);
    assert_eq!(projects.iter().find(|p| p.id == made).map(|p| p.members.clone()), Some(members(&["/r/c"])));
}

/// a dragged from P to Q, then Q deleted by hand: the drag's redo leaves a in P, never in no project.
#[test]
fn a_redo_toward_a_project_deleted_since_leaves_the_photo_where_it_stands() {
    let library = rows(&["/r/a", "/r/b", "/r/c"]);
    let reach = Reach::rows(&library);
    let p = Project::new("P", stamp(), members(&["/r/a", "/r/b"]));
    let q = Project::new("Q", stamp(), members(&["/r/c"]));
    let (p_id, q_id) = (p.id, q.id);
    let mut projects = vec![p, q];
    let before = rosters(&projects);
    assert!(reorder_drop(&mut projects, &reach, &picked(&["/r/a"]), "/r/c", false));
    let after = rosters(&projects);
    regroup(&mut projects, &after, &before);
    delete(&mut projects, q_id);
    regroup(&mut projects, &before, &after);
    assert_eq!(projects.iter().find(|p| p.id == p_id).unwrap().members, members(&["/r/a", "/r/b"]), "a stays in P");
    // Twin: P's end laid alone takes a out of every project.
    assert!(!after[&p_id].members.contains(&of("/r/a")));
}

#[test]
fn a_project_dragged_among_its_siblings_takes_an_order_between_the_target_and_the_one_above() {
    let old = Project::new("Old", Date::since_reference_date(0.0), vec![]);
    let mid = Project::new("Mid", Date::since_reference_date(100.0), vec![]);
    let new = Project::new("New", Date::since_reference_date(200.0), vec![]);
    let (old_id, mid_id, new_id) = (old.id, mid.id, new.id);
    let mut projects = vec![old, mid, new];
    let order = |p: &[Project]| -> Vec<String> { grouped(p, &Reach::rows(&[])).into_iter().map(|g| g.project.name).collect() };
    assert_eq!(order(&projects), ["New", "Mid", "Old"]);
    assert!(reorder_root(&mut projects, &HashSet::from([old_id]), mid_id));
    assert_eq!(order(&projects), ["New", "Old", "Mid"], "dropped before Mid, it shows above it");
    assert_eq!(projects[0].manual_order, Some(150.0));
    // Before the head, a day above it frames the step.
    assert!(reorder_root(&mut projects, &HashSet::from([mid_id]), new_id));
    assert_eq!(order(&projects), ["Mid", "New", "Old"]);
    assert_eq!(projects[1].manual_order, Some(200.0 + 86_400.0 / 2.0));
    // Twin: the target alone dragged onto itself is refused.
    assert!(!reorder_root(&mut projects, &HashSet::from([new_id]), new_id));
}
