//! A drop in a project shown roll by roll lands where its line shows, a roll's end included, and
//! writes that order back; a photo of another roll lands nowhere, and none leaves its project unbidden.

use super::*;
use crate::index::Entry;
use crate::project_tree::by_roll;
use std::collections::HashSet;

fn of(path: &str) -> Member {
    Member::new(Some(path), Some(&format!("f{path}")))
}

fn rows(paths: &[&str]) -> Vec<Entry> {
    paths.iter().map(|p| Entry::new(p, Date::since_reference_date(8e8), &format!("f{p}"))).collect()
}

fn picked(paths: &[&str]) -> HashSet<String> {
    paths.iter().map(|s| s.to_string()).collect()
}

/// Each roll's photos as the project shows them.
fn shown(projects: &[Project], id: Id, library: &[Entry]) -> Vec<Vec<String>> {
    let group = grouped(projects, &Reach::rows(library)).into_iter().find(|g| g.id == id).unwrap();
    by_roll(&group.entries).into_iter().map(|(_, photos)| photos.into_iter().map(|e| e.path).collect()).collect()
}

const R10: [&str; 3] = ["/s/Roll 10/A", "/s/Roll 10/C", "/s/Roll 10/D"];
const R8: [&str; 2] = ["/s/Roll 8/A", "/s/Roll 8/B"];

/// Stored A C, a member no row shows, Roll 8's A B, then D: Roll 10 shows A C D. A dragged past D
/// ends Roll 10, the order shown written back, the unseen member kept in its place.
#[test]
fn a_drop_past_a_roll_s_last_photo_ends_that_roll_and_writes_the_order_shown() {
    let library = rows(&[R10[0], R10[1], R10[2], R8[0], R8[1], "/s/Roll 10/X"]);
    let unseen = Member::new(None, Some("1-unseen"));
    let client = Project::new("Client", Date::since_reference_date(8e8), vec![of(R10[0]), of(R10[1]), unseen.clone(), of(R8[0]), of(R8[1]), of(R10[2])]);
    let other = Project::new("Other", Date::since_reference_date(9e8), vec![of("/s/Roll 10/X")]);
    let (id, other_id) = (client.id, other.id);
    let mut projects = vec![client, other];
    let stored = projects[0].members.clone();
    assert_eq!(shown(&projects, id, &library), [R10.to_vec(), R8.to_vec()]);
    assert!(reorder_drop(&mut projects, &Reach::rows(&library), &picked(&[R10[0]]), R10[2], true));
    assert_eq!(shown(&projects, id, &library), [vec![R10[1], R10[2], R10[0]], R8.to_vec()], "Roll 10 reads C D A");
    assert_eq!(projects[0].members, [of(R10[1]), of(R10[2]), unseen, of(R10[0]), of(R8[0]), of(R8[1])]);
    // Twin: spliced in the stored order before Roll 8's first photo, A stays inside Roll 10.
    let mut spliced = stored.clone();
    let a = spliced.remove(0);
    let before = spliced.iter().position(|m| *m == of(R8[0])).unwrap();
    spliced.insert(before, a);
    let respliced = [Project { members: spliced, ..projects[0].clone() }, projects[1].clone()];
    assert_eq!(shown(&respliced, id, &library)[0], [R10[1], R10[0], R10[2]]);

    // A photo of Roll 8 beside Roll 10's lands nowhere; one of Roll 10 joins where the line is.
    let before = projects.clone();
    assert!(!reorder_drop(&mut projects, &Reach::rows(&library), &picked(&[R8[1]]), R10[1], false));
    assert!(!lands_beside(&picked(&[R8[1], R10[0]]), R10[1]), "nor a pick spanning two rolls");
    assert_eq!(projects, before);
    assert!(reorder_drop(&mut projects, &Reach::rows(&library), &picked(&["/s/Roll 10/X"]), R10[0], true));
    assert_eq!(shown(&projects, id, &library), [vec![R10[1], R10[2], R10[0], "/s/Roll 10/X"], R8.to_vec()]);
    assert!(projects.iter().find(|p| p.id == other_id).unwrap().members.is_empty(), "X left Other, as the line was Client's");
    // Twin: the same photo beside Roll 8's last stays out, a roll it does not lie in.
    assert!(lands_beside(&picked(&[R8[0]]), R8[1]));
    assert!(!lands_beside(&picked(&["/s/Roll 10/X"]), R8[1]));
}
