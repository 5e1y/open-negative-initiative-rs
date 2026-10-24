use super::*;
use crate::favourites::Favourites;
use crate::scratch::Scratch;
use oni_settings::Date;

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// The file as the Swift app's `JSONEncoder` writes it.
fn swift_writes_favourites(support: &Path, paths: &[&str]) {
    let quoted: Vec<String> = paths.iter().map(|p| format!("\"{}\"", p.replace('/', "\\/"))).collect();
    std::fs::write(Favourites::file_path(support), format!("{{\"version\":1,\"paths\":[{}]}}", quoted.join(","))).unwrap();
}

#[test]
fn a_list_keeps_both_sides_changes() {
    let base = strings(&["/a", "/b"]);
    assert_eq!(list(&base, &strings(&["/a", "/b", "/c"]), &strings(&["/b", "/a", "/d"])), ["/b", "/a", "/d", "/c"]);
    assert_eq!(list(&base, &strings(&["/b"]), &strings(&["/a", "/b", "/d"])), ["/b", "/d"]);
    // Twin: one side unchanged gives the other's order whole, a reorder included.
    assert_eq!(list(&base, &strings(&["/b", "/a"]), &base), ["/b", "/a"]);
    assert_eq!(list(&base, &base, &strings(&["/b"])), ["/b"]);
}

#[test]
fn a_favourite_added_by_another_app_survives_our_save() {
    let scratch = Scratch::new("merge-favourites");
    let support = scratch.path();
    swift_writes_favourites(support, &["/a"]);
    let mut ours = Favourites::load(support).unwrap();
    let mut read = ours.clone();
    swift_writes_favourites(support, &["/a", "/b"]);
    ours.toggle("/c");
    ours.save_merged(&mut read, support).unwrap();
    assert_eq!(Favourites::load(support).unwrap().paths, ["/a", "/b", "/c"]);
    assert_eq!(ours.paths, ["/a", "/b", "/c"], "the app shows what it wrote");
    // A star taken off here stays off, and one taken off there stays off too.
    ours.toggle("/a");
    swift_writes_favourites(support, &["/a", "/c"]);
    ours.save_merged(&mut read, support).unwrap();
    assert_eq!(Favourites::load(support).unwrap().paths, ["/c"]);
    // A file another app made unreadable is set aside, and ours is written whole.
    std::fs::write(Favourites::file_path(support), b"{\"version\":1,\"pa").unwrap();
    ours.toggle("/d");
    ours.save_merged(&mut read, support).unwrap();
    assert_eq!(Favourites::load(support).unwrap().paths, ["/c", "/d"]);
    // Twin: the whole save from memory loses the favourite the other app added.
    swift_writes_favourites(support, &["/c", "/d", "/e"]);
    ours.save(support).unwrap();
    assert!(!Favourites::load(support).unwrap().paths.contains(&"/e".to_string()));
}

fn project(name: &str, members: &[&str]) -> Project {
    Project::new(name, Date::since_reference_date(8e8), strings(members))
}

fn names(projects: &[Project]) -> Vec<String> {
    let mut names: Vec<String> = projects.iter().map(|p| p.name.clone()).collect();
    names.sort();
    names
}

fn other_app_writes(support: &Path, projects: &[Project]) {
    project::save(support, &ProjectsFile { version: 1, projects: projects.to_vec() }).unwrap();
}

#[test]
fn a_project_added_by_another_app_survives_our_save() {
    let scratch = Scratch::new("merge-projects");
    let support = scratch.path();
    let trip = project("Trip", &["f1"]);
    other_app_writes(support, std::slice::from_ref(&trip));
    let mut ours = project::load(support).projects;
    let read = ours.clone();
    // Meanwhile the other app adds a project and a member to Trip.
    let added = project("Portraits", &["f9"]);
    let mut grown = trip.clone();
    grown.members.push("f2".into());
    other_app_writes(support, &[grown, added.clone()]);
    ours[0].name = "Iceland".into();
    let mine = project("Mine", &["f5"]);
    ours.push(mine.clone());
    let read = save_projects(support, &read, &ours).unwrap();
    let file = project::load(support).projects;
    assert_eq!(names(&file), ["Iceland", "Mine", "Portraits"]);
    assert_eq!(file.iter().find(|p| p.id == trip.id).unwrap().members, ["f1", "f2"], "both sides' changes kept");
    // A project deleted here stays deleted; one deleted there, untouched here, stays deleted too.
    let mut ours = read.clone();
    ours.retain(|p| p.id != added.id);
    other_app_writes(support, &read.iter().filter(|p| p.id != mine.id).cloned().collect::<Vec<_>>());
    save_projects(support, &read, &ours).unwrap();
    assert_eq!(names(&project::load(support).projects), ["Iceland"]);
    // Twin: the whole save from memory loses the other app's project.
    let read = project::load(support).projects;
    other_app_writes(support, &[read[0].clone(), added.clone()]);
    project::save(support, &ProjectsFile { version: 1, projects: read.clone() }).unwrap();
    assert_eq!(names(&project::load(support).projects), ["Iceland"], "the plain save drops Portraits");
}

