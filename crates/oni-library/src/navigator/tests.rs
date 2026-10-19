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
    assert_eq!((title(None), title(Some("/Volumes/T7"))), ("Computer".to_string(), "T7".to_string()));
}

#[test]
fn a_row_opens_or_unfolds_as_it_holds() {
    let some = strings(&["/a/b"]);
    let none: Vec<String> = Vec::new();
    assert_eq!(clicked(1, true, Some(&some)), Clicked::Open);
    assert_eq!(clicked(2, true, None), Clicked::Nothing);
    assert_eq!(clicked(1, false, Some(&none)), Clicked::Open);
    assert_eq!((clicked(1, false, Some(&some)), clicked(2, false, Some(&some))), (Clicked::Toggle, Clicked::Open));
    assert_eq!(clicked(1, false, None), Clicked::Read);
    // Twin: one first click, two answers by what the row holds.
    assert_ne!(clicked(1, false, Some(&none)), clicked(1, false, Some(&some)));
}

#[test]
fn an_unlisted_folder_is_read_then_opened_when_bare_and_unfolded_when_not() {
    let mut nav = Navigation::new(Some("/r".into()), false, BTreeSet::new(), vec![]);
    assert_eq!(nav.click("/r/a", 1, false), None);
    assert_eq!(nav.take_wanted(), ["/r/a"]);
    assert_eq!(nav.listed("/r/a", strings(&["/r/a/x"])), Landed::Toggled);
    assert!(nav.unfolded.contains("/r/a"));
    assert_eq!(nav.click("/r/b", 1, false), None);
    nav.take_wanted();
    assert_eq!(nav.listed("/r/b", Vec::new()), Landed::Open("/r/b".into()));
    // Twin: a listing nobody clicked for decides nothing.
    assert_eq!(nav.listed("/r/c", Vec::new()), Landed::Nothing);
    assert!(!nav.unfolded.contains("/r/b") && !nav.unfolded.contains("/r/c"));
    // A second click on an unfolded parent opens it and leaves it unfolded.
    assert_eq!(nav.click("/r/a", 2, false).as_deref(), Some("/r/a"));
    assert!(nav.unfolded.contains("/r/a"));
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
    assert_eq!(nav.click("/Volumes/T7", 1, false), None);
    assert_eq!(nav.take_wanted(), ["/Volumes/T7"]);
    assert_eq!(nav.listed("/Volumes/T7", strings(&["/Volumes/T7/Scans"])), Landed::Toggled);
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
