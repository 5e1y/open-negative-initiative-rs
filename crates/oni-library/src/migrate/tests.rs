use super::*;
use crate::project::Member;
use crate::scratch::Scratch;
use std::time::{Duration, Instant, SystemTime};

/// `favourites.json` as the old app's `JSONEncoder` wrote it: compact, slashes escaped.
fn favourites_file(support: &Path, paths: &[&str]) -> Vec<u8> {
    let quoted: Vec<String> = paths.iter().map(|p| format!("\"{}\"", p.replace('/', "\\/"))).collect();
    let bytes = format!("{{\"version\":1,\"paths\":[{}]}}", quoted.join(",")).into_bytes();
    std::fs::write(support.join("favourites.json"), &bytes).unwrap();
    bytes
}

/// `bookmarks.json` as builds 1.4 to 1.5.6 wrote it: indented, by path.
fn bookmarks_file(support: &Path) -> Vec<u8> {
    let bytes = br#"{
  "version": 1,
  "places": {
    "/Users/a/Rolls": {
      "fileId": 81
    },
    "/Volumes/T7/Scans": {
      "volume": {
        "uuid": "6C1E0D7A-1F1B-4C55-9A3E-2B0F3C7D1E11",
        "mount": "/Volumes/T7"
      },
      "fileId": 2
    }
  }
}"#
    .to_vec();
    std::fs::write(support.join("bookmarks.json"), &bytes).unwrap();
    bytes
}

fn modified(path: &Path) -> SystemTime {
    std::fs::metadata(path).unwrap().modified().unwrap()
}

/// `Favourites` as 1.5.6 shipped it (eac39604): its two fields, both required.
#[derive(Deserialize)]
#[allow(dead_code)]
struct Shipped {
    version: i64,
    paths: Vec<String>,
}

#[test]
fn places_import_keeps_the_favourites_order_and_lays_each_place_on_its_path() {
    let scratch = Scratch::new("migrate-places");
    let support = scratch.path();
    let order = ["/Volumes/T7/Scans", "/Users/a/Archive", "/Users/a/Rolls"];
    let favourites = favourites_file(support, &order);
    let bookmarks = bookmarks_file(support);
    assert!(places(support).unwrap());
    let read = Places::load(support).unwrap();
    assert_eq!(read.paths(), order, "the favourites' order");
    let t7 = Volume { uuid: Some("6C1E0D7A-1F1B-4C55-9A3E-2B0F3C7D1E11".into()), mount: "/Volumes/T7".into() };
    assert_eq!(read.places[0], Place { path: order[0].into(), volume: Some(t7), file_id: Some(2) });
    assert_eq!(read.places[1], Place::at(order[1]), "a favourite with no place kept");
    assert_eq!(read.places[2].file_id, Some(81));
    assert_eq!(std::fs::read(support.join("favourites.json")).unwrap(), favourites, "left as it was");
    assert_eq!(std::fs::read(support.join("bookmarks.json")).unwrap(), bookmarks, "left as it was");
    let shipped: Shipped = serde_json::from_slice(&std::fs::read(support.join("favourites.json")).unwrap()).expect("1.5.6 still reads it");
    assert_eq!(shipped.paths, order);
    // Twin: an import that sorts the paths fails the order assertion.
    let mut sorted = order.to_vec();
    sorted.sort();
    assert_ne!(sorted, read.paths());
    // Twin: an import that had rewritten `favourites.json` in the new shape fails both checks above.
    read.save(support).unwrap();
    std::fs::copy(Places::file_path(support), support.join("favourites.json")).unwrap();
    assert_ne!(std::fs::read(support.join("favourites.json")).unwrap(), favourites);
    assert!(serde_json::from_slice::<Shipped>(&std::fs::read(support.join("favourites.json")).unwrap()).is_err());
}

