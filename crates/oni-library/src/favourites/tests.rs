use super::*;
use crate::scratch::Scratch;

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
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
    // Followed onto a favourite already there: kept once, where the earlier one stood.
    let mut both = Favourites { version: 1, paths: strings(&["/x/new", "/r/old", "/r/old/in"]) };
    both.follow("/r/old", "/x/new");
    assert_eq!(both.paths, ["/x/new", "/x/new/in"]);
}

#[test]
fn forgetting_a_folder_takes_its_favourites_and_those_inside_alone() {
    let mut favourites = Favourites { version: 1, paths: strings(&["/r/trip", "/r/trip/day2", "/r/tripod", "/s"]) };
    assert_eq!(favourites.forget("/r/trip"), 2);
    assert_eq!(favourites.paths, ["/r/tripod", "/s"]);
    // Twin: a folder holding no favourite takes none, a sibling named alike neither.
    assert_eq!(favourites.forget("/r/tri"), 0);
    assert_eq!(favourites.paths, ["/r/tripod", "/s"]);
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

/// A starred folder on a NAS asleep: its read hangs in the disk, and every other folder lands all
/// the same, on the very pool the navigator lists its rows with.
#[test]
fn a_stuck_favourite_does_not_block_the_others() {
    let run = |stuck: Duration| {
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let (send, landed) = mpsc::channel::<String>();
        let held = release.clone();
        let folders = strings(&["/Volumes/NAS/Scans", "/a", "/b", "/c"]);
        read_apart(
            folders,
            1,
            stuck,
            move |folder| {
                if folder.starts_with("/Volumes/NAS") {
                    let (lock, wake) = &*held;
                    let mut free = lock.lock().unwrap();
                    while !*free {
                        free = wake.wait(free).unwrap();
                    }
                }
                folder.len()
            },
            move |batch| batch.into_iter().for_each(|(f, _)| {
                let _ = send.send(f);
            }),
        );
        let began = Instant::now();
        let mut got = Vec::new();
        while got.len() < 3 && began.elapsed() < Duration::from_millis(600) {
            if let Ok(f) = landed.recv_timeout(Duration::from_millis(20)) {
                got.push(f);
            }
        }
        let (lock, wake) = &*release;
        *lock.lock().unwrap() = true;
        wake.notify_all();
        got
    };
    assert_eq!(run(STUCK), ["/a", "/b", "/c"], "the others land while the NAS sleeps");
    // Twin: a pool that waits for every read it started holds the others behind the stuck one.
    assert!(run(Duration::from_secs(3600)).is_empty());
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

/// The Finder renames `roll 8` to `Roll 8`: the star stays on its row and its photos stay inside it.
#[cfg(any(target_os = "macos", windows))]
#[test]
fn a_case_only_rename_outside_oni_keeps_the_favourite() {
    let scratch = Scratch::new("case");
    let root = crate::paths::normal(&scratch.path().to_string_lossy()).into_owned();
    assert!(crate::paths::ignores_case(&root), "the check runs on a volume blind to case");
    std::fs::create_dir_all(scratch.path().join("roll 8")).unwrap();
    std::fs::write(scratch.path().join("roll 8").join("a.NEF"), [0u8]).unwrap();
    let starred = crate::navigator::children(&root).unwrap().remove(0);
    let mut stars = Favourites::default();
    stars.toggle(&starred);
    stars.save(scratch.path()).unwrap();
    std::fs::rename(scratch.path().join("roll 8"), scratch.path().join("Roll 8")).unwrap();
    let row = crate::navigator::children(&root).unwrap().remove(0);
    assert!(row.ends_with("/Roll 8"), "the disk lists the new case");
    let image = crate::paths::canonical(&scratch.path().join("Roll 8").join("a.NEF"));
    let survey = crate::roots::survey(std::slice::from_ref(&row), &|| true);
    for loaded in [stars.clone(), Favourites::load(scratch.path()).unwrap()] {
        assert!(loaded.contains(&row), "the star stays on the renamed row");
        assert!(survey.images.iter().all(|i| loaded.paths.iter().any(|p| holds(p, i))) && survey.images == [image.clone()]);
    }
    stars.toggle(&row);
    assert!(stars.paths.is_empty(), "unstarring the row takes the old spelling with it");
    // Twin: compared byte for byte, the favourite names neither the row nor the photo.
    assert!(starred != row && !image.starts_with(&format!("{starred}/")));
}
