//! Locate… follows a folder gone once its photos are recognised, Remove from Library touches no file,
//! and a disk remounted under another name is offered, never followed alone. Every disk is the check's.

use oni_app::app::{Options, Shell};
use oni_app::{Confirm, Library, Told};
use oni_core::{Archive, EditorStore};
use oni_library::bookmarks::{self, Bookmarks, Place, Volume};
use oni_library::favourites::Favourites;
use oni_library::index::LibraryIndex;
use oni_library::navigator::Computer;
use oni_library::{paths, project, relink};
use slint::{ComponentHandle, Model};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use crate::common;

struct Disk {
    root: PathBuf,
    support: PathBuf,
    computer: Computer,
}

/// A home and a volumes folder in the check's own scratch, a still TIFF of its own tone at each file.
fn disk(label: &str, files: &[&str]) -> Disk {
    let root = std::env::temp_dir().join(format!("oni-app-locate-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for (i, file) in files.iter().enumerate() {
        common::linear_tiff(&root.join(file), 48, 32, 0.2 + 0.1 * i as f32);
    }
    std::fs::create_dir_all(root.join("Home")).unwrap();
    std::fs::create_dir_all(root.join("Volumes")).unwrap();
    let root = PathBuf::from(paths::canonical(&root));
    let computer = Computer { home: paths::canonical(&root.join("Home")), volumes: paths::canonical(&root.join("Volumes")) };
    let support = root.join("Support");
    std::fs::create_dir_all(&support).unwrap();
    Disk { root, support, computer }
}

impl Disk {
    fn launch(&self, gpu: &oni_app::render::Device, picked: Rc<RefCell<Option<PathBuf>>>, identify: bookmarks::Identify) -> Shell {
        let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
        let options = Options {
            support: self.support.clone(),
            caches: self.root.join("Caches"),
            picker: Box::new(move || picked.borrow().clone()),
            computer: self.computer.clone(),
            identify,
            ..Options::default()
        };
        let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(gpu), options).unwrap();
        shell.window.show().unwrap();
        shell
    }

    fn path(&self, rel: &str) -> String {
        paths::normal(&self.root.join(rel).to_string_lossy()).into_owned()
    }

    fn index(&self) -> LibraryIndex {
        LibraryIndex::open(&self.root.join("Caches/index.sqlite")).unwrap()
    }

    fn favourites(&self) -> Vec<String> {
        Favourites::load(&self.support).map(|f| f.paths).unwrap_or_default()
    }
}

fn gpu(what: &str) -> Option<oni_app::render::Device> {
    oni_app::render::Device::open().map_err(|e| eprintln!("{e}: {what} is not exercised")).ok()
}

fn rig() -> common::Rig {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    rig
}

fn opened(rig: &common::Rig, shell: &Shell, folder: &str) {
    shell.open_folder(Path::new(folder));
    let ctl = shell.ctl.clone();
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().opened() == Some(folder) && ctl.state.borrow().read_done());
}

/// The banner: its title, its words and its keys.
fn banner(shell: &Shell) -> Option<(String, String, Vec<String>)> {
    let l = shell.window.global::<Library>();
    let keys = l.get_banner_keys();
    l.get_banner_shown().then(|| (l.get_banner_title().to_string(), l.get_banner_message().to_string(), (0..keys.row_count()).filter_map(|i| keys.row_data(i)).map(|k| k.to_string()).collect()))
}

fn press(rig: &common::Rig, shell: &Shell, key: &str) {
    let keys = banner(shell).map(|b| b.2).unwrap_or_default();
    let i = keys.iter().position(|k| k == key).unwrap_or_else(|| panic!("{key} in {keys:?}"));
    shell.window.global::<Library>().invoke_banner_acted(i as i32);
    rig.pump();
}

fn confirm(shell: &Shell) -> Option<(String, String)> {
    let c = shell.window.global::<Confirm>();
    c.get_shown().then(|| (c.get_title().to_string(), c.get_message().to_string()))
}

fn told(shell: &Shell) -> Option<String> {
    let t = shell.window.global::<Told>();
    t.get_shown().then(|| t.get_message().to_string())
}

fn print(path: &str) -> String {
    relink::fingerprint(Path::new(path)).unwrap()
}

