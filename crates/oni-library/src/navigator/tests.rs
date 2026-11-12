use super::*;
use crate::scratch::Scratch;

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn mac() -> Computer {
    Computer { home: "/Users/someone".into(), volumes: "/Volumes".into() }
}

#[test]
fn a_path_lies_on_a_volume_only_under_the_volumes_folder() {
    let c = mac();
    assert!(c.is_volume_root("/Volumes/T7") && !c.is_volume_root("/Volumes/T7/Scans"));
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
    assert_eq!(mac().name("/Volumes/T7"), "T7");
}

/// The Finder sidebar's rule: one click opens any folder, whatever it holds; the chevron alone
/// folds, and opens nothing.
#[test]
fn a_click_opens_and_the_chevron_folds() {
    let mut nav = Navigation::new(BTreeSet::new(), BTreeSet::new(), BTreeSet::new(), vec![]);
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
    let mut nav = Navigation::new(BTreeSet::new(), BTreeSet::new(), BTreeSet::new(), vec![]);
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
    let mut nav = Navigation::new(BTreeSet::new(), BTreeSet::from(["/Volumes/T7".to_string()]), BTreeSet::new(), strings(&["/Volumes/T7"]));
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
    let mut nav = Navigation::new(BTreeSet::new(), BTreeSet::from(["/h".to_string()]), BTreeSet::new(), strings(&["/h", "/Volumes/T7"]));
    let flat = |rows: Vec<(String, usize)>| rows.into_iter().collect::<Vec<_>>();
    let laid = |rows: &[(&str, usize)]| rows.iter().map(|(p, d)| (p.to_string(), *d)).collect::<Vec<_>>();
    assert_eq!(flat(nav.rows(&[])), laid(&[(COMPUTER_ROW, 0), ("/h", 0), ("/Volumes/T7", 0)]), "no favourite, no section for them");
    assert_eq!(nav.take_wanted(), ["/h"]);
    nav.rows(&[]);
    assert!(nav.take_wanted().is_empty(), "a listing under way is asked once");
    nav.listed("/h", strings(&["/h/a", "/h/b"]));
    let computer = [(COMPUTER_ROW, 0), ("/h", 0), ("/h/a", 1), ("/h/b", 1), ("/Volumes/T7", 0)];
    assert_eq!(flat(nav.rows(&[])), laid(&computer));
    // The favourites stand in a section of their own before Computer's, each folder in both.
    let favourites = strings(&["/f", "/h/a"]);
    let both = [(FAVOURITES_ROW, 0), ("/f", 0), ("/h/a", 0), (COMPUTER_ROW, 0), ("/h", 0), ("/h/a", 1), ("/h/b", 1), ("/Volumes/T7", 0)];
    assert_eq!(flat(nav.rows(&favourites)), laid(&both));
    // A folded section keeps its heading alone; twin: unfolded again, its rows come back.
    nav.toggle_section(COMPUTER_ROW);
    assert_eq!(flat(nav.rows(&favourites)), laid(&[(FAVOURITES_ROW, 0), ("/f", 0), ("/h/a", 0), (COMPUTER_ROW, 0)]));
    nav.toggle_section(COMPUTER_ROW);
    assert_eq!(flat(nav.rows(&favourites)), laid(&both));
    assert!(is_heading(FAVOURITES_ROW) && is_heading(COMPUTER_ROW) && !is_heading("/f"));
    // A folder in both sections unfolds where its chevron is; twin: Computer's row of it stays folded.
    nav.listed("/h/a", strings(&["/h/a/x"]));
    nav.toggle_in(FAVOURITES_ROW, "/h/a");
    let rows = flat(nav.rows(&favourites));
    assert_eq!(rows[..4], laid(&[(FAVOURITES_ROW, 0), ("/f", 0), ("/h/a", 0), ("/h/a/x", 1)])[..]);
    assert_eq!(rows.iter().filter(|(p, _)| p == "/h/a/x").count(), 1, "{rows:?}");
    let ids: Vec<String> = rows.iter().map(|(p, _)| p.clone()).collect();
    assert_eq!((heading_above(&ids, 3), heading_above(&ids, 6)), (FAVOURITES_ROW, COMPUTER_ROW));
}

