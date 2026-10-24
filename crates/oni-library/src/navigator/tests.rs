use super::*;
use crate::scratch::Scratch;

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn mac() -> Computer {
    Computer { home: "/Users/someone".into(), volumes: "/Volumes".into() }
}

#[test]
fn the_navigator_climbs_one_folder_at_a_time_up_to_computer() {
    let c = mac();
    assert_eq!(c.parent("/Users/someone/Scans/2026").as_deref(), Some("/Users/someone/Scans"));
    assert_eq!((c.parent("/Users/someone"), c.parent("/Volumes/T7")), (None, None));
    assert_eq!(c.parent("/tmp"), None, "the boot volume's top reaches Computer, never `/`");
    // Twin: a folder named like a volume but one level deeper climbs to the volume.
    assert_eq!(c.parent("/Volumes/T7/Scans").as_deref(), Some("/Volumes/T7"));
    assert_eq!(c.ancestors("/Volumes/T7/Scans/2026"), ["/Volumes/T7/Scans/2026", "/Volumes/T7/Scans", "/Volumes/T7"]);
    assert!(c.is_on_volume("/Volumes/T7/a") && !c.is_on_volume("/Volumesque/a") && !c.is_on_volume("/Users/someone"));
    // Twin: no volumes' folder known, no path is on a volume, the home's least of all.
    assert!(!Computer::none().is_on_volume("/Users/someone/Scans"));
}

