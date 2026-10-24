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

/// A share keeps its two slashes: `/nas/film` would name a folder of the current drive.
#[test]
fn a_former_library_on_a_share_is_offered_on_that_share() {
    assert_eq!(suggested(&["//nas/film/Scans/A/1.NEF".to_string()], "C:/Users/a"), ["//nas/film/Scans/A"]);
    let spread = ["//nas/film/A/1.NEF", "//nas/film/B/2.NEF", "//nas/other/C/3.NEF"].map(String::from);
    assert_eq!(suggested(&spread, "C:/Users/a"), ["//nas/film/A", "//nas/film/B", "//nas/other/C"]);
    // Twin: a path rooted at one slash stays there, and never becomes a share.
    assert_eq!(suggested(&["/nas/film/Scans/A/1.NEF".to_string()], "/Users/a"), ["/nas/film/Scans/A"]);
}

fn files(root: &Path, relatives: &[&str]) {
    for relative in relatives {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, [0u8]).unwrap();
    }
}

/// The Finder's bundle bit, `kHasBundle` in the Finder flags, as the Photo Booth library carries.
#[cfg(target_os = "macos")]
fn bundled(folder: &Path) {
    use std::os::unix::ffi::OsStrExt;
    let mut info = [0u8; 32];
    info[8] = 0x20;
    let name = std::ffi::CString::new(folder.as_os_str().as_bytes()).unwrap();
    // SAFETY: a valid C string and 32 bytes of attribute.
    let set = unsafe { libc::setxattr(name.as_ptr(), c"com.apple.FinderInfo".as_ptr(), info.as_ptr().cast(), 32, 0, 0) };
    assert_eq!(set, 0);
}

#[test]
fn photo_booth_library_is_a_package() {
    let scratch = Scratch::new("roots-packages");
    files(scratch.path(), &["Photo Booth Library/Pictures/a.jpg", "Photo Booth Library/Contents/PkgInfo", "Lr.lrlibrary/b.jpg", "Album.photoslibrary/c.jpg", "Film/d.RW2"]);
    let root = paths::canonical(scratch.path());
    let booth = scratch.path().join("Photo Booth Library");
    let listed = |root: &str| scan(&[root.to_owned()]).into_iter().map(|p| p[root.len() + 1..].to_owned()).collect::<BTreeSet<_>>();
    // Twin: without its bundle bit the library is a plain folder, its photo listed.
    assert!(!is_package(&booth) && listed(&root).contains("Photo Booth Library/Pictures/a.jpg"));
    #[cfg(target_os = "macos")]
    {
        bundled(&booth);
        assert!(is_package(&booth), "a package with no extension");
        assert_eq!(package_holding(&booth.join("Pictures")), Some(booth.clone()));
        // A `.lrlibrary` no installed app declares is a folder, as the Finder shows it.
        assert_eq!(listed(&root), BTreeSet::from(["Film/d.RW2", "Lr.lrlibrary/b.jpg"].map(String::from)));
        let rows = crate::navigator::children(&root).unwrap();
        assert_eq!(rows, [paths::join(&root, "Film"), paths::join(&root, "Lr.lrlibrary")]);
    }
    assert!(is_package(&scratch.path().join("Album.photoslibrary")));
    assert_eq!(package_holding(&scratch.path().join("Film")), None);
}