#[test]
fn a_project_the_other_app_deleted_survives_an_edit_here() {
    let (base, mut ours) = (vec![project("Trip", &["f1"])], Vec::new());
    ours.push(base[0].clone());
    ours[0].members.push("f2".into());
    assert_eq!(projects(&base, &ours, &[]).len(), 1, "an edit here is not thrown away");
    // Twin: only the paths a save writes down are no edit, so the deletion holds.
    let mut located = base.clone();
    located[0].paths = Some([("f1".to_string(), "/r/a.NEF".to_string())].into());
    assert!(projects(&base, &located, &[]).is_empty());
}

/// Closes `at` to every reader; `false` when the check runs as root, which reads it all the same.
#[cfg(unix)]
fn closed(at: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(at, std::fs::Permissions::from_mode(0o000)).unwrap();
    std::fs::read(at).is_err()
}

#[cfg(unix)]
fn opened(at: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(at, std::fs::Permissions::from_mode(0o644)).unwrap();
}

/// A favourites file the disk refuses reads as none, as a launch meets it; the launch's seeded save
/// is refused, and the user's file comes back whole once it reads again.
#[cfg(unix)]
#[test]
fn an_unreadable_favourites_file_is_never_replaced() {
    let scratch = Scratch::new("favourites-closed");
    let support = scratch.path();
    swift_writes_favourites(support, &["/Volumes/T7/Scans", "/Users/a/Roll 8"]);
    let stars = Favourites::file_path(support);
    let kept = std::fs::read(&stars).unwrap();
    if !closed(&stars) {
        opened(&stars);
        return eprintln!("SKIPPED: root reads a closed file");
    }
    assert_eq!(Favourites::load(support), None);
    let mut seeded = Favourites::seeded(&crate::roots::Roots { version: 1, paths: strings(&["/Users/a/Old"]), active: None });
    let mut read = Favourites::default();
    let refused = seeded.save_merged(&mut read, support);
    opened(&stars);
    assert!(refused.is_err(), "the seeded list is not written over the user's");
    assert_eq!(std::fs::read(&stars).unwrap(), kept);
    // Twin: a file never written is made at the first save.
    std::fs::remove_file(&stars).unwrap();
    seeded.save_merged(&mut read, support).unwrap();
    assert_eq!(Favourites::load(support).unwrap().paths, ["/Users/a/Old"]);
}

#[cfg(unix)]
#[test]
fn an_unreadable_projects_file_is_never_replaced() {
    let scratch = Scratch::new("projects-closed");
    let support = scratch.path();
    other_app_writes(support, &[project("Iceland", &["/r/a.NEF"])]);
    let book = project::file_path(support);
    let kept = std::fs::read(&book).unwrap();
    if !closed(&book) {
        opened(&book);
        return eprintln!("SKIPPED: root reads a closed file");
    }
    assert!(project::load(support).projects.is_empty(), "a refused file reads as no project");
    let refused = save_projects(support, &[], &[project("Portraits", &["/r/b.NEF"])]);
    opened(&book);
    assert!(refused.is_err(), "the first gesture's save does not write over every project");
    assert_eq!(std::fs::read(&book).unwrap(), kept);
    assert_eq!(names(&project::load(support).projects), ["Iceland"]);
    // Twin: with no file yet, the same save makes it.
    std::fs::remove_file(&book).unwrap();
    save_projects(support, &[], &[project("Portraits", &["/r/b.NEF"])]).unwrap();
    assert_eq!(names(&project::load(support).projects), ["Portraits"]);
}