#[test]
fn computer_lists_the_home_first_then_every_volume_but_the_boot_one() {
    let scratch = Scratch::new("computer");
    let root = scratch.path().to_string_lossy().into_owned();
    let volumes = paths::join(&root, "Volumes");
    for name in ["T7 10", "T7 2", ".hidden"] {
        std::fs::create_dir_all(paths::join(&volumes, name)).unwrap();
    }
    std::fs::create_dir_all(paths::join(&root, "home")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("/", paths::join(&volumes, "Macintosh HD")).unwrap();
    let c = Computer { home: paths::join(&root, "home"), volumes: volumes.clone() };
    let listed = c.computer();
    assert_eq!(listed, [c.home.clone(), paths::join(&volumes, "T7 2"), paths::join(&volumes, "T7 10")]);
    // Twin: the boot volume, a link to `/`, is never listed, nor `/` itself.
    assert!(!listed.iter().any(|p| p == "/" || p.ends_with("Macintosh HD")));
}

#[test]
fn a_folder_lists_its_subfolders_alone_in_the_finders_order() {
    let scratch = Scratch::new("navigator");
    for relative in ["Roll 10/a.RW2", "Roll 2/b.RW2", "Roll 1", ".hidden", "Album.photoslibrary"] {
        std::fs::create_dir_all(scratch.path().join(relative)).unwrap();
    }
    std::fs::write(scratch.path().join("loose.RW2"), [0u8]).unwrap();
    let root = scratch.path().to_string_lossy().into_owned();
    let listed: Vec<String> = children(&root).unwrap().iter().map(|p| paths::name(p)).collect();
    assert_eq!(listed, ["Roll 1", "Roll 2", "Roll 10"]);
    // Twin: an empty folder lists nothing.
    assert_eq!(children(&paths::join(&root, "Roll 1")), Some(Vec::new()));
    // Twin: a folder the disk refuses is not an empty one.
    assert_eq!(children(&paths::join(&root, "gone")), None);
    assert_eq!((title(None, &mac()), title(Some("/Volumes/T7"), &mac())), ("Computer".to_string(), "T7".to_string()));
}

/// The Finder sidebar's rule: one click opens any folder, whatever it holds; the chevron alone
/// folds, and opens nothing.
#[test]
fn a_click_opens_and_the_chevron_folds() {
    let mut nav = Navigation::new(Some("/r".into()), false, BTreeSet::new(), vec![]);
    nav.listings.insert("/r/2024".into(), strings(&["/r/2024/Roll 12"]));
    assert_eq!(nav.click("/r/2024", 1).as_deref(), Some("/r/2024"), "a folder holding others opens on the first click");
    assert!(!nav.unfolded.contains("/r/2024"), "the click folds nothing");
    assert_eq!(nav.click("/r/2024/Roll 12", 1).as_deref(), Some("/r/2024/Roll 12"), "a leaf opens alike");
    assert_eq!(nav.click("/r/unlisted", 1).as_deref(), Some("/r/unlisted"), "an unlisted folder opens without waiting on its listing");
    // A double click's second opens nothing more.
    assert_eq!(nav.click("/r/unlisted", 2), None);
    nav.toggle("/r/2024");
    assert!(nav.unfolded.contains("/r/2024") && nav.selected.as_deref() == Some("/r/2024"));
    nav.toggle("/r/2024");
    assert!(!nav.unfolded.contains("/r/2024"), "twin: the chevron folds what it unfolded");
}

/// A listing read slowly, a T7 spinning up, lands after the person opened another folder: it lays
/// its rows and opens nothing, as no click waits on it.
#[test]
fn a_late_listing_never_opens_over_a_newer_open() {
    let mut nav = Navigation::new(Some("/Volumes/T7".into()), false, BTreeSet::new(), vec![]);
    assert_eq!(nav.click("/Volumes/T7/Roll 3", 1).as_deref(), Some("/Volumes/T7/Roll 3"));
    assert_eq!(nav.click("/Volumes/T7/Roll 4", 1).as_deref(), Some("/Volumes/T7/Roll 4"));
    nav.list("/Volumes/T7/Roll 3", false);
    assert_eq!(nav.take_wanted(), ["/Volumes/T7/Roll 3"]);
    let landed = nav.listed("/Volumes/T7/Roll 3", Vec::new());
    assert_eq!(landed, Landed::Nothing, "the empty listing landing late opens nothing");
    assert_eq!(nav.selected.as_deref(), Some("/Volumes/T7/Roll 4"), "the pick stays on the folder opened last");
    // Twin: the listing does land, its row now known to hold no folder.
    assert_eq!(nav.listings.get("/Volumes/T7/Roll 3"), Some(&Vec::new()));
    assert!(!nav.unfolded.contains("/Volumes/T7/Roll 3"));
}

#[test]
fn a_refused_folder_stays_unlisted_until_read_again() {
    let mut nav = Navigation::new(None, false, BTreeSet::from(["/Volumes/T7".to_string()]), strings(&["/Volumes/T7"]));
    nav.rows(&[]);
    assert_eq!(nav.take_wanted(), ["/Volumes/T7"]);
    assert_eq!(nav.refused("/Volumes/T7"), Landed::Nothing);
    assert!(!nav.listings.contains_key("/Volumes/T7"), "a refusal is never an empty listing");
    nav.rows(&[]);
    assert!(nav.take_wanted().is_empty(), "a refused folder is not asked on every publish");
    assert_eq!(nav.refused("/Volumes/T7"), Landed::Same);
    // A click on it reads it again, as does a retry.
    assert_eq!(nav.click("/Volumes/T7", 1).as_deref(), Some("/Volumes/T7"));
    assert_eq!(nav.take_wanted(), ["/Volumes/T7"]);
    assert_eq!(nav.listed("/Volumes/T7", strings(&["/Volumes/T7/Scans"])), Landed::Nothing);
    assert!(nav.refused.is_empty());
    nav.refused("/Volumes/T7/Scans");
    nav.retry();
    assert_eq!(nav.take_wanted(), ["/Volumes/T7/Scans"]);
    // Twin: the same listing landing again asks no publish, a new one does.
    assert_eq!(nav.listed("/Volumes/T7", strings(&["/Volumes/T7/Scans"])), Landed::Same);
    assert_eq!(nav.listed("/Volumes/T7", strings(&["/Volumes/T7/Film"])), Landed::Nothing);
}

#[test]
fn the_rows_walk_the_unfolded_folders_and_ask_each_listing_once() {
    let mut nav = Navigation::new(None, false, BTreeSet::from(["/h".to_string()]), strings(&["/h", "/Volumes/T7"]));
    assert_eq!(nav.rows(&[]), [("/h".to_string(), 0), ("/Volumes/T7".to_string(), 0)]);
    assert_eq!(nav.take_wanted(), ["/h"]);
    nav.rows(&[]);
    assert!(nav.take_wanted().is_empty(), "a listing under way is asked once");
    nav.listed("/h", strings(&["/h/a", "/h/b"]));
    let rows = nav.rows(&[]);
    assert_eq!(rows.iter().map(|(p, d)| (p.as_str(), *d)).collect::<Vec<_>>(), [("/h", 0), ("/h/a", 1), ("/h/b", 1), ("/Volumes/T7", 0)]);
    // Twin: the favourites view lists the favourites alone, whatever the base.
    nav.show_favourites(true);
    assert_eq!(nav.rows(&strings(&["/f"])), [("/f".to_string(), 0)]);
}

#[test]
fn going_up_moves_the_base_and_a_rename_carries_what_the_navigator_holds() {
    let c = mac();
    let mut nav = Navigation::new(Some("/Users/someone/Scans".into()), false, BTreeSet::new(), vec![]);
    assert!(nav.can_go_up());
    nav.go_up(&c);
    assert_eq!(nav.base.as_deref(), Some("/Users/someone"));
    nav.go_up(&c);
    assert_eq!(nav.base, None);
    assert!(!nav.can_go_up());
    // In the favourites view, Parent climbs from the picked favourite.
    nav.show_favourites(true);
    assert!(!nav.can_go_up());
    nav.selected = Some("/Volumes/T7/Film".into());
    nav.go_up(&c);
    assert_eq!((nav.base.as_deref(), nav.favourites_only), (Some("/Volumes/T7"), false));

    let mut moved = Navigation::new(Some("/r/trip".into()), false, BTreeSet::from(["/r/trip/day2".to_string(), "/r/tripod".to_string()]), vec![]);
    moved.selected = Some("/r/trip/day2".into());
    moved.listings.insert("/r/trip".into(), vec![]);
    moved.listings.insert("/r/tripod".into(), vec![]);
    moved.follow("/r/trip", "/s/trip");
    assert_eq!(moved.base.as_deref(), Some("/s/trip"));
    assert_eq!(moved.selected.as_deref(), Some("/s/trip/day2"));
    // Twin: a sibling whose name starts alike keeps its row and its listing.
    assert_eq!(moved.unfolded, BTreeSet::from(["/s/trip/day2".to_string(), "/r/tripod".to_string()]));
    assert!(moved.listings.contains_key("/r/tripod") && !moved.listings.contains_key("/r/trip"));
}

#[test]
fn the_open_photos_folder_is_unfolded_down_to_or_the_base_moves() {
    let c = mac();
    let mut nav = Navigation::new(None, false, BTreeSet::new(), strings(&["/Users/someone", "/Volumes/T7"]));
    nav.show("/Users/someone/Scans/2026", &[], &c);
    assert_eq!(nav.unfolded, BTreeSet::from(["/Users/someone".to_string(), "/Users/someone/Scans".to_string()]));
    assert_eq!((nav.base.as_deref(), nav.selected.as_deref()), (None, Some("/Users/someone/Scans/2026")));
    // Under a base, the folders between it and the photo's unfold; the base itself is the list.
    let mut based = Navigation::new(Some("/Volumes/T7".into()), false, BTreeSet::new(), vec![]);
    based.show("/Volumes/T7/Film/Roll 1", &[], &c);
    assert_eq!(based.unfolded, BTreeSet::from(["/Volumes/T7/Film".to_string()]));
    // Twin: a folder outside the base moves the base to its parent.
    based.show("/Users/someone/Scans/2026", &[], &c);
    assert_eq!(based.base.as_deref(), Some("/Users/someone/Scans"));
    // A folder made inside another is picked and named, its parent unfolded.
    based.made("/Users/someone/Scans/New Folder");
    assert_eq!(based.renaming.as_deref(), Some("/Users/someone/Scans/New Folder"));
    assert!(based.unfolded.contains("/Users/someone/Scans"));
}

#[test]
fn a_folder_opened_elsewhere_is_picked_where_it_shows_else_the_base_moves() {
    let c = mac();
    let mut nav = Navigation::new(Some("/Users/someone".into()), false, BTreeSet::new(), vec![]);
    nav.listings.insert("/Users/someone".into(), strings(&["/Users/someone/Scans"]));
    nav.reveal("/Users/someone/Scans", &c);
    assert_eq!((nav.base.as_deref(), nav.selected.as_deref()), (Some("/Users/someone"), Some("/Users/someone/Scans")));
    // Twin: one not on screen moves the base to its parent.
    nav.reveal("/Volumes/T7/Film", &c);
    assert_eq!((nav.base.as_deref(), nav.selected.as_deref()), (Some("/Volumes/T7"), Some("/Volumes/T7/Film")));
    nav.toggle_trash();
    assert_eq!(nav.selected.as_deref(), Some(TRASH_ROW));
    assert!(nav.unfolded.contains(TRASH_ROW));
}

/// Windows mounts no disk in one folder: each drive's root is a volume, its parent Computer, and a
/// drive other than the home's comes and goes.
#[test]
fn on_windows_each_drive_is_a_volume() {
    let c = Computer { home: r"C:\Users\someone".into(), volumes: DRIVES.into() };
    assert!(c.is_volume_root(r"D:\") && c.is_volume_root("e:/"));
    assert!(!c.is_volume_root(r"D:\Scans"), "twin: a folder on the drive is not its root");
    assert!(c.is_on_volume(r"D:\Scans\Roll 8") && !c.is_on_volume(r"C:\Scans"));
    assert_eq!(c.parent(r"D:\"), None);
    // Twin: `/Volumes` reads none of them as a volume.
    assert!(!mac().is_volume_root(r"D:\"));
}


/// A disk image formatted exFAT like the T7, attached inside the scratch folder, detached on drop.
#[cfg(target_os = "macos")]
struct Exfat(std::path::PathBuf);

#[cfg(target_os = "macos")]
impl Exfat {
    fn attach(scratch: &Scratch) -> Option<Exfat> {
        use std::process::Command;
        let (image, mount) = (scratch.path().join("t7.dmg"), scratch.path().join("T7"));
        let run = |c: &mut Command| c.status().is_ok_and(|s| s.success());
        let made = run(Command::new("hdiutil").args(["create", "-quiet", "-size", "8m", "-fs", "ExFAT", "-volname", "T7", "-layout", "NONE"]).arg(&image));
        std::fs::create_dir_all(&mount).ok()?;
        let attached = made && run(Command::new("hdiutil").args(["attach", "-quiet", "-nobrowse", "-noverify", "-mountpoint"]).arg(&mount).arg(&image));
        attached.then_some(Exfat(mount))
    }
}

#[cfg(target_os = "macos")]
impl Drop for Exfat {
    fn drop(&mut self) {
        let _ = std::process::Command::new("hdiutil").args(["detach", "-quiet", "-force"]).arg(&self.0).status();
    }
}

/// exFAT lists `Été` decomposed while its real path is composed: the row, the opened folder, the
/// survey's roll and photos and the star all name it one way. Without `hdiutil`, APFS stands in.
#[cfg(target_os = "macos")]
#[test]
fn an_accented_folder_on_exfat_opens_marks_and_stars() {
    use crate::favourites::{Favourites, Stop, Surveys, read};
    let scratch = Scratch::new("exfat");
    let (nfc, nfd) = ("\u{c9}t\u{e9} 2025", "E\u{301}te\u{301} 2025");
    let exfat = Exfat::attach(&scratch);
    let disk = match &exfat {
        Some(Exfat(mount)) => mount.clone(),
        None => {
            eprintln!("hdiutil refused an exFAT image: decomposed names made on APFS stand in");
            scratch.path().join("apfs")
        }
    };
    std::fs::create_dir_all(disk.join(if exfat.is_some() { nfc } else { nfd }).join("Roll 3")).unwrap();
    let root = paths::canonical(&disk);
    let roll = paths::join(&root, &format!("{nfc}/Roll 3"));
    std::fs::write(paths::join(&roll, "a.NEF"), [0u8]).unwrap();
    crate::folders::set_roll(Some(&oni_settings::Film::new("Portra 400")), &roll).unwrap();
    let raw: Vec<String> = std::fs::read_dir(&disk)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.'))
        .collect();
    assert_eq!(raw, [nfd], "the disk lists the name decomposed");
    let ete = children(&root).unwrap().remove(0);
    let row = children(&ete).unwrap().remove(0);
    assert_eq!((ete.as_str(), row.as_str()), (paths::join(&root, nfc).as_str(), roll.as_str()));
    assert_eq!(paths::canonical(Path::new(&row)), row, "the opened folder is its row");
    let area = read(&root, &Stop::new()).unwrap();
    assert!(area.rolls.contains_key(&row) && area.folders.contains(&ete), "the roll's icon falls on its row");
    assert!(!area.images.is_empty() && area.images.iter().all(|i| holds(&row, i)));
    let mut nav = Navigation::new(Some(root.clone()), false, BTreeSet::new(), Vec::new());
    nav.show(&row, &[], &Computer::none());
    let mut wanted = nav.take_wanted();
    while !wanted.is_empty() {
        for folder in wanted {
            nav.listed(&folder, children(&folder).unwrap());
        }
        nav.rows(&[]);
        wanted = nav.take_wanted();
    }
    assert!(nav.unfolded.contains(&ete) && nav.rows(&[]).iter().any(|(p, _)| *p == row), "the photo's folder unfolds to its row");
    let mut stars = Favourites::default();
    stars.toggle(&row);
    let mut surveys = Surveys::default();
    surveys.store(area.clone(), &root);
    assert!(stars.contains(&row) && surveys.union(None, &stars.paths).images == area.images, "the star holds its photos");
    // Twin: the row the disk's listing spells, joined byte for byte, is no folder the survey knows.
    let listed = format!("{root}/{}", raw[0]);
    assert!(listed != ete && !area.folders.contains(&listed));
}
