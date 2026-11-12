//! A gesture's writes or moves own the disk until they land: no read of the opened folder starts
//! meanwhile, the folder menu's writes and a mark's rows read back run off the window.

use crate::freshness::{disk, gpu, names, on_disk, opened, rig, row_titles, told};
use oni_app::{Act, Library, Navigator, Told};
use oni_library::favourites::Favourites;
use oni_library::index::LibraryIndex;
use oni_library::paths;
use slint::{ComponentHandle, Model};
use std::path::Path;
use std::time::{Duration, Instant};

/// The line `text` of the folder menu on the navigator's row `title`, chosen.
fn chosen(shell: &oni_app::app::Shell, title: &str, text: &str) {
    let n = shell.window.global::<Navigator>();
    let at = row_titles(shell).iter().position(|t| t == title).unwrap_or_else(|| panic!("{title} not among {:?}", row_titles(shell)));
    assert!(n.invoke_row_menu(at as i32));
    let lines = n.get_menu_lines();
    let item = (0..lines.row_count()).filter_map(|i| lines.row_data(i)).position(|l| l.text == text).unwrap_or_else(|| panic!("{text} in the menu"));
    n.invoke_menu_chosen(item as i32);
}

pub(crate) fn a_read_asked_while_a_gesture_writes_waits_for_it() {
    let Some(gpu) = gpu("a read during a gesture") else { return };
    let d = disk("turns", &["Home/Scans/Roll 8/A.tif", "Home/Scans/Roll 8/B.tif", "Home/Scans/Roll 8/C.tif"]);
    let rig = rig();
    let shell = d.launch(&gpu);
    let ctl = shell.ctl.clone();
    ctl.set_watching(false);
    opened(&rig, &shell, &d.path("Home/Scans/Roll 8"), 3);
    rig.settle();
    let counts = || (ctl.state.borrow().reads(), ctl.state.borrow().reads_whole());
    let lib = shell.window.global::<Library>();
    let closed = || shell.window.global::<Told>().invoke_closed();

    // Twin: with no gesture under way, Refresh starts its read at once.
    let before = counts();
    ctl.act(Act::Refresh);
    assert_eq!(counts().0, before.0 + 1);
    rig.until("Refresh said", Duration::from_secs(30), || !told(&shell).is_empty());
    closed();

    // Asked while a mark runs, the read waits for it to land, then exactly one runs.
    lib.invoke_tile_clicked(0, false, false);
    rig.pump();
    ctl.act(Act::Remove);
    assert!(ctl.state.borrow().disk_busy(), "the mark runs");
    let before = counts();
    ctl.act(Act::Refresh);
    assert_eq!(counts().0, before.0, "no read starts while the gesture's writes run");
    rig.landed(&ctl);
    rig.until("the read owed", Duration::from_secs(30), || counts().0 == before.0 + 1 && ctl.state.borrow().read_done());
    rig.settle();
    assert_eq!(counts().0, before.0 + 1, "one read follows the gesture");
    assert_eq!(names(&shell).len(), 2);
    closed();

    // A read under way when a mark begins stops and never lands; one more reads the folder after it.
    lib.invoke_tile_clicked(0, false, false);
    rig.pump();
    let before = counts();
    ctl.act(Act::Refresh);
    assert!(!ctl.state.borrow().read_done(), "the read runs");
    ctl.act(Act::Remove);
    rig.landed(&ctl);
    rig.until("the read again", Duration::from_secs(30), || counts().0 == before.0 + 2 && ctl.state.borrow().read_done());
    rig.settle();
    assert_eq!(counts().1, before.1 + 1, "the read the mark stopped never lands");
    assert_eq!(names(&shell).len(), 1);
    drop((ctl, shell));
    let _ = std::fs::remove_dir_all(&d.root);
}