#[test]
fn a_second_import_writes_nothing_and_keeps_a_star_added_since() {
    let scratch = Scratch::new("migrate-again");
    let support = scratch.path();
    favourites_file(support, &["/Volumes/T7/Scans"]);
    std::fs::write(support.join("projects.json"), br#"{"version":1,"projects":[]}"#).unwrap();
    assert!(places(support).unwrap() && projects(support).unwrap());
    let mut held = Places::load(support).unwrap();
    held.toggle("/Users/a/Rolls");
    held.save(support).unwrap();
    let stamps = (modified(&Places::file_path(support)), modified(&project::file_path(support)));
    std::thread::sleep(Duration::from_millis(20));
    assert!(!places(support).unwrap() && !projects(support).unwrap(), "nothing imported twice");
    assert_eq!((modified(&Places::file_path(support)), modified(&project::file_path(support))), stamps);
    assert_eq!(Places::load(support).unwrap().paths(), ["/Volumes/T7/Scans", "/Users/a/Rolls"]);
    // Twin: without the test of the new file, the second run writes over the star added since.
    assert!(places_imported(support).unwrap());
    assert_eq!(Places::load(support).unwrap().paths(), ["/Volumes/T7/Scans"]);
}

#[test]
fn nothing_is_imported_where_no_favourite_was_ever_kept() {
    let scratch = Scratch::new("migrate-none");
    assert!(!places(scratch.path()).unwrap() && !projects(scratch.path()).unwrap());
    assert!(!crate::state::folder(scratch.path()).exists(), "the roots seed the favourites, as at a first launch");
}

/// One file each of the shapes builds wrote: version 1's texts (1.5.6's placeholder among them),
/// version 2's objects, version 4's `exactMembers` beside the texts.
#[test]
fn projects_import_each_former_format_to_the_members_its_reader_gave() {
    let waiting = format!("{}/r/w.NEF", crate::library::WAITING_MARK);
    let id = oni_settings::Id::new().to_string();
    let project = |members: serde_json::Value, extra: serde_json::Value| {
        let mut p = serde_json::json!({"id": id, "name": "Trip", "description": "", "createdAt": 8.0e8 + 0.25, "members": members});
        p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        p
    };
    let exact = serde_json::json!({
        "paths": {"1-a": "/r/a.NEF", "/r8/a.NEF": "/r8/a.NEF"},
        "exactMembers": [{"path": "/r/a.NEF", "print": "1-a"}, {"path": "/r8/a.NEF", "print": "1-a"}]
    });
    let cases = [
        (1, project(serde_json::json!(["1-a", "/r/b.NEF", waiting]), serde_json::json!({"paths": {"1-a": "/r/a.NEF", (waiting.clone()): "/r/w.NEF"}})),
            vec![Member::new(Some("/r/a.NEF"), Some("1-a")), Member::new(Some("/r/b.NEF"), None), Member::new(Some("/r/w.NEF"), Some(&waiting))]),
        (2, project(serde_json::json!([{"path": "/r/a.NEF", "print": "1-a"}, {"print": "1-s"}]), serde_json::json!({})),
            vec![Member::new(Some("/r/a.NEF"), Some("1-a")), Member::new(None, Some("1-s"))]),
        (4, project(serde_json::json!(["1-a", "/r8/a.NEF"]), exact.clone()),
            vec![Member::new(Some("/r/a.NEF"), Some("1-a")), Member::new(Some("/r8/a.NEF"), Some("1-a"))]),
    ];
    for (version, project, members) in cases {
        let scratch = Scratch::new("migrate-projects");
        let support = scratch.path();
        let text = serde_json::to_vec(&serde_json::json!({"version": version, "projects": [project.clone()]})).unwrap();
        std::fs::write(support.join("projects.json"), &text).unwrap();
        assert!(projects(support).unwrap(), "format {version}");
        let read = project::load(support).projects;
        assert_eq!(read[0].members, members, "format {version}");
        assert_eq!(read[0].created_at, Date::since_reference_date(8.0e8), "whole seconds, as the file keeps them");
        assert_eq!(std::fs::read(support.join("projects.json")).unwrap(), text, "format {version} left as it was");
    }
    // Twin: an import reading `members` and ignoring `exactMembers` loses the copy's print.
    let blind = serde_json::json!({"version": 4, "projects": [project(serde_json::json!(["1-a", "/r8/a.NEF"]), serde_json::json!({"paths": exact["paths"]}))]});
    let read = projects::read(&serde_json::to_vec(&blind).unwrap()).unwrap();
    assert_eq!(read[0].members[1], Member::new(Some("/r8/a.NEF"), None));
}

#[test]
fn a_newer_places_file_is_shown_and_left_as_it_is() {
    let scratch = Scratch::new("migrate-newer");
    let support = scratch.path();
    let path = Places::file_path(support);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let newer = br#"{"version": 2, "places": [{"path": "/Volumes/T7/Scans", "colour": "red"}]}"#;
    std::fs::write(&path, newer).unwrap();
    assert!(matches!(state::read::<Places>(&path, crate::places::VERSION), state::Read::Newer(2, Some(_))));
    let mut held = Places::load(support).unwrap();
    assert_eq!(held.paths(), ["/Volumes/T7/Scans"], "shown as far as this build reads it");
    held.toggle("/Users/a/Rolls");
    assert!(held.save(support).is_err(), "the star is refused");
    assert_eq!(std::fs::read(&path).unwrap(), newer, "untouched");
    // Twin: a reader that sets a newer file aside leaves nothing at its name.
    crate::set_aside(&path);
    assert!(std::fs::read(&path).is_err());
}

/// A read that waits for another app's write: a torn file read again after 100 ms before it counts.
fn read_with_retry(path: &Path) -> Option<Places> {
    let data = std::fs::read(path).ok()?;
    if let Ok(value) = serde_json::from_slice(&data) {
        return Some(value);
    }
    std::thread::sleep(Duration::from_millis(100));
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

#[test]
fn an_unreadable_places_file_is_set_aside_at_the_first_read_with_no_wait() {
    let scratch = Scratch::new("migrate-unreadable");
    let support = scratch.path();
    let path = Places::file_path(support);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let torn = br#"{"version": 1, "places": [{"path": "/Volumes/T7/Sc"#;
    std::fs::write(&path, torn).unwrap();
    let began = Instant::now();
    assert_eq!(Places::load(support), None);
    assert!(began.elapsed() < Duration::from_millis(20), "{:?}", began.elapsed());
    let aside: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with("places.json.unreadable-")).collect();
    assert_eq!((aside.len(), std::fs::read(aside[0].path()).unwrap()), (1, torn.to_vec()), "kept beside, whole");
    // Twin: the read that waits for another app's write fails the time bound.
    std::fs::write(&path, torn).unwrap();
    let began = Instant::now();
    assert_eq!(read_with_retry(&path), None);
    assert!(began.elapsed() >= Duration::from_millis(20));
}

#[test]
fn a_favourite_starred_is_there_at_the_next_launch() {
    let scratch = Scratch::new("migrate-starred");
    let support = scratch.path();
    let mut held = Places::load(support).unwrap_or_default();
    held.toggle("/Volumes/T7/Scans");
    held.save(support).unwrap();
    assert!(Places::load(support).unwrap().contains("/Volumes/T7/Scans"), "read again as written");
    // Twin: a save that skips the write leaves the star out of the next launch.
    let mut skipped = Places::load(support).unwrap();
    skipped.toggle("/Users/a/Rolls");
    assert!(!Places::load(support).unwrap().contains("/Users/a/Rolls"));
}
