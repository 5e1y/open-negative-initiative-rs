use super::*;
use crate::scratch::Scratch;

fn stamp() -> Date {
    Date::since_reference_date(8e8)
}

fn row(path: &str, fingerprint: &str) -> Entry {
    let mut e = Entry::new(path, stamp(), fingerprint);
    e.captured_at = Some(stamp());
    e
}

fn members(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn picked(paths: &[&str]) -> HashSet<String> {
    paths.iter().map(|s| s.to_string()).collect()
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

#[test]
fn a_file_round_trips_through_the_disk_in_the_swift_spelling() {
    let scratch = Scratch::new("projects");
    let mut iceland = Project::new("Iceland", stamp(), members(&["/r/a.NEF", "/r/b.NEF"]));
    iceland.description = "Two weeks, June".into();
    let mut portraits = Project::new("Portraits", stamp(), vec![]);
    portraits.manual_order = Some(3.0);
    let file = ProjectsFile { version: 1, projects: vec![iceland, portraits] };
    save(scratch.path(), &file).unwrap();
    assert_eq!(load(scratch.path()), file);
    let text = std::fs::read_to_string(file_path(scratch.path())).unwrap();
    assert!(text.contains("\"createdAt\" : 800000000"), "{text}");
    assert!(text.contains("\"manualOrder\" : 3"));
    // Twin: the sidecar's ISO spelling of the same date is not what a project writes.
    assert!(!text.contains(&stamp().iso8601()));
}

/// A former file held prints, each with the path it was last seen at: each member reads as that
/// path, and a Swift file's print with none stays a print for the library to find.
#[test]
fn a_former_file_reads_each_member_at_its_recorded_path() {
    let scratch = Scratch::new("projects-former");
    let id = Id::new();
    let former = format!(
        r#"{{"version":1,"projects":[{{"id":"{id}","name":"Trip","description":"","createdAt":8e8,"members":["1-no-date-a","1-no-date-b","1-no-date-c"],"paths":{{"1-no-date-a":"/r8/a.NEF","1-no-date-c":"/r10/a.NEF"}}}}]}}"#
    );
    std::fs::write(file_path(scratch.path()), former).unwrap();
    let read = load(scratch.path()).projects;
    assert_eq!(read[0].members, ["/r8/a.NEF", "1-no-date-b", "/r10/a.NEF"]);
    assert!(!is_print(&read[0].members[0]) && is_print(&read[0].members[1]));
    // The written file names paths alone: the former map is never written back.
    save(scratch.path(), &ProjectsFile { version: 1, projects: read.clone() }).unwrap();
    let text = std::fs::read_to_string(file_path(scratch.path())).unwrap();
    assert!(!text.contains("\"paths\""), "{text}");
    assert_eq!(load(scratch.path()).projects, read);
    // Twin: read as the prints it held, the two copies' rows would stand for one photo.
    assert_ne!(read[0].members[0], read[0].members[2]);
}

/// Two rolls holding the same file under the same name: a project keeps both, each by its path.
#[test]
fn two_copies_of_one_file_are_two_members_each_found_at_its_path() {
    let library = [row("/r10/a.NEF", "print-a"), row("/r8/a.NEF", "print-a"), row("/r8/b.NEF", "print-b")];
    let mut projects = vec![Project::new("Client", stamp(), members(&["/r10/a.NEF"]))];
    let id = projects[0].id;
    let done = add_to(&mut projects, &library, &picked(&["/r8/a.NEF", "/r8/b.NEF"]), id);
    assert_eq!(done, Joined { added: 2, already: 0 }, "the copy of a member joins it");
    let listed: Vec<String> = grouped(&projects, &library)[0].entries.iter().map(|e| e.path.clone()).collect();
    assert_eq!(listed, ["/r10/a.NEF", "/r8/a.NEF", "/r8/b.NEF"]);
    // Adding them again changes nothing and says each was there.
    let before = projects.clone();
    assert_eq!(add_to(&mut projects, &library, &picked(&["/r8/a.NEF", "/r10/a.NEF"]), id), Joined { added: 0, already: 2 });
    assert_eq!(projects, before);
    // Twin: keyed by print, as the former members were, the copy counts as there and b joins alone.
    let held: HashSet<&str> = library.iter().filter(|e| e.path == "/r10/a.NEF").map(|e| e.fingerprint.as_str()).collect();
    let by_print = library.iter().filter(|e| e.path.starts_with("/r8/") && !held.contains(e.fingerprint.as_str())).count();
    assert_ne!(by_print, done.added);
}

#[test]
fn projects_order_newest_first_and_follow_a_move_made_in_oni() {
    let older = Project::new("Older", Date::since_reference_date(0.0), vec![]);
    let newer = Project::new("Newer", Date::since_reference_date(100.0), vec![]);
    let mut pinned = Project::new("Pinned", Date::since_reference_date(0.0), vec![]);
    pinned.manual_order = Some(200.0);
    let order: Vec<String> = grouped(&[older, newer, pinned], &[]).into_iter().map(|g| g.project.name).collect();
    assert_eq!(order, ["Pinned", "Newer", "Older"]);

    let library = [row("/negs/a.RW2", "fp-a"), row("/negs/b.RW2", "fp-b")];
    let gap = Project::new("Gap", stamp(), members(&["/negs/b.RW2", "/negs/gone.RW2", "/negs/a.RW2"]));
    let resolved: Vec<String> = grouped(std::slice::from_ref(&gap), &library)[0].entries.iter().map(|e| e.path.clone()).collect();
    assert_eq!(resolved, ["/negs/b.RW2", "/negs/a.RW2"]);
    assert_eq!(gap.members.len(), 3, "a member no row holds is never pruned by reading");

    let trip = Project::new("Trip", stamp(), members(&["/r/a.RW2", "/s/b.RW2"]));
    let carried = moved(std::slice::from_ref(&trip), &[("/r".into(), "/t/r".into())]);
    assert_eq!(carried[0].members, ["/t/r/a.RW2", "/s/b.RW2"]);
    let file = moved(std::slice::from_ref(&trip), &[("/s/b.RW2".into(), "/s/B.RW2".into())]);
    assert_eq!(file[0].members, ["/r/a.RW2", "/s/B.RW2"], "a file moved alone is carried too");
    // Twin: a prefix that is not a folder moves nothing.
    let sibling = moved(std::slice::from_ref(&trip), &[("/r/a".into(), "/x".into())]);
    assert_eq!(sibling[0].members, trip.members);
}

#[test]
fn a_shared_member_stays_with_the_newest_project() {
    let first = Project::new("First", Date::since_reference_date(0.0), members(&["/r/a", "/r/shared"]));
    let second = Project::new("Second", Date::since_reference_date(100.0), members(&["/r/shared", "/r/b"]));
    let settled: Vec<Vec<String>> = exclusive(&[first.clone(), second.clone()]).into_iter().map(|p| p.members).collect();
    assert_eq!(settled, [members(&["/r/a"]), members(&["/r/shared", "/r/b"])]);
    // Twin: listed the other way round, creation still decides, not position.
    let swapped: Vec<Vec<String>> = exclusive(&[second, first]).into_iter().map(|p| p.members).collect();
    assert_eq!(swapped, [members(&["/r/shared", "/r/b"]), members(&["/r/a"])]);
}

#[test]
fn a_new_project_takes_the_picked_photos_in_library_order_under_the_lowest_free_name() {
    let library = [row("/r/a", "fa"), row("/r/b", "fb"), row("/r/c", "fc")];
    let mut projects = vec![Project::new("Project 1", stamp(), members(&["/r/z"]))];
    let id = new_from(&mut projects, &library, &picked(&["/r/c", "/r/a"]), stamp());
    let made = projects.iter().find(|p| p.id == id).unwrap().clone();
    assert_eq!((made.name.as_str(), made.members.clone()), ("Project 2", members(&["/r/a", "/r/c"])));
    // Twin: the picked set's own order would put c first.
    assert_ne!(made.members, members(&["/r/c", "/r/a"]));
    // Nothing picked makes an empty project, named, for Add to Project to fill.
    let empty = new_from(&mut projects, &library, &HashSet::new(), stamp());
    let empty = projects.iter().find(|p| p.id == empty).unwrap();
    assert!(empty.members.is_empty() && empty.name == "Project 3");
}

/// "Add to Project" takes a photo out of the project it sat in, as a photo sits in one: the ask
/// names that project first, and an emptied project stays, never dropped without a word.
#[test]
fn adding_to_a_project_never_drops_another_silently() {
    let library = [row("/r/a", "fa"), row("/r/b", "fb"), row("/r/c", "fc")];
    let first = Project::new("First", stamp(), members(&["/r/a"]));
    let second = Project::new("Second", stamp(), members(&["/r/b", "/r/c"]));
    let target = Project::new("Target", stamp(), members(&["/r/z"]));
    let (first_id, second_id, target_id) = (first.id, second.id, target.id);
    let mut projects = vec![first, second, target];
    let asked: Vec<(&str, usize)> = leaving(&projects, &picked(&["/r/a", "/r/b"]), target_id).into_iter().map(|(p, n)| (p.name.as_str(), n)).collect();
    assert_eq!(asked, [("First", 1), ("Second", 1)], "the ask names each project a photo leaves");
    assert!(leaving(&projects, &picked(&["/r/c"]), second_id).is_empty(), "twin: photos already there leave nothing");
    let before = rosters(&projects);
    assert_eq!(add_to(&mut projects, &library, &picked(&["/r/a", "/r/b"]), target_id), Joined { added: 2, already: 0 });
    let first = projects.iter().find(|p| p.id == first_id).expect("First stays, empty, with its name");
    assert!(first.members.is_empty() && first.name == "First");
    assert_eq!(projects.iter().find(|p| p.id == second_id).unwrap().members, members(&["/r/c"]));
    assert_eq!(projects.iter().find(|p| p.id == target_id).unwrap().members, members(&["/r/z", "/r/a", "/r/b"]));
    // Twin: the rule it replaces, emptied projects dropped, loses First and its name.
    let mut dropped = projects.clone();
    dropped.retain(|p| !p.members.is_empty());
    assert!(!dropped.iter().any(|p| p.id == first_id));
    // The rosters held before lay every project back as it was, the Undo key's step.
    let mut undone = projects.clone();
    regroup(&mut undone, &before);
    assert_eq!(rosters(&undone), before);
    assert_eq!(remove_from(&mut projects, &picked(&["/r/a", "/r/c"]), target_id), 1, "only the member there is counted");
}

#[test]
fn a_drop_lands_before_its_target_members_first_then_the_joining_ones() {
    let library = [row("/r/a", "fa"), row("/r/b", "fb"), row("/r/c", "fc"), row("/r/d", "fd"), row("/r/x", "fx")];
    let trip = Project::new("Trip", stamp(), members(&["/r/a", "/r/b", "/r/c", "/r/d"]));
    let other = Project::new("Other", stamp(), members(&["/r/x"]));
    let trip_id = trip.id;
    let mut projects = vec![other, trip];
    // d and x dragged before b: d already a member moves first, x joins after it and leaves Other.
    assert!(reorder_drop(&mut projects, &library, &picked(&["/r/x", "/r/d"]), "/r/b"));
    assert!(projects[0].name == "Other" && projects[0].members.is_empty(), "Other held only x and stays, empty");
    assert_eq!(projects[1].id, trip_id);
    assert_eq!(projects[1].members, members(&["/r/a", "/r/d", "/r/x", "/r/b", "/r/c"]));
    // Twin: the target dragged onto itself moves nothing.
    assert!(!reorder_drop(&mut projects, &library, &picked(&["/r/b"]), "/r/b"));
    // A drop on a photo in no project lands nowhere.
    assert!(!reorder_drop(&mut projects, &library, &picked(&["/r/a"]), "/r/loose"));
}

#[test]
fn a_project_dragged_among_its_siblings_takes_an_order_between_the_target_and_the_one_above() {
    let old = Project::new("Old", Date::since_reference_date(0.0), vec![]);
    let mid = Project::new("Mid", Date::since_reference_date(100.0), vec![]);
    let new = Project::new("New", Date::since_reference_date(200.0), vec![]);
    let (old_id, mid_id, new_id) = (old.id, mid.id, new.id);
    let mut projects = vec![old, mid, new];
    let order = |p: &[Project]| -> Vec<String> { grouped(p, &[]).into_iter().map(|g| g.project.name).collect() };
    assert_eq!(order(&projects), ["New", "Mid", "Old"]);
    assert!(reorder_root(&mut projects, &[], &HashSet::from([old_id]), mid_id));
    assert_eq!(order(&projects), ["New", "Old", "Mid"], "dropped before Mid, it shows above it");
    assert_eq!(projects[0].manual_order, Some(150.0));
    // Before the head, a day above it frames the step.
    assert!(reorder_root(&mut projects, &[], &HashSet::from([mid_id]), new_id));
    assert_eq!(order(&projects), ["Mid", "New", "Old"]);
    assert_eq!(projects[1].manual_order, Some(200.0 + 86_400.0 / 2.0));
    // Twin: the target alone dragged onto itself is refused.
    assert!(!reorder_root(&mut projects, &[], &HashSet::from([new_id]), new_id));
}