pub(crate) fn the_folder_menus_writes_run_off_the_window() {
    let Some(gpu) = gpu("the folder menu's writes") else { return };
    let d = disk("menu", &["Home/Scans/Roll 3/A.tif"]);
    let rig = rig();
    let shell = d.launch(&gpu);
    let ctl = shell.ctl.clone();
    shell.add(&d.root.join("Home/Scans/Roll 3"));
    rig.until("Roll 3 read", Duration::from_secs(30), || ctl.state.borrow().read_done() && row_titles(&shell).contains(&"Roll 3".to_string()));
    rig.settle();
    let busy = || ctl.state.borrow().disk_busy();
    let at = |title: &str| row_titles(&shell).iter().position(|t| t == title).unwrap() as i32;
    let n = shell.window.global::<Navigator>();

    // Twin: a name refused is said, and nothing runs.
    chosen(&shell, "Roll 3", "Rename…");
    rig.pump();
    n.invoke_rename_committed(at("Roll 3"), "a/b".into());
    assert!(!busy() && !told(&shell).is_empty(), "{}", told(&shell));
    shell.window.global::<Told>().invoke_closed();

    // Renamed off the window: under way once committed, the folder renamed once it lands.
    chosen(&shell, "Roll 3", "Rename…");
    rig.pump();
    n.invoke_rename_committed(at("Roll 3"), "Portra".into());
    assert!(busy(), "the rename runs off the window");
    rig.landed(&ctl);
    assert_eq!(on_disk(&d.root.join("Home/Scans")), ["Portra"]);
    rig.until("Portra listed", Duration::from_secs(10), || row_titles(&shell).contains(&"Portra".to_string()));
    let portra = d.path("Home/Scans/Portra");

    // The roll marker written off the window.
    chosen(&shell, "Portra", "Make a Roll");
    assert!(busy(), "the roll marker is written off the window");
    rig.landed(&ctl);
    assert!(oni_library::folders::roll(&portra).is_some());

    // The new folder made off the window.
    chosen(&shell, "Portra", "New Folder Inside");
    assert!(busy(), "the folder is made off the window");
    rig.landed(&ctl);
    assert_eq!(on_disk(Path::new(&portra)), ["New Folder"]);
    assert_eq!(told(&shell), "");
    drop((ctl, shell));
    let _ = std::fs::remove_dir_all(&d.root);
}

pub(crate) fn a_marks_rows_are_read_back_off_the_window() {
    let Some(gpu) = gpu("a mark's rows read back") else { return };
    let d = disk("reread", &["Home/Scans/Roll 8/A.tif", "Home/Scans/Roll 8/B.tif"]);
    let rig = rig();
    let shell = d.launch(&gpu);
    let ctl = shell.ctl.clone();
    ctl.set_watching(false);
    opened(&rig, &shell, &d.path("Home/Scans/Roll 8"), 2);
    rig.settle();
    let index = LibraryIndex::open(&LibraryIndex::default_path(&d.root.join("Caches"))).unwrap();
    shell.window.global::<Library>().invoke_tile_clicked(0, false, false);
    rig.pump();
    let photo = ctl.state.borrow().order()[0].clone();
    assert!(index.row(&photo).unwrap().trashed_at.is_none());

    // The window left alone, never pumped: the gesture's own thread reads the row back into the index.
    ctl.act(Act::Remove);
    let start = Instant::now();
    while index.row(&photo).unwrap().trashed_at.is_none() {
        assert!(start.elapsed() < Duration::from_secs(30), "the mark's row is read back on the window's turn only");
        std::thread::sleep(Duration::from_millis(20));
    }
    // Twin: the landing, the window's own part, has not run meanwhile.
    assert!(ctl.state.borrow().disk_busy());
    rig.landed(&ctl);
    assert_eq!(names(&shell).len(), 1);
    drop((ctl, shell));
    let _ = std::fs::remove_dir_all(&d.root);
}

/// A favourite kept in another spelling takes the archive's off the window, and its row opens there.
pub(crate) fn a_favourite_in_another_spelling_opens_in_the_archives() {
    let Some(gpu) = gpu("a favourite's spelling") else { return };
    if !cfg!(target_os = "macos") {
        return;
    }
    let d = disk("spelt", &["Home/Scans/Roll 8/A.tif"]);
    let roll = d.path("Home/Scans/Roll 8");
    // The scratch lies under `/var`, which the disk resolves through `/private`.
    let other = format!("/private{roll}");
    // Twin: that spelling is not the archive's, so a check comparing spellings can tell them apart.
    assert_ne!(paths::canonical(Path::new(&other)), other);
    let support = d.root.join("Support");
    Favourites { version: 1, paths: vec![other.clone()] }.save(&support).unwrap();
    let rig = rig();
    let shell = d.launch(&gpu);
    let ctl = shell.ctl.clone();
    let kept = || Favourites::load(&support).map(|f| f.paths).unwrap_or_default();
    rig.until("the favourite respelt", Duration::from_secs(30), || kept() == [roll.clone()]);
    let at = row_titles(&shell).iter().position(|t| t == "Roll 8").expect("the favourite's row");
    shell.window.global::<Navigator>().invoke_row_clicked(at as i32);
    rig.until("the favourite read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().photos() == 1);
    assert_eq!(ctl.state.borrow().opened(), Some(roll.as_str()));
    drop((ctl, shell));
    let _ = std::fs::remove_dir_all(&d.root);
}