#[cfg(unix)]
#[test]
fn a_symlinked_folder_is_listed_and_a_loop_is_not_followed() {
    use std::os::unix::fs::symlink;
    let scratch = Scratch::new("roots-links");
    files(scratch.path(), &["Archive/Real/a.NEF", "T7/Scans/b.NEF", "T7/Scans/Deep/c.NEF"]);
    let archive = scratch.path().join("Archive");
    let scans = scratch.path().join("T7/Scans");
    symlink(&scans, archive.join("Scans")).unwrap();
    // Loops: to the root itself, to a folder inside it, and from inside the linked folder back to it.
    symlink(&archive, archive.join("Up")).unwrap();
    symlink(archive.join("Real"), archive.join("Again")).unwrap();
    symlink(&scans, scans.join("Deep/Back")).unwrap();
    symlink(scratch.path().join("gone"), archive.join("Dangling")).unwrap();
    let root = paths::canonical(&archive);
    let surveyed = survey(std::slice::from_ref(&root), &|| true);
    let images: BTreeSet<String> = surveyed.images.iter().map(|p| p[root.len() + 1..].to_owned()).collect();
    assert_eq!(images, BTreeSet::from(["Real/a.NEF", "Scans/b.NEF", "Scans/Deep/c.NEF"].map(String::from)), "each photo once, under the link's name");
    assert!(surveyed.folders.contains(&paths::join(&root, "Scans")));
    let rows = crate::navigator::children(&root).unwrap();
    assert_eq!(rows, [paths::join(&root, "Real"), paths::canonical(&scans)], "a link by its folder");
    // Twin: the link's own folder never lists itself back, and an unfolded loop shows once.
    assert!(!rows.contains(&root));
    let back = crate::navigator::children(&paths::join(&paths::canonical(&scans), "Deep")).unwrap();
    assert!(back.is_empty(), "{back:?}");
    let mut nav = crate::navigator::Navigation::new(Some(root.clone()), false, BTreeSet::from([root.clone(), "/x".to_owned(), "/y".to_owned()]), Vec::new());
    nav.listed(&root, vec!["/x".to_owned()]);
    nav.listed("/x", vec!["/y".to_owned()]);
    nav.listed("/y", vec!["/x".to_owned()]);
    let laid: Vec<String> = nav.rows(&[]).into_iter().map(|(p, _)| p).collect();
    assert_eq!(laid, ["/x", "/y", "/x"]);
}

#[cfg(target_os = "macos")]
#[test]
fn a_finder_alias_to_a_folder_is_listed_by_its_folder() {
    let scratch = Scratch::new("roots-alias");
    files(scratch.path(), &["Home/note.txt", "T7/Scans/b.NEF"]);
    let scans = scratch.path().join("T7/Scans");
    assert!(make_alias(&scans, &scratch.path().join("Home/Scans alias")));
    let home = paths::canonical(&scratch.path().join("Home"));
    assert_eq!(crate::navigator::children(&home), Some(vec![paths::canonical(&scans)]));
    // Twin: a plain file of that name is no row.
    assert_eq!(alias_target(&scratch.path().join("Home/note.txt")), None);
}

#[test]
fn a_file_still_being_written_is_settling_until_it_rests() {
    use std::io::Write;
    let scratch = Scratch::new("roots-settling");
    files(scratch.path(), &["Roll/a.NEF", "Roll/b.NEF", "Roll/c.NEF"]);
    let roll = scratch.path().join("Roll");
    let future = SystemTime::now() + std::time::Duration::from_secs(365 * 86_400);
    // A camera's clock a year ahead: a date to come is no copy.
    std::fs::OpenOptions::new().write(true).open(roll.join("c.NEF")).unwrap().set_modified(future).unwrap();
    // A copy in progress: `a.NEF` grows while the survey looks.
    let growing = roll.join("a.NEF");
    let copying = std::thread::spawn(move || {
        let mut file = std::fs::OpenOptions::new().append(true).open(growing).unwrap();
        for _ in 0..40 {
            file.write_all(&[1; 4096]).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    });
    std::thread::sleep(std::time::Duration::from_millis(20));
    let root = paths::canonical(scratch.path());
    let surveyed = survey(std::slice::from_ref(&root), &|| true);
    copying.join().unwrap();
    let settling: BTreeSet<String> = surveyed.settling.iter().map(|p| paths::name(p)).collect();
    assert_eq!(settling, BTreeSet::from(["a.NEF".to_owned()]), "b.NEF, written as recently, rests");
    // Twin: once the copy ends, every file is read, the one just written too.
    assert!(survey(std::slice::from_ref(&root), &|| true).settling.is_empty());
}

#[cfg(unix)]
#[test]
fn a_link_to_a_whole_disk_or_the_home_is_never_walked() {
    use std::os::unix::fs::symlink;
    let scratch = Scratch::new("roots-wide-links");
    files(scratch.path(), &["Archive/a.NEF", "Elsewhere/b.NEF"]);
    let archive = scratch.path().join("Archive");
    symlink("/", archive.join("Disk")).unwrap();
    symlink(crate::navigator::home_folder(), archive.join("Home")).unwrap();
    symlink(scratch.path().join("Elsewhere"), archive.join("Elsewhere")).unwrap();
    let root = paths::canonical(&archive);
    let surveyed = survey(std::slice::from_ref(&root), &|| true);
    let images: BTreeSet<String> = surveyed.images.iter().map(|p| p[root.len() + 1..].to_owned()).collect();
    // Twin: a link to a plain folder is still walked under its name.
    assert_eq!(images, BTreeSet::from(["a.NEF", "Elsewhere/b.NEF"].map(String::from)));
}
