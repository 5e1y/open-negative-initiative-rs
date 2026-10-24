//! The navigator on the headless window over a disk of its own: clicks, favourites, the base, the
//! folder menu on folders made for the check, the photo's folder revealed, a former library, a relaunch.

use oni_app::app::{Options, Shell};
use oni_app::preferences::Preferences;
use oni_app::{Advice, App, FolderLine, Navigator};
use oni_core::{Archive, EditorStore};
use oni_library::navigator::Computer;
use oni_library::paths::{self, canonical};
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::common;

fn lines(w: &oni_app::MainWindow) -> Vec<FolderLine> {
    let rows = w.global::<Navigator>().get_rows();
    (0..rows.row_count()).filter_map(|i| rows.row_data(i)).collect()
}

fn titles(w: &oni_app::MainWindow) -> Vec<String> {
    lines(w).iter().map(|l| l.title.to_string()).collect()
}

fn at(w: &oni_app::MainWindow, title: &str) -> usize {
    titles(w).iter().position(|t| t == title).unwrap_or_else(|| panic!("{title} is not among {:?}", titles(w)))
}

fn row(w: &oni_app::MainWindow, title: &str) -> FolderLine {
    lines(w)[at(w, title)].clone()
}

fn click(rig: &common::Rig, w: &oni_app::MainWindow, title: &str) {
    w.global::<Navigator>().invoke_row_clicked(at(w, title) as i32);
    rig.pump();
}