#[test]
fn a_folder_both_sections_show_is_picked_in_one() {
    let mut nav = Navigation::new(BTreeSet::new(), BTreeSet::from(["/h".to_string()]), BTreeSet::new(), strings(&["/h"]));
    nav.listed("/h", strings(&["/h/a"]));
    let favourites = strings(&["/h/a"]);
    let rows: Vec<String> = nav.rows(&favourites).into_iter().map(|(p, _)| p).collect();
    assert_eq!(rows, strings(&[FAVOURITES_ROW, "/h/a", COMPUTER_ROW, "/h", "/h/a"]));
    // Picked under Computer, the pick stands on Computer's row of it alone; revealed, it stays there.
    nav.pick(&rows, 4);
    nav.reveal("/h/a", &favourites);
    assert_eq!((nav.selected.as_deref(), nav.picked_row(&rows)), (Some("/h/a"), Some(4)));
    // Twin: picked among the favourites, on the favourite's row.
    nav.pick(&rows, 1);
    nav.reveal("/h/a", &favourites);
    assert_eq!(nav.picked_row(&rows), Some(1));
    // A folder made inside it unfolds it in the pick's section alone.
    nav.made("/h/a/New Folder");
    assert!(nav.unfolded_favourites.contains("/h/a") && !nav.unfolded.contains("/h/a"));
    // Its section folded away, the pick falls on the row still showing the folder.
    nav.pick(&rows, 4);
    nav.toggle_section(COMPUTER_ROW);
    let folded: Vec<String> = nav.rows(&favourites).into_iter().map(|(p, _)| p).collect();
    assert_eq!(nav.picked_row(&folded), Some(1));
}

#[test]
fn a_rename_carries_what_the_navigator_holds() {
    let mut moved = Navigation::new(BTreeSet::new(), BTreeSet::from(["/r/trip/day2".to_string(), "/r/tripod".to_string()]), BTreeSet::new(), vec![]);
    moved.beyond.insert("/r/trip".into());
    moved.selected = Some("/r/trip/day2".into());
    moved.listings.insert("/r/trip".into(), vec![]);
    moved.listings.insert("/r/tripod".into(), vec![]);
    moved.follow("/r/trip", "/s/trip");
    assert_eq!(moved.beyond, BTreeSet::from(["/s/trip".to_string()]));
    assert_eq!(moved.selected.as_deref(), Some("/s/trip/day2"));
    // Twin: a sibling whose name starts alike keeps its row and its listing.
    assert_eq!(moved.unfolded, BTreeSet::from(["/s/trip/day2".to_string(), "/r/tripod".to_string()]));
    assert!(moved.listings.contains_key("/r/tripod") && !moved.listings.contains_key("/r/trip"));
}

#[test]
fn the_open_photos_folder_unfolds_under_its_section() {
    let mut nav = Navigation::new(BTreeSet::from([COMPUTER_ROW.to_string()]), BTreeSet::new(), BTreeSet::new(), strings(&["/Users/someone", "/Volumes/T7"]));
    nav.show("/Users/someone/Scans/2026", &[]);
    assert_eq!(nav.unfolded, BTreeSet::from(["/Users/someone".to_string(), "/Users/someone/Scans".to_string()]));
    assert_eq!(nav.selected.as_deref(), Some("/Users/someone/Scans/2026"));
    assert!(nav.folded.is_empty() && nav.beyond.is_empty(), "its section opens; nothing beyond Computer's rows");
    // Under a favourite holding it, the favourite's section unfolds down to it rather than Computer's.
    let mut starred = Navigation::new(BTreeSet::from([FAVOURITES_ROW.to_string()]), BTreeSet::new(), BTreeSet::new(), strings(&["/Volumes/T7"]));
    starred.show("/Volumes/T7/Film/Roll 1", &strings(&["/Volumes/T7/Film"]));
    assert_eq!(starred.unfolded_favourites, BTreeSet::from(["/Volumes/T7/Film".to_string()]));
    assert!(starred.folded.is_empty() && starred.unfolded.is_empty(), "twin: Computer's rows stay folded");
    // Beyond the home and the volumes, it comes under Computer as itself: nothing above it is listed.
    nav.show("/opt/scans/2026", &[]);
    assert_eq!(nav.beyond, BTreeSet::from(["/opt/scans/2026".to_string()]));
    assert!(!nav.unfolded.iter().any(|u| u.starts_with("/opt")), "{:?}", nav.unfolded);
    assert_eq!(nav.computer_tops(), strings(&["/Users/someone", "/Volumes/T7", "/opt/scans/2026"]));
    // A folder beyond holding it takes its place, and a folder under the home brings nothing beyond.
    nav.show("/opt/scans", &[]);
    nav.show("/Users/someone/Scans", &[]);
    assert_eq!(nav.beyond, BTreeSet::from(["/opt/scans".to_string()]));
    // Twin: once a volume listed holds a folder beyond, the folder is unfolded down to under it alone.
    nav.show("/Volumes/T8/Film", &[]);
    assert!(nav.beyond.contains("/Volumes/T8/Film"));
    nav.set_volumes(strings(&["/Users/someone", "/Volumes/T7", "/Volumes/T8"]));
    assert_eq!(nav.computer_tops(), strings(&["/Users/someone", "/Volumes/T7", "/Volumes/T8", "/opt/scans"]));
    assert!(nav.unfolded.contains("/Volumes/T8") && nav.selected.as_deref() == Some("/Volumes/T8/Film"));
    // A folder made inside another is picked and named, its parent unfolded.
    nav.made("/Users/someone/Scans/New Folder");
    assert_eq!(nav.renaming.as_deref(), Some("/Users/someone/Scans/New Folder"));
    assert!(nav.unfolded.contains("/Users/someone/Scans"));
}

