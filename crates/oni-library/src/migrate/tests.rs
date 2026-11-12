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
    assert_eq!(places(support).unwrap(), Imported::Brought);
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
    assert_eq!((places(support).unwrap(), projects(support).unwrap()), (Imported::Brought, Imported::Brought));
    let mut held = Places::load(support).unwrap();
    held.toggle("/Users/a/Rolls");
    held.save(support).unwrap();
    let stamps = (modified(&Places::file_path(support)), modified(&project::file_path(support)));
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!((places(support).unwrap(), projects(support).unwrap()), (Imported::Nothing, Imported::Nothing), "nothing imported twice");
    assert_eq!((modified(&Places::file_path(support)), modified(&project::file_path(support))), stamps);
    assert_eq!(Places::load(support).unwrap().paths(), ["/Volumes/T7/Scans", "/Users/a/Rolls"]);
    // Twin: without the test of the new file, the second run writes over the star added since.
    former_places(support).unwrap().read().unwrap().save(support).unwrap();
    assert_eq!(Places::load(support).unwrap().paths(), ["/Volumes/T7/Scans"]);
}

#[test]
fn nothing_is_imported_where_no_favourite_was_ever_kept() {
    let scratch = Scratch::new("migrate-none");
    assert_eq!((places(scratch.path()).unwrap(), projects(scratch.path()).unwrap()), (Imported::Nothing, Imported::Nothing));
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
        assert_eq!(projects(support).unwrap(), Imported::Brought, "format {version}");
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
    // A newer shape this build cannot parse is read as nothing and left too.
    let reshaped = br#"{"version": 2, "places": {"/Volumes/T7/Scans": {}}}"#;
    std::fs::write(&path, reshaped).unwrap();
    assert_eq!(state::read::<Places>(&path, crate::places::VERSION), state::Read::Newer(2, None));
    assert_eq!(std::fs::read(&path).unwrap(), reshaped, "untouched");
    // Twin: a reader blind to the version sets the newer file aside, leaving nothing at its name.
    assert_eq!(state::read::<Places>(&path, i64::MAX), state::Read::Unreadable);
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

/// A star toggled, saved by `save`, then the places read as the next launch reads them.
fn starred_survives(save: impl Fn(&Places, &Path) -> std::io::Result<()>) -> bool {
    let scratch = Scratch::new("migrate-starred");
    let support = scratch.path();
    let mut held = Places::load(support).unwrap_or_default();
    held.toggle("/Volumes/T7/Scans");
    save(&held, support).unwrap();
    Places::load(support).is_some_and(|read| read.contains("/Volumes/T7/Scans"))
}

#[test]
fn a_favourite_starred_is_there_at_the_next_launch() {
    assert!(starred_survives(|places, support| places.save(support)), "read again as written");
    // Twin: a save that skips the write leaves the star out of the next launch.
    assert!(!starred_survives(|_, _| Ok(())));
}

#[test]
fn an_import_that_does_not_read_back_is_taken_away_and_due_again() {
    let scratch = Scratch::new("migrate-verified");
    let support = scratch.path();
    favourites_file(support, &["/Volumes/T7/Scans"]);
    let built = former_places(support).unwrap().read().unwrap();
    built.save(support).unwrap();
    let path = Places::file_path(support);
    let other = Places { places: vec![Place::at("/Users/a/Rolls")], ..built.clone() };
    assert!(verified(&path, state::Read::Current(other), built.clone()).is_err());
    assert!(!path.exists(), "the file that differs is taken away");
    assert_eq!(places(support).unwrap(), Imported::Brought, "so the next launch imports again");
    // Twin: a file that reads back as built stays, and the next launch finds nothing to import.
    assert_eq!(verified(&path, state::read(&path, crate::places::VERSION), built).unwrap(), Imported::Brought);
    assert!(path.exists());
    assert_eq!(places(support).unwrap(), Imported::Nothing);
}

#[test]
fn an_old_file_the_disk_refuses_keeps_its_import_due_and_nothing_is_written_over_it() {
    let scratch = Scratch::new("migrate-refused");
    let support = scratch.path();
    std::fs::write(support.join("roots.json"), br#"{"version":1,"paths":["/Users/a/Roots"]}"#).unwrap();
    // A folder at the file's name: the disk refuses to read it, as a file a permission withholds.
    std::fs::create_dir_all(support.join("favourites.json")).unwrap();
    std::fs::create_dir_all(support.join("projects.json")).unwrap();
    assert!(places(support).is_err() && projects(support).is_err(), "an error, not nothing to import");
    assert_eq!(places_held(support).paths(), ["/Users/a/Roots"], "shown from the roots meanwhile");
    assert!(!crate::state::folder(support).exists(), "and written nowhere");
    std::fs::remove_dir(support.join("favourites.json")).unwrap();
    std::fs::remove_dir(support.join("projects.json")).unwrap();
    favourites_file(support, &["/Volumes/T7/Scans"]);
    assert_eq!(places_held(support).paths(), ["/Volumes/T7/Scans"], "the old file's favourites once it reads");
    assert_eq!(places(support).unwrap(), Imported::Brought, "the next launch brings it");
    assert_eq!(Places::load(support).unwrap().paths(), ["/Volumes/T7/Scans"]);
    // Twin: the roots seeded and written over the pending import make the next launch import nothing.
    std::fs::remove_file(Places::file_path(support)).unwrap();
    Places::seeded(&Roots::load(support)).save(support).unwrap();
    assert_eq!(places(support).unwrap(), Imported::Nothing);
    assert_eq!(Places::load(support).unwrap().paths(), ["/Users/a/Roots"], "the old favourites lost");
}

#[test]
fn an_old_file_that_does_not_parse_gives_nothing_and_stays() {
    let scratch = Scratch::new("migrate-unparsed");
    let support = scratch.path();
    std::fs::write(support.join("favourites.json"), b"{\"version\":1,\"paths\":[").unwrap();
    std::fs::write(support.join("projects.json"), b"{\"version\":1,").unwrap();
    assert_eq!((places(support).unwrap(), projects(support).unwrap()), (Imported::Unparsed, Imported::Unparsed));
    assert_eq!(std::fs::read(support.join("favourites.json")).unwrap(), b"{\"version\":1,\"paths\":[", "left as it is");
    assert!(!crate::state::folder(support).exists());
    // Twin: a file that parses is brought.
    favourites_file(support, &["/Volumes/T7/Scans"]);
    assert_eq!(places(support).unwrap(), Imported::Brought);
}

#[test]
fn the_old_files_last_write_moves_when_an_older_build_writes_them() {
    let scratch = Scratch::new("migrate-written");
    let support = scratch.path();
    assert_eq!(last_written(support), None);
    favourites_file(support, &["/Volumes/T7/Scans"]);
    let file = std::fs::File::options().write(true).open(support.join("favourites.json")).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(60)).unwrap();
    let then = last_written(support).unwrap();
    std::fs::write(support.join("projects.json"), br#"{"version":1,"projects":[]}"#).unwrap();
    assert!(last_written(support).unwrap() > then, "the newer of the two");
    // Twin: `bookmarks.json`, which carries no change of the person's, moves nothing.
    let now = last_written(support);
    std::thread::sleep(Duration::from_millis(20));
    bookmarks_file(support);
    assert_eq!(last_written(support), now);
}