/// Lets the double-click interval pass, so the next click is a first one.
fn rest(rig: &common::Rig) {
    let start = Instant::now();
    while start.elapsed() < oni_app::system::double_click() + Duration::from_millis(50) {
        rig.pump();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn menu(w: &oni_app::MainWindow, title: &str) -> Vec<(String, bool, bool)> {
    let n = w.global::<Navigator>();
    assert!(n.invoke_row_menu(at(w, title) as i32));
    let m = n.get_menu_lines();
    (0..m.row_count()).filter_map(|i| m.row_data(i)).map(|l| (l.text.to_string(), l.enabled, l.rule)).collect()
}

fn choose(rig: &common::Rig, w: &oni_app::MainWindow, title: &str, item: &str) {
    let i = menu(w, title).iter().position(|(t, _, _)| t == item).unwrap_or_else(|| panic!("{item} is not in {title}'s menu"));
    w.global::<Navigator>().invoke_menu_chosen(i as i32);
    rig.pump();
}

pub(crate) fn the_navigator_walks_its_disk_from_computer_and_the_next_launch_finds_it_where_it_was_left() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the navigator is not exercised"),
    };
    let made = std::env::temp_dir().join(format!("oni-app-navigator-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&made);
    std::fs::create_dir_all(&made).unwrap();
    let root = PathBuf::from(canonical(&made));
    let (home, volumes, support, caches) = (root.join("Home"), root.join("Volumes"), root.join("Support"), root.join("Caches"));
    common::linear_tiff(&home.join("Scans/Roll 1/A.tif"), 64, 48, 1.0);
    common::linear_tiff(&home.join("Scans/Roll 2/B.tif"), 64, 48, 0.6);
    common::linear_tiff(&volumes.join("T7/Film/C.tif"), 64, 48, 0.8);
    for empty in ["Scans/Empty", "Other", "Old"] {
        std::fs::create_dir_all(home.join(empty)).unwrap();
    }
    let p = |path: &Path| oni_library::paths::normal(&path.to_string_lossy()).into_owned();
    // A library made before the archive followed the disk: two photos in Old, one in Roll 1.
    let former = [p(&home.join("Old/x.tif")), p(&home.join("Old/y.tif")), p(&home.join("Scans/Roll 1/A.tif"))];
    std::fs::create_dir_all(&support).unwrap();
    std::fs::write(oni_library::library::file_path(&support), serde_json::json!({ "version": 2, "files": former }).to_string()).unwrap();
    let computer = Computer { home: p(&home), volumes: p(&volumes) };

    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    let launch = || {
        let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
        let options = Options {
            support: support.clone(),
            caches: caches.clone(),
            picker: Box::new(|| None),
            computer: computer.clone(),
            ..Options::default()
        };
        let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
        shell.window.show().unwrap();
        shell
    };
    let shell = launch();
    let w = &shell.window;
    let ctl = shell.ctl.clone();
    let n = w.global::<Navigator>();
    let opened = || ctl.state.borrow().opened().map(str::to_owned);

    // A former library: the catch-up sheet counts the photos no folder holds, and the welcome offers
    // the folders they were in, per volume, never the home itself.
    assert!(n.get_catch_up_shown());
    assert_eq!(n.get_catch_up_left().as_str(), "3 photos of your library are not in it yet:");
    let caught = n.get_catch_up_lines();
    let caught: Vec<(String, String)> = (0..caught.row_count()).filter_map(|i| caught.row_data(i)).map(|l| (l.folder.to_string(), l.count.to_string())).collect();
    assert_eq!(caught, [(p(&home.join("Old")), "2".to_string()), (p(&home.join("Scans/Roll 1")), "1".to_string())]);
    assert_eq!(n.get_catch_up_choose().as_str(), "ADD FOLDER…");
    assert!(n.get_welcome_shown());
    let welcome = n.get_welcome();
    let offered: Vec<String> = (0..welcome.row_count()).filter_map(|i| welcome.row_data(i)).map(|l| l.title.to_string()).collect();
    assert_eq!(offered, ["ADD OLD", "ADD SCANS"]);
    // Adding a folder from the sheet stars it and opens it; its photos are then in.
    n.invoke_catch_up_added(0);
    rig.until("Old opened", Duration::from_secs(30), || ctl.state.borrow().read_done() && opened().is_some());
    assert_eq!(opened().as_deref(), Some(p(&home.join("Old")).as_str()));
    assert_eq!(n.get_catch_up_left().as_str(), "1 photos of your library are not in it yet:");
    assert_eq!(n.get_catch_up_choose().as_str(), "ADD ANOTHER FOLDER…", "twin: a folder in, the key asks for another");
    rig.settle();
    println!("shot {}", rig.shot("roots-catch-up").display());
    assert!(!n.get_welcome_shown(), "a folder opened, the welcome leaves");
    n.invoke_catch_up_done();
    rig.pump();
    assert!(!n.get_catch_up_shown());
    assert_eq!(Preferences::load(&support).bool("rootsCatchUpShown"), Some(true));

    // Computer: the home folder, then every volume; nothing to climb to, the ONI Trash under them.
    assert_eq!(titles(w), ["Home", "T7"]);
    assert!(n.get_at_computer() && !n.get_can_go_up());
    assert_eq!((n.get_pill_title().as_str(), n.get_pill_help().as_str()), ("Computer", "The home folder and every volume"));
    // A folder holding others unfolds on the first click and opens nothing.
    click(&rig, w, "Home");
    rig.until("the home listed", Duration::from_secs(10), || titles(w).contains(&"Scans".to_string()));
    assert_eq!(titles(w), ["Home", "Old", "Other", "Scans", "T7"]);
    assert!(row(w, "Home").unfolded && row(w, "Old").opened && row(w, "Old").kind == 2);
    assert_eq!(opened().as_deref(), Some(p(&home.join("Old")).as_str()), "a first click on a parent opens nothing");
    click(&rig, w, "Scans");
    rig.until("Scans listed", Duration::from_secs(10), || titles(w).contains(&"Roll 1".to_string()));
    assert_eq!(lines(w).iter().map(|l| (l.title.to_string(), l.depth)).collect::<Vec<_>>()[3..7], [
        ("Scans".to_string(), 1),
        ("Empty".to_string(), 2),
        ("Roll 1".to_string(), 2),
        ("Roll 2".to_string(), 2)
    ]);
    // A folder holding none opens on the first click.
    rig.until("Empty listed bare", Duration::from_secs(10), || !row(w, "Empty").chevron && !row(w, "Roll 1").chevron);
    click(&rig, w, "Empty");
    rig.until("Empty opened", Duration::from_secs(30), || opened().as_deref() == Some(p(&home.join("Scans/Empty")).as_str()));
    assert!(row(w, "Empty").picked && !row(w, "Scans").picked);
    // A double click on a parent opens it: the first click folds it, the second opens.
    rest(&rig);
    click(&rig, w, "Scans");
    click(&rig, w, "Scans");
    rig.until("Scans opened", Duration::from_secs(30), || opened().as_deref() == Some(p(&home.join("Scans")).as_str()));
    assert!(!titles(w).contains(&"Roll 1".to_string()) && row(w, "Scans").opened);

    // A volume mounted while the app runs joins Computer's rows.
    std::fs::create_dir_all(volumes.join("T8")).unwrap();
    rig.until("T8 mounted", Duration::from_secs(10), || titles(w).contains(&"T8".to_string()));

    // Favourites: a star keeps a folder in `favourites.json`; the view lists them alone.
    rest(&rig);
    n.invoke_star_clicked(at(w, "Scans") as i32);
    rig.pump();
    click(&rig, w, "T7");
    rig.until("T7 listed", Duration::from_secs(10), || titles(w).contains(&"Film".to_string()));
    n.invoke_star_clicked(at(w, "Film") as i32);
    rig.pump();
    let favourites = oni_library::favourites::Favourites::load(&support).unwrap().paths;
    assert_eq!(favourites, [p(&home.join("Old")), p(&home.join("Scans")), p(&volumes.join("T7/Film"))]);
    assert!(row(w, "Scans").favourite && !row(w, "Other").favourite);
    n.invoke_favourites_toggled();
    rig.pump();
    assert_eq!(titles(w), ["Old", "Scans", "Film"]);
    assert!(n.get_favourites_only() && !n.get_at_computer() && !n.get_no_favourites());
    assert_eq!(n.get_pill_title().as_str(), "Favourites");
    // A favourite on a volume gone fades; mounted again, it is there.
    std::fs::rename(volumes.join("T7"), root.join("T7 away")).unwrap();
    rig.until("T7 gone", Duration::from_secs(10), || !row(w, "Film").present);
    assert!(row(w, "Scans").present, "twin: a favourite on the home stays");
    std::fs::rename(root.join("T7 away"), volumes.join("T7")).unwrap();
    rig.until("T7 back", Duration::from_secs(10), || row(w, "Film").present);
    // Unstarred in the view, a favourite leaves it.
    n.invoke_star_clicked(at(w, "Old") as i32);
    rig.pump();
    assert_eq!(titles(w), ["Scans", "Film"]);
    n.invoke_favourites_toggled();
    rig.pump();

    // A folder opened from elsewhere moves the base to its parent; Parent climbs back to Computer.
    shell.add(&volumes.join("T7/Film"));
    rig.until("Film opened", Duration::from_secs(30), || ctl.state.borrow().read_done() && opened().as_deref() == Some(p(&volumes.join("T7/Film")).as_str()));
    assert!(titles(w).contains(&"Film".to_string()), "Film shows under T7's unfolded row, so the base stays");
    shell.add(&home.join("Scans/Roll 2"));
    rig.until("Roll 2 opened", Duration::from_secs(30), || opened().as_deref() == Some(p(&home.join("Scans/Roll 2")).as_str()));
    assert_eq!(n.get_pill_title().as_str(), "Scans");
    assert_eq!(n.get_pill_help().as_str(), format!("{} — click to jump to a folder above", p(&home.join("Scans"))));
    let ancestors = n.get_ancestors();
    let ancestors: Vec<(String, bool)> = (0..ancestors.row_count()).filter_map(|i| ancestors.row_data(i)).map(|a| (a.title.to_string(), a.current)).collect();
    assert_eq!(ancestors, [("Scans".to_string(), true), ("Home".to_string(), false), ("Computer".to_string(), false)]);
    assert!(row(w, "Roll 2").picked && row(w, "Roll 2").opened);
    n.invoke_go_up();
    rig.pump();
    assert_eq!((n.get_pill_title().as_str(), n.get_can_go_up()), ("Home", true));
    n.invoke_go_up();
    rig.pump();
    assert_eq!((n.get_pill_title().as_str(), n.get_can_go_up()), ("Computer", false), "the home climbs to Computer, never `/`");
    shell.add(&home.join("Scans/Roll 2"));
    rig.until("Roll 2 revealed", Duration::from_secs(10), || n.get_pill_title().as_str() == "Scans");
    n.invoke_ancestor_chosen(1);
    rig.pump();
    assert_eq!(n.get_pill_title().as_str(), "Home");
    rig.until("Home's folders", Duration::from_secs(10), || titles(w).first().map(String::as_str) == Some("Old"));

    // The folder menu, in Swift's order, its rules above Rename… and Show in Finder.
    assert_eq!(menu(w, "Scans"), [
        ("Open".to_string(), true, false),
        ("Remove from Favourites".to_string(), true, false),
        ("Rename…".to_string(), true, true),
        ("Make a Roll".to_string(), true, false),
        ("New Folder Inside".to_string(), true, false),
        ("Show in Finder".to_string(), true, true),
    ]);
    // New Folder Inside, numbered once taken; the new row is named at once.
    choose(&rig, w, "Scans", "New Folder Inside");
    rig.until("the new folder listed", Duration::from_secs(10), || titles(w).contains(&"New Folder".to_string()));
    assert!(home.join("Scans/New Folder").is_dir() && row(w, "New Folder").renaming && row(w, "Scans").unfolded);
    n.invoke_rename_cancelled();
    rig.pump();
    choose(&rig, w, "Scans", "New Folder Inside");
    rig.until("the second new folder", Duration::from_secs(10), || titles(w).contains(&"New Folder 2".to_string()));
    assert!(home.join("Scans/New Folder 2").is_dir(), "twin: a taken name is numbered");
    n.invoke_rename_cancelled();
    rig.pump();
    // Rename… trims the name typed and renames the folder on the disk.
    choose(&rig, w, "New Folder", "Rename…");
    assert!(row(w, "New Folder").renaming);
    n.invoke_rename_committed(at(w, "New Folder") as i32, "  Roll 3 ".into());
    rig.until("Roll 3 listed", Duration::from_secs(10), || titles(w).contains(&"Roll 3".to_string()));
    assert!(home.join("Scans/Roll 3").is_dir() && !home.join("Scans/New Folder").exists());
    // Twin: a name taken is refused, both folders left as they were.
    choose(&rig, w, "New Folder 2", "Rename…");
    n.invoke_rename_committed(at(w, "New Folder 2") as i32, "Roll 3".into());
    rig.pump();
    let advice = w.global::<Advice>();
    assert!(advice.get_shown() && advice.get_title() == "Something Went Wrong");
    assert!(advice.get_message().starts_with("The folder could not be renamed:"), "{}", advice.get_message());
    assert!(home.join("Scans/New Folder 2").is_dir() && home.join("Scans/Roll 3").is_dir());
    advice.invoke_closed();
    // Make a Roll writes the folder's marker, Back to a Folder takes it away.
    choose(&rig, w, "Roll 3", "Make a Roll");
    let roll3 = p(&home.join("Scans/Roll 3"));
    assert_eq!(oni_library::folders::roll(&roll3).map(|f| f.name), Some("Roll 3".to_string()));
    assert_eq!(row(w, "Roll 3").kind, 1);
    assert_eq!(menu(w, "Roll 3")[3].0, "Back to a Folder");
    choose(&rig, w, "Roll 3", "Back to a Folder");
    assert!(oni_library::folders::roll(&roll3).is_none() && row(w, "Roll 3").kind == 0);
    assert_eq!(menu(w, "Roll 3")[3].0, "Make a Roll", "twin: a folder again");
    // A roll renamed carries its name into its marker.
    choose(&rig, w, "Roll 3", "Make a Roll");
    choose(&rig, w, "Roll 3", "Rename…");
    n.invoke_rename_committed(at(w, "Roll 3") as i32, "Roll 4".into());
    rig.until("Roll 4 listed", Duration::from_secs(10), || titles(w).contains(&"Roll 4".to_string()));
    assert_eq!(oni_library::folders::roll(&p(&home.join("Scans/Roll 4"))).map(|f| f.name), Some("Roll 4".to_string()));
    // Renaming a favourite holding the opened folder carries both.
    choose(&rig, w, "Scans", "Rename…");
    n.invoke_rename_committed(at(w, "Scans") as i32, "Scans 2026".into());
    rig.until("Scans 2026 listed", Duration::from_secs(10), || titles(w).contains(&"Scans 2026".to_string()));
    let scans = p(&home.join("Scans 2026"));
    assert!(oni_library::favourites::Favourites::load(&support).unwrap().paths.contains(&scans));
    assert_eq!(opened().as_deref(), Some(paths::join(&scans, "Roll 2").as_str()));
    assert_eq!(Preferences::load(&support).string("openedFolder"), Some(paths::join(&scans, "Roll 2").as_str()));
    rig.until("Roll 2 read again", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().photos() == 1);

    // Beside a photo, the navigator keeps its height over the tree and reveals the photo's folder.
    rig.until("Scans 2026's folders", Duration::from_secs(10), || titles(w).contains(&"Roll 1".to_string()));
    shell.add(&home.join("Scans 2026/Roll 1/A.tif"));
    rig.until("A in the editor", Duration::from_secs(60), || ctl.state.borrow().open_path().is_some());
    assert!(!w.global::<App>().get_gallery());
    rig.pump();
    assert!(row(w, "Roll 1").picked && row(w, "Scans 2026").unfolded);
    assert_eq!(n.get_reveal_row() as usize, at(w, "Roll 1"));
    assert_eq!((n.get_opened_title().as_str(), n.get_height()), ("Roll 1", 200.0));
    n.set_height(320.0);
    n.invoke_height_stored(320.0);
    rig.settle();
    println!("shot {}", rig.shot("navigator-beside-photo").display());
    w.global::<App>().set_gallery(true);
    rig.settle();
    println!("shot {}", rig.shot("navigator-gallery").display());

    // The next launch: the base, the unfolded rows, the favourites and the height where they were.
    let base_before = n.get_pill_title().to_string();
    let rows_before = titles(w);
    drop(shell);
    let shell = launch();
    let w = &shell.window;
    let n = w.global::<Navigator>();
    rig.until("the rows listed again", Duration::from_secs(10), || titles(w) == rows_before);
    assert_eq!((n.get_pill_title().to_string(), n.get_height()), (base_before, 320.0));
    assert!(!n.get_catch_up_shown() && !n.get_favourites_only());
    assert!(row(w, "Scans 2026").favourite);
    let prefs = Preferences::load(&support);
    assert_eq!(prefs.string("navigatorBase"), Some(p(&home).as_str()));
    assert_eq!(prefs.bool("navigatorFavouritesOnly"), Some(false));
    assert_eq!(prefs.value("navigatorEditorHeight").and_then(|v| v.as_f64()), Some(320.0));
    let unfolded: Vec<String> = prefs.value("navigatorUnfolded").and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().filter_map(|v| v.as_str().map(str::to_owned)).collect();
    assert!(unfolded.contains(&scans), "{unfolded:?}");
    // Twin: a row never unfolded is not remembered so.
    assert!(!unfolded.contains(&paths::join(&scans, "Roll 1")));
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