#[test]
fn a_folder_opened_elsewhere_is_picked_where_it_shows_else_unfolded_down_to() {
    let mut nav = Navigation::new(BTreeSet::new(), BTreeSet::from(["/Users/someone".to_string()]), BTreeSet::new(), strings(&["/Users/someone"]));
    nav.listings.insert("/Users/someone".into(), strings(&["/Users/someone/Scans"]));
    nav.reveal("/Users/someone/Scans", &[]);
    assert_eq!(nav.selected.as_deref(), Some("/Users/someone/Scans"));
    assert_eq!(nav.unfolded.len(), 1, "on screen already, nothing more unfolds");
    // Twin: one no row shows, beyond Computer's rows, comes under them as itself, picked.
    nav.reveal("/Volumes/T7/Film", &[]);
    assert_eq!((nav.beyond.iter().next().map(String::as_str), nav.selected.as_deref()), (Some("/Volumes/T7/Film"), Some("/Volumes/T7/Film")));
    assert!(!nav.unfolded.contains("/Volumes/T7"));
    nav.toggle_trash();
    assert_eq!(nav.selected.as_deref(), Some(TRASH_ROW));
    assert!(nav.unfolded.contains(TRASH_ROW));
}

/// Windows mounts no disk in one folder: each drive's root is a volume, and a drive other than the
/// home's comes and goes.
#[test]
fn on_windows_each_drive_is_a_volume() {
    let c = Computer { home: r"C:\Users\someone".into(), volumes: DRIVES.into() };
    assert!(c.is_volume_root(r"D:\") && c.is_volume_root("e:/"));
    assert!(!c.is_volume_root(r"D:\Scans"), "twin: a folder on the drive is not its root");
    assert!(c.is_on_volume(r"D:\Scans\Roll 8") && !c.is_on_volume(r"C:\Scans"));
    // Twin: `/Volumes` reads none of them as a volume.
    assert!(!mac().is_volume_root(r"D:\"));
}

/// A network share comes and goes like a drive: its root is `//server/share`, whatever lies under it.
#[test]
fn a_share_is_a_volume_on_windows() {
    let c = Computer { home: "C:/Users/someone".into(), volumes: DRIVES.into() };
    assert_eq!(c.volume_root("//nas/film/Roll 8/A.NEF").as_deref(), Some("//nas/film"));
    assert_eq!(c.volume_root(r"\\nas\film").as_deref(), Some("//nas/film"));
    assert_eq!(c.volume_root("D:/Scans").as_deref(), Some("D:/"));
    // Twins: the home's drive, a server alone and a device path are no volume, nor a share on the Mac.
    assert_eq!(c.volume_root("C:/Users/someone/Scans"), None);
    assert_eq!((c.volume_root("//nas"), c.volume_root("//?/C:/x")), (None, None));
    assert_eq!(mac().volume_root("//nas/film/A.NEF"), None);
    assert!(!c.is_on_volume("//nas/film/A.NEF"), "Computer's rows list no share");
}

/// A volumes' folder handed in Windows' own spelling, `\` and another case, holds the archive's `/` paths.
#[cfg(windows)]
#[test]
fn an_injected_volumes_folder_is_read_in_any_spelling_on_windows() {
    let c = Computer { home: r"C:\x\home".into(), volumes: r"C:\x\Volumes".into() };
    assert!(c.is_on_volume("C:/x/Volumes/T7/Roll") && c.is_on_volume("c:/X/volumes/T7"));
    assert!(c.is_volume_root("C:/x/Volumes/T7") && !c.is_volume_root("C:/x/Volumes/T7/Roll"));
    // Twin: a sibling named like it, and the folder itself, lie on no volume.
    assert!(!c.is_on_volume("C:/x/Volumesque/T7") && !c.is_on_volume("C:/x/Volumes"));
    assert_eq!(c.volume_root("C:/x/Volumes/T7/Roll/A.NEF").as_deref(), Some("C:/x/Volumes/T7"));
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
    use crate::favourites::Favourites;
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
    let area = crate::roots::survey(std::slice::from_ref(&root), &|| true);
    assert!(area.rolls.contains_key(&row) && area.folders.contains(&ete), "the roll's icon falls on its row");
    assert!(!area.images.is_empty() && area.images.iter().all(|i| holds(&row, i)));
    let mut nav = Navigation::new(BTreeSet::new(), BTreeSet::new(), BTreeSet::new(), vec![root.clone()]);
    nav.show(&row, &[]);
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
    assert!(stars.contains(&row) && area.images.iter().all(|i| stars.paths.iter().any(|p| holds(p, i))), "the star holds its photos");
    // Twin: the row the disk's listing spells, joined byte for byte, is no folder the survey knows.
    let listed = format!("{root}/{}", raw[0]);
    assert!(listed != ete && !area.folders.contains(&listed));
}
