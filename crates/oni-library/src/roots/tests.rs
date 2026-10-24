use super::*;
use crate::scratch::Scratch;

#[test]
fn a_scan_keeps_images_and_leaves_sidecars_hidden_folders_and_packages_out() {
    let scratch = Scratch::new("roots");
    for relative in [
        "a.RW2",
        "a.RW2.json",
        "Roll 1/b.tif",
        "Roll 1/Deep/c.jpg",
        ".hidden/d.RW2",
        "e.txt",
        "Album.photoslibrary/f.jpg",
        ".oni-roll.json",
    ] {
        let path = scratch.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, [0u8]).unwrap();
    }
    let root = paths::normal(&scratch.path().to_string_lossy()).into_owned();
    let scanned: BTreeSet<String> =
        scan(std::slice::from_ref(&root)).into_iter().map(|p| p[root.len() + 1..].to_owned()).collect();
    assert_eq!(scanned, BTreeSet::from(["a.RW2", "Roll 1/b.tif", "Roll 1/Deep/c.jpg"].map(String::from)));
    // Twin: a root one folder down finds its two images alone.
    assert_eq!(scan(&[paths::join(&root, "Roll 1")]).len(), 2);
    let surveyed = survey(std::slice::from_ref(&root), &|| true);
    assert_eq!(surveyed.folders.len(), 2);
    assert_eq!(surveyed.sizes.values().copied().collect::<Vec<_>>(), [1, 1, 1]);
    // Twin: a root that is not there is unreadable, never an empty folder.
    let gone = paths::join(&root, "gone");
    assert_eq!(survey(std::slice::from_ref(&gone), &|| true).unreadable, [gone]);
}

#[cfg(target_os = "macos")]
#[test]
fn a_folder_the_finder_hides_by_its_flag_is_left_out() {
    let scratch = Scratch::new("roots-flagged");
    for relative in ["Recovery/a.RW2", "Film/b.RW2"] {
        let path = scratch.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, [0u8]).unwrap();
    }
    let root = paths::normal(&scratch.path().to_string_lossy()).into_owned();
    assert_eq!(scan(std::slice::from_ref(&root)).len(), 2, "twin: unflagged, both folders count");
    let flagged = std::process::Command::new("/usr/bin/chflags").arg("hidden").arg(scratch.path().join("Recovery")).status();
    assert!(flagged.is_ok_and(|s| s.success()));
    assert_eq!(scan(std::slice::from_ref(&root)), [paths::join(&root, "Film/b.RW2")]);
    assert_eq!(crate::navigator::children(&root), Some(vec![paths::join(&root, "Film")]));
}

#[test]
fn a_stopped_survey_still_names_every_root_the_disk_refuses() {
    let scratch = Scratch::new("roots-stopped");
    std::fs::write(scratch.path().join("a.RW2"), [0u8]).unwrap();
    let here = paths::normal(&scratch.path().to_string_lossy()).into_owned();
    let gone = paths::join(&here, "gone");
    let both = [here.clone(), gone.clone()];
    let stopped = survey(&both, &|| false);
    assert_eq!((stopped.images.len(), stopped.unreadable), (0, vec![gone.clone()]));
    // Twin: left running, the same survey also lists the photo.
    let running = survey(&both, &|| true);
    assert_eq!((running.images.len(), running.unreadable), (1, vec![gone]));
}

#[test]
fn a_survey_reads_the_rolls_of_the_folders_holding_images() {
    let scratch = Scratch::new("roots-rolls");
    for relative in ["Roll 1/a.RW2", "Loose/b.RW2"] {
        let path = scratch.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, [0u8]).unwrap();
    }
    let root = paths::normal(&scratch.path().to_string_lossy()).into_owned();
    crate::folders::set_roll(Some(&Film::new("Roll 1")), &paths::join(&root, "Roll 1")).unwrap();
    let surveyed = survey(std::slice::from_ref(&root), &|| true);
    assert_eq!(surveyed.rolls.keys().cloned().collect::<Vec<_>>(), [paths::join(&root, "Roll 1")]);
    assert!(!surveyed.stopped && surveyed.presence(&paths::join(&root, "Loose/b.RW2")).is_present());
    // Twin: a survey stopped before its first entry says so.
    assert!(survey(std::slice::from_ref(&root), &|| false).stopped);
}

#[test]
fn roots_never_hold_a_photo_twice() {
    let mut roots = Roots::default();
    roots.add("/r/Roll 1");
    roots.add("/r");
    assert_eq!((roots.paths.clone(), roots.current()), (vec!["/r".to_owned()], Some("/r")));
    roots.add("/r/Roll 1/Deep");
    assert_eq!(roots.paths, ["/r"]);
    // Twin: a sibling sharing a prefix is a root of its own.
    roots.add("/ro");
    assert_eq!(roots.paths, ["/r", "/ro"]);
    assert_eq!(roots.current(), Some("/ro"));
}

#[test]
fn a_former_library_is_offered_per_volume_never_the_home_itself() {
    let home = "/Users/a";
    let proposed = suggested(
        &[
            "/Users/a/Pictures/Scans/2025/a.RW2",
            "/Users/a/Pictures/Scans/2026/Roll/b.RW2",
            "/Volumes/NAS/Film/c.tif",
            "/Users/a/Downloads/d.jpg",
        ]
        .map(String::from),
        home,
    );
    assert_eq!(proposed, ["/Users/a/Downloads", "/Users/a/Pictures", "/Volumes/NAS/Film"]);
    // Twin: photos under one folder of the home offer that folder.
    assert_eq!(
        suggested(
            &["/Users/a/Pictures/Scans/2025/a.RW2", "/Users/a/Pictures/Scans/2026/b.RW2"].map(String::from),
            home
        ),
        ["/Users/a/Pictures/Scans"]
    );
}

/// On Windows a drive roots the folders offered: the home's children, another drive's deepest
/// common folder; the twin, a path rooted at `/`, never gains a drive.
#[test]
fn a_former_library_on_windows_drives_is_offered_per_drive() {
    let members = ["C:/Users/a/Old/1.RW2", "C:/Users/a/Scans/Roll 1/2.RW2", "D:/Scans/A/3.NEF", "D:/Scans/B/4.NEF"].map(String::from);
    assert_eq!(suggested(&members, "C:/Users/a"), ["C:/Users/a/Old", "C:/Users/a/Scans", "D:/Scans"]);
    assert_eq!(suggested(&["/Volumes/T7/Scans/A/1.NEF".to_string()], "/Users/a"), ["/Volumes/T7/Scans/A"]);
}
