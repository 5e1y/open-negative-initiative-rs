use super::*;
use crate::scratch::Scratch;
use std::time::{Duration, Instant};

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn area(images: &[&str], folders: &[&str]) -> Area {
    Area { images: strings(images), folders: strings(folders), ..Area::default() }
}

#[test]
fn favourites_star_beside_each_other_and_follow_a_rename() {
    let seeded =
        Favourites::seeded(&Roots { version: 1, paths: strings(&["/a", "/b/c"]), active: Some("/b/c".into()) });
    assert_eq!(seeded.paths, ["/a", "/b/c"]);
    let mut nested = Favourites { version: 1, paths: strings(&["/a"]) };
    nested.toggle("/a/b");
    assert_eq!(nested.paths, ["/a", "/a/b"]);
    nested.toggle("/a");
    assert_eq!(nested.paths, ["/a/b"]);
    let mut moved = Favourites { version: 1, paths: strings(&["/r/trip", "/r/trip/day2", "/r/tripod"]) };
    moved.follow("/r/trip", "/s/trip");
    assert_eq!(moved.paths, ["/s/trip", "/s/trip/day2", "/r/tripod"]);
}

#[test]
fn the_file_is_written_as_a_plain_encoder_writes_it() {
    let scratch = Scratch::new("favourites");
    assert_eq!(Favourites::load(scratch.path()), None);
    let favourites = Favourites { version: 1, paths: strings(&["/Users/a/Scans"]) };
    favourites.save(scratch.path()).unwrap();
    let text = std::fs::read_to_string(Favourites::file_path(scratch.path())).unwrap();
    assert_eq!(text, r#"{"version":1,"paths":["\/Users\/a\/Scans"]}"#);
    assert_eq!(Favourites::load(scratch.path()), Some(favourites));
}

#[test]
fn surveys_read_each_folder_once_the_opened_one_first() {
    assert_eq!(missing(&strings(&["/r/a", "/r", "/s"]), &[]), ["/r", "/s"]);
    assert!(missing(&strings(&["/r/a/b"]), &strings(&["/r"])).is_empty());
    // Twin: a sibling sharing a prefix covers nothing.
    assert_eq!(missing(&strings(&["/r/a/b"]), &strings(&["/r/a/bc"])), ["/r/a/b"]);
    assert_eq!(ordered(&missing(&strings(&["/r/a", "/r", "/s"]), &[]), Some("/r/a")), ["/r/a", "/r", "/s"]);
    // Twin: opened in a sibling sharing a prefix, nothing is read ahead.
    assert_eq!(ordered(&strings(&["/r/a", "/s"]), Some("/r/ab")), ["/r/a", "/s"]);

    let mut surveys = Surveys::default();
    let favourite = surveys.begin("/fav");
    surveys.yield_keeping(&strings(&["/fav/roll"]));
    let own = surveys.begin("/fav/roll");
    surveys.yield_keeping(&strings(&["/fav/roll"]));
    assert!(favourite.raised());
    // Twin: the opened folder's own read runs on.
    assert!(!own.raised());
}

#[test]
fn a_folder_read_again_inside_a_held_area_is_spliced_into_it() {
    let mut surveys = Surveys::default();
    surveys.store(area(&["/s/1"], &["/s"]), "/s");
    surveys.store(area(&["/r/a/1"], &["/r/a"]), "/r/a");
    surveys.store(area(&["/r/a/1", "/r/2"], &["/r", "/r/a"]), "/r");
    assert_eq!(surveys.areas.keys().cloned().collect::<Vec<_>>(), ["/r", "/s"]);
    assert_eq!(surveys.union(Some("/s/x"), &strings(&["/r", "/s"])).images, ["/s/1", "/r/a/1", "/r/2"]);
    assert_eq!(surveys.union(Some("/r/a"), &strings(&["/r/a"])).images, ["/r/a/1"]);

    let mut spliced = Surveys::default();
    spliced.store(area(&["/r/2", "/r/a/1", "/r/a/old", "/r/ab/3"], &["/r/a", "/r/a/sub", "/r/ab"]), "/r");
    spliced.store(area(&["/r/a/1", "/r/a/new"], &["/r/a/sub2"]), "/r/a");
    assert_eq!(spliced.areas.keys().cloned().collect::<Vec<_>>(), ["/r"]);
    assert_eq!(spliced.areas["/r"].images, ["/r/2", "/r/ab/3", "/r/a/1", "/r/a/new"]);
    // Twin: a sibling sharing the prefix and the folder itself stay.
    assert_eq!(spliced.areas["/r"].folders, ["/r/a", "/r/ab", "/r/a/sub2"]);
}

#[test]
fn a_read_stands_only_for_a_folder_it_saw_whole() {
    let scratch = Scratch::new("surveys");
    std::fs::write(scratch.path().join("a.RW2"), [0u8]).unwrap();
    std::fs::create_dir(scratch.path().join("empty")).unwrap();
    let root = crate::paths::normal(&scratch.path().to_string_lossy()).into_owned();
    let quiet = Stop::default();
    assert_eq!(read(&crate::paths::join(&root, "gone"), &quiet), None);
    let photo = read(&root, &quiet).and_then(|a| a.images.first().and_then(|i| a.sizes.get(i).copied()));
    assert_eq!(photo, Some(1));
    // Twin: a folder there and empty reads as an empty area.
    assert_eq!(read(&crate::paths::join(&root, "empty"), &quiet).map(|a| a.images.is_empty()), Some(true));
    let stopped = Stop::default();
    stopped.raise();
    assert_eq!(read(&root, &stopped), None);
    // Twin: read with nothing raised, the folder gives its photo.
    assert_eq!(read(&root, &quiet).map(|a| a.images.len()), Some(1));
}

#[test]
fn a_read_stuck_in_the_disk_lets_go_once_stopped() {
    let stuck = |_: String, _: Arc<Stop>| {
        std::thread::sleep(Duration::from_secs(1));
        Some(Area::default())
    };
    let timed = |raising: bool| {
        let stop = Stop::new();
        let began = Instant::now();
        if raising {
            let later = stop.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(50));
                later.raise();
            });
        }
        let got = reading("/stuck", stop, stuck);
        (began.elapsed().as_secs_f64(), got.is_some())
    };
    let (left, gave) = timed(true);
    assert!(left < 0.5 && !gave, "stopped after {left:.2} s");
    // Twin: the same read left running is waited for whole.
    let (waited, gave) = timed(false);
    assert!(waited >= 0.9 && gave, "waited {waited:.2} s");
}

/// A favourites file that stays unreadable is set aside whole before the seeded one is written;
/// the twin, a file that reads, stays where it is.
#[test]
fn an_unreadable_favourites_file_is_set_aside_never_written_over() {
    let scratch = Scratch::new("favourites-unreadable");
    let file = Favourites::file_path(scratch.path());
    let torn = b"{\"version\": 1, \"paths\": [\"/Volumes/T7/Scans\"";
    std::fs::write(&file, torn).unwrap();
    assert_eq!(Favourites::load(scratch.path()), None);
    assert!(!file.exists(), "the torn file left its place");
    let aside: Vec<_> = std::fs::read_dir(scratch.path()).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with("favourites.json.unreadable-")).collect();
    assert_eq!(aside.len(), 1);
    assert_eq!(std::fs::read(aside[0].path()).unwrap(), torn);
    std::fs::write(&file, b"{\"version\": 1, \"paths\": [\"/Volumes/T7/Scans\"]}").unwrap();
    assert_eq!(Favourites::load(scratch.path()).map(|f| f.paths), Some(vec!["/Volumes/T7/Scans".to_string()]));
    assert!(file.exists(), "twin: a file that reads stays");
}
