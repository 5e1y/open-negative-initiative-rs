//! A drop in a project lands where its line shows, in the project's own order whatever roll each
//! photo lies in, and writes that order back; none leaves its project unbidden.

use super::*;
use crate::index::Entry;
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

/// The project's photos as it shows them.
fn shown(projects: &[Project], id: Id, library: &[Entry]) -> Vec<String> {
    let group = grouped(projects, &Reach::rows(library)).into_iter().find(|g| g.id == id).unwrap();
    group.entries.into_iter().map(|e| e.path).collect()
}

const R10: [&str; 3] = ["/s/Roll 10/A", "/s/Roll 10/C", "/s/Roll 10/D"];
const R8: [&str; 2] = ["/s/Roll 8/A", "/s/Roll 8/B"];

/// Stored A C, a member no row shows, Roll 8's A B, then D, shown so. A dragged past D lands last,
/// the order shown written back, the unseen member kept at its index.
#[test]
fn a_drop_lands_where_its_line_shows_and_writes_the_order_shown() {
    let library = rows(&[R10[0], R10[1], R10[2], R8[0], R8[1], "/s/Roll 10/X"]);
    let unseen = Member::new(None, Some("1-unseen"));
    let client = Project::new("Client", Date::since_reference_date(8e8), vec![of(R10[0]), of(R10[1]), unseen.clone(), of(R8[0]), of(R8[1]), of(R10[2])]);
    let other = Project::new("Other", Date::since_reference_date(9e8), vec![of("/s/Roll 10/X")]);
    let (id, other_id) = (client.id, other.id);
    let mut projects = vec![client, other];
    assert_eq!(shown(&projects, id, &library), [R10[0], R10[1], R8[0], R8[1], R10[2]], "the stored order, rolls interleaved");
    assert!(reorder_drop(&mut projects, &Reach::rows(&library), &picked(&[R10[0]]), R10[2], true));
    assert_eq!(shown(&projects, id, &library), [R10[1], R8[0], R8[1], R10[2], R10[0]]);
    assert_eq!(projects[0].members, [of(R10[1]), of(R8[0]), unseen, of(R8[1]), of(R10[2]), of(R10[0])]);
    // A photo of Roll 8 lands beside one of Roll 10: no roll keeps a photo apart.
    assert!(reorder_drop(&mut projects, &Reach::rows(&library), &picked(&[R8[1]]), R10[1], false));
    assert_eq!(shown(&projects, id, &library), [R8[1], R10[1], R8[0], R10[2], R10[0]]);
    // Twin: dropped where it already stands, nothing changes.
    let before = projects.clone();
    assert!(!reorder_drop(&mut projects, &Reach::rows(&library), &picked(&[R8[1]]), R10[1], false));
    assert_eq!(projects, before);
    assert!(reorder_drop(&mut projects, &Reach::rows(&library), &picked(&["/s/Roll 10/X"]), R10[0], true));
    assert_eq!(shown(&projects, id, &library).last().map(String::as_str), Some("/s/Roll 10/X"));
    assert!(projects.iter().find(|p| p.id == other_id).unwrap().members.is_empty(), "X left Other, as the line was Client's");
}