/// Every file under `root` but the app's own support and caches, each with its bytes.
fn listing(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).unwrap().flatten() {
            let path = entry.path();
            if folder == root && ["Support", "Caches"].iter().any(|n| entry.file_name() == *n) {
                continue;
            }
            if path.is_dir() {
                folders.push(path);
            } else {
                out.insert(path.to_string_lossy().into_owned(), std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

pub(crate) fn locate_follows_favourite_rows_and_projects() {
    let Some(gpu) = gpu("Locate…") else { return };
    let d = disk("follow", &["Home/Scans/2026-03 Lisbon/A.tif", "Home/Scans/2026-03 Lisbon/Sub/B.tif", "Home/Other/C.tif"]);
    let (old, new) = (d.path("Home/Scans/Roll 12"), d.path("Home/Scans/2026-03 Lisbon"));
    let (old_a, old_b) = (paths::join(&old, "A.tif"), paths::join(&old, "Sub/B.tif"));
    let (new_a, new_b) = (paths::join(&new, "A.tif"), paths::join(&new, "Sub/B.tif"));
    // Renamed in the Finder while ONI was closed: its rows, its favourite, its project and its fold kept the old name.
    let (print_a, print_b) = (print(&new_a), print(&new_b));
    let index = d.index();
    for (path, print) in [(&old_a, &print_a), (&old_b, &print_b)] {
        index.upsert(&oni_library::Entry::new(path, oni_settings::Date::now(), print)).unwrap();
    }
    Favourites { version: 1, paths: vec![old.clone()] }.save(&d.support).unwrap();
    let mut held = project::Project::new("Lisbon", oni_settings::Date::now(), vec![print_a.clone()]);
    held.paths = Some([(print_a.clone(), old_a.clone())].into());
    project::save(&d.support, &project::ProjectsFile { version: 1, projects: vec![held] }).unwrap();
    let mut preferences = oni_app::preferences::Preferences::load(&d.support);
    preferences.set("navigatorUnfolded", serde_json::json!([old.clone()])).unwrap();

    let picked = Rc::new(RefCell::new(None));
    let rig = rig();
    let shell = d.launch(&gpu, picked.clone(), bookmarks::system());
    let ctl = shell.ctl.clone();
    opened(&rig, &shell, &old);
    rig.until("its banner", Duration::from_secs(10), || banner(&shell).is_some_and(|b| b.0 == "Folder Not Found"));
    assert_eq!(banner(&shell).unwrap().2, ["LOCATE…", "REMOVE FROM FAVOURITES", "REMOVE FROM LIBRARY…"]);

    // Twin first: a folder sharing no photo asks before anything follows, and Cancel leaves all as it was.
    *picked.borrow_mut() = Some(d.root.join("Home/Other"));
    press(&rig, &shell, "LOCATE…");
    rig.until("the sheet", Duration::from_secs(10), || confirm(&shell).is_some());
    let (title, message) = confirm(&shell).unwrap();
    assert_eq!(title, "No Photo Recognised");
    assert!(message.starts_with("None of the 2 photos ONI knew in “Roll 12” is in “Other”."), "{message}");
    shell.window.global::<Confirm>().invoke_answered(false);
    rig.pump();
    assert_eq!(d.favourites(), [old.clone()]);
    assert_eq!(d.index().under(&old).len(), 2);
    assert_eq!(ctl.state.borrow().opened(), Some(old.as_str()));

    *picked.borrow_mut() = Some(d.root.join("Home/Scans/2026-03 Lisbon"));
    press(&rig, &shell, "LOCATE…");
    rig.until("followed", Duration::from_secs(10), || ctl.state.borrow().opened() == Some(new.as_str()));
    assert!(confirm(&shell).is_none(), "photos recognised follow at once");
    assert_eq!(told(&shell).as_deref(), Some("2 photos of 2 recognised in “2026-03 Lisbon”: “Roll 12” follows it."));
    assert_eq!(d.favourites(), [new.clone()]);
    let rows: Vec<(String, String)> = d.index().under(&d.path("Home/Scans")).into_iter().map(|e| (e.path, e.fingerprint)).collect();
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert!(rows.contains(&(new_a.clone(), print_a.clone())) && rows.contains(&(new_b.clone(), print_b.clone())), "{rows:?}");
    let recorded = project::load(&d.support).projects[0].paths.clone().unwrap_or_default();
    assert_eq!(recorded.get(&print_a), Some(&new_a), "the project's recorded path follows");
    let unfolded = oni_app::preferences::Preferences::load(&d.support).value("navigatorUnfolded").cloned();
    assert_eq!(unfolded, Some(serde_json::json!([new.clone()])), "the fold follows");
    rig.until("the folder there", Duration::from_secs(20), || ctl.state.borrow().read_done() && banner(&shell).is_none());
    rig.settle();
    rig.shot("locate-followed");
    drop((ctl, shell));
    let _ = std::fs::remove_dir_all(&d.root);
}

pub(crate) fn forget_touches_no_file() {
    let Some(gpu) = gpu("Remove from Library…") else { return };
    let d = disk("forget", &["Home/Scans/Roll 12/A.tif", "Home/Scans/Roll 12/B.tif", "Volumes/T7/Roll 9/D.tif"]);
    let (roll, copy, away) = (d.path("Home/Scans/Roll 12"), d.path("Home/Copy"), d.path("Volumes/T7/Roll 9"));
    oni_settings::sidecar::set_label(oni_settings::Medal::Gold, &d.root.join("Home/Scans/Roll 12/A.tif"), false).unwrap();
    std::fs::create_dir_all(d.root.join("Home/Copy")).unwrap();
    std::fs::copy(d.root.join("Home/Scans/Roll 12/A.tif"), d.root.join("Home/Copy/A copy.tif")).unwrap();
    Favourites { version: 1, paths: vec![roll.clone(), away.clone()] }.save(&d.support).unwrap();
    let rig = rig();
    let shell = d.launch(&gpu, Rc::new(RefCell::new(None)), bookmarks::system());
    let ctl = shell.ctl.clone();
    opened(&rig, &shell, &copy);
    opened(&rig, &shell, &away);
    opened(&rig, &shell, &roll);
    rig.until("Roll 12 drawn", Duration::from_secs(60), || ctl.state.borrow().loaded() >= 2);
    let (print_a, print_b) = (print(&paths::join(&roll, "A.tif")), print(&paths::join(&roll, "B.tif")));
    let thumbnail = |p: &str| oni_app::thumbnails::file(&d.support, p);
    rig.until("both thumbnails kept", Duration::from_secs(30), || thumbnail(&print_a).exists() && thumbnail(&print_b).exists());

    // Deleted in the Finder, as far as ONI can tell: moved out of every folder it knows.
    std::fs::create_dir_all(d.root.join("Elsewhere")).unwrap();
    std::fs::rename(d.root.join("Home/Scans/Roll 12"), d.root.join("Elsewhere/Roll 12")).unwrap();
    opened(&rig, &shell, &roll);
    rig.until("its banner", Duration::from_secs(10), || banner(&shell).is_some_and(|b| b.0 == "Folder Not Found"));
    // Its favourite's file id finds it where it went, offered beside Locate and the two that forget.
    rig.until("the lead", Duration::from_secs(10), || banner(&shell).is_some_and(|b| b.2.len() == 4));
    let (_, message, keys) = banner(&shell).unwrap();
    assert_eq!(keys, ["FOLLOW", "LOCATE…", "REMOVE FROM FAVOURITES", "REMOVE FROM LIBRARY…"]);
    assert!(message.ends_with("It seems to be in “Elsewhere” now."), "{message}");
    let before = listing(&d.root);
    press(&rig, &shell, "REMOVE FROM LIBRARY…");
    let (title, message) = confirm(&shell).expect("the sheet asks first");
    assert_eq!(title, "Remove “Roll 12” from the Library?");
    assert_eq!(message, format!("ONI forgets the 2 photos it knows in “{roll}”. No file is touched: if the folder comes back, its photos return with their settings."));
    assert_eq!(d.index().under(&roll).len(), 2, "nothing forgotten before the key");
    shell.window.global::<Confirm>().invoke_answered(true);
    rig.pump();
    rig.until("the rows forgotten", Duration::from_secs(10), || d.index().under(&roll).is_empty());
    rig.until("B's thumbnail gone", Duration::from_secs(10), || !thumbnail(&print_b).exists());
    assert!(thumbnail(&print_a).exists(), "the copy elsewhere still claims A's thumbnail");
    assert_eq!(d.index().under(&copy).len(), 1, "the copy's row stays");
    assert_eq!(d.favourites(), [away.clone()], "the favourite gone, the one away kept");
    rig.until("the banner without the key", Duration::from_secs(10), || banner(&shell).is_some_and(|b| b.2 == ["LOCATE…"]));
    let after = listing(&d.root);
    assert_eq!(before.keys().collect::<Vec<_>>(), after.keys().collect::<Vec<_>>());
    assert!(before == after, "every file byte for byte as it was");
    assert!(after.keys().any(|k| k.ends_with("Elsewhere/Roll 12/A.tif.json")), "the sidecar is in the listing");
    // Twin: the listing sees a sidecar touched.
    std::fs::write(d.root.join("Elsewhere/Roll 12/A.tif.json"), b"{}").unwrap();
    assert!(listing(&d.root) != before, "a touched sidecar is seen");

    // A disk away is kept forever: its banner offers no Remove from Library, and its rows stay.
    std::fs::rename(d.root.join("Volumes/T7"), d.root.join("T7 away")).unwrap();
    opened(&rig, &shell, &away);
    rig.until("T7 away", Duration::from_secs(10), || banner(&shell).is_some_and(|b| b.0 == "T7 Not Connected"));
    assert!(!banner(&shell).unwrap().2.iter().any(|k| k.starts_with("REMOVE FROM LIBRARY")));
    assert_eq!(d.index().under(&away).len(), 1);
    drop((ctl, shell));
    let _ = std::fs::remove_dir_all(&d.root);
}

/// Each volume of `computer` named by the UUID `uuids` gives its folder's name, as `getattrlist` would.
fn fake(computer: &Computer, uuids: &'static [(&'static str, &'static str)]) -> bookmarks::Identify {
    let computer = computer.clone();
    Arc::new(move |path: &str| {
        let root = oni_library::absence::volume_root(&computer, path)?;
        let uuid = uuids.iter().find(|(n, _)| *n == paths::name(&root)).map(|(_, u)| u.to_string());
        Some(Volume { uuid, mount: root })
    })
}

/// The T7 known by its UUID, its favourite on it, then the disk remounted as "T7 1".
fn remounted(label: &str) -> (Disk, String) {
    let d = disk(label, &["Volumes/T7/Scans/A.tif", "Volumes/T7/Scans/B.tif"]);
    let scans = d.path("Volumes/T7/Scans");
    Favourites { version: 1, paths: vec![scans.clone()] }.save(&d.support).unwrap();
    let place = Place { volume: Some(Volume { uuid: Some("T7-UUID".into()), mount: d.path("Volumes/T7") }), file_id: None };
    Bookmarks { version: 1, places: [(scans.clone(), place)].into() }.save(&d.support).unwrap();
    (d, scans)
}

pub(crate) fn a_t7_remounted_as_t7_1_is_offered_to_follow() {
    let Some(gpu) = gpu("a disk remounted") else { return };
    let rig = rig();
    // Twin first: another disk mounted as "T7 1" is offered nothing.
    {
        let (d, scans) = remounted("other");
        let shell = d.launch(&gpu, Rc::new(RefCell::new(None)), fake(&d.computer, &[("T7 1", "ANOTHER")]));
        opened(&rig, &shell, &scans);
        std::fs::rename(d.root.join("Volumes/T7"), d.root.join("Volumes/T7 1")).unwrap();
        rig.until("T7 away", Duration::from_secs(10), || banner(&shell).is_some_and(|b| b.0 == "T7 Not Connected"));
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(2) {
            rig.pump();
        }
        let (_, message, keys) = banner(&shell).unwrap();
        assert!(keys.is_empty() && !message.contains("seems"), "{message} {keys:?}");
        drop(shell);
        let _ = std::fs::remove_dir_all(&d.root);
    }

    let (d, scans) = remounted("same");
    let shell = d.launch(&gpu, Rc::new(RefCell::new(None)), fake(&d.computer, &[("T7", "T7-UUID"), ("T7 1", "T7-UUID")]));
    let ctl = shell.ctl.clone();
    opened(&rig, &shell, &scans);
    rig.until("Scans drawn", Duration::from_secs(60), || ctl.state.borrow().loaded() >= 2);
    assert!(banner(&shell).is_none());
    std::fs::rename(d.root.join("Volumes/T7"), d.root.join("Volumes/T7 1")).unwrap();
    rig.until("the lead offered", Duration::from_secs(10), || banner(&shell).is_some_and(|b| b.2 == ["FOLLOW"]));
    let (title, message, _) = banner(&shell).unwrap();
    assert_eq!(title, "T7 Not Connected");
    assert!(message.ends_with("“T7” seems to have become “T7 1”."), "{message}");
    // Offered, never followed by itself: nothing moved before the key.
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(1) {
        rig.pump();
    }
    assert_eq!(d.favourites(), [scans.clone()]);
    assert_eq!(ctl.state.borrow().opened(), Some(scans.as_str()));
    rig.settle();
    rig.shot("locate-t7-offered");

    press(&rig, &shell, "FOLLOW");
    let moved = d.path("Volumes/T7 1/Scans");
    rig.until("followed", Duration::from_secs(10), || ctl.state.borrow().opened() == Some(moved.as_str()));
    assert_eq!(told(&shell).as_deref(), Some("2 photos of 2 recognised in “T7 1”: “T7” follows it."));
    assert_eq!(d.favourites(), [moved.clone()]);
    assert_eq!(d.index().under(&moved).len(), 2);
    assert!(d.index().under(&scans).is_empty());
    rig.until("the folder there", Duration::from_secs(20), || ctl.state.borrow().read_done() && banner(&shell).is_none());
    drop((ctl, shell));
    let _ = std::fs::remove_dir_all(&d.root);
}
