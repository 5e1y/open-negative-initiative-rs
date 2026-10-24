//! What another app changes shows by itself: a photo dropped in the opened folder, a disk back, the
//! window's return, and Refresh saying what moved. Every wait has a deadline, never a fixed sleep.

use oni_app::app::{Options, Shell};
use oni_app::{Navigator, Told};
use oni_core::{Archive, EditorStore};
use oni_library::navigator::Computer;
use oni_library::paths;
use oni_library::presence::Presence;
use oni_library::watching::{Pace, Watch};
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crate::common;

struct Disk {
    root: PathBuf,
    computer: Computer,
}

/// A home and a volumes folder in the check's own scratch, `files` made under it.
fn disk(label: &str, files: &[&str]) -> Disk {
    let root = std::env::temp_dir().join(format!("oni-app-fresh-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for file in files {
        common::plain_tiff(&root.join(file), 48, 32, 0.5);
    }
    std::fs::create_dir_all(root.join("Home")).unwrap();
    std::fs::create_dir_all(root.join("Volumes")).unwrap();
    let root = PathBuf::from(paths::canonical(&root));
    let computer = Computer { home: paths::canonical(&root.join("Home")), volumes: paths::canonical(&root.join("Volumes")) };
    Disk { root, computer }
}

impl Disk {
    fn launch(&self, gpu: &oni_app::render::Device) -> Shell {
        let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
        let options = Options {
            support: self.root.join("Support"),
            caches: self.root.join("Caches"),
            picker: Box::new(|| None),
            computer: self.computer.clone(),
            ..Options::default()
        };
        let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(gpu), options).unwrap();
        shell.window.show().unwrap();
        shell
    }

    fn path(&self, rel: &str) -> String {
        paths::normal(&self.root.join(rel).to_string_lossy()).into_owned()
    }

    /// A photo written beside the folder, then moved in whole, as the Finder drops one.
    fn drop_in(&self, rel: &str) {
        let staged = self.root.join(format!("staged-{}", paths::name(rel)));
        common::plain_tiff(&staged, 48, 32, 0.4);
        std::fs::rename(staged, self.root.join(rel)).unwrap();
    }
}

fn rig() -> common::Rig {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    rig
}

fn gpu(what: &str) -> Option<oni_app::render::Device> {
    oni_app::render::Device::open().map_err(|e| eprintln!("{e}: {what} is not exercised")).ok()
}

fn names(shell: &Shell) -> Vec<String> {
    let mut out: Vec<String> = shell.ctl.state.borrow().order().iter().map(|p| paths::name(p)).collect();
    out.sort();
    out
}

fn told(shell: &Shell) -> String {
    let t = shell.window.global::<Told>();
    if t.get_shown() { t.get_message().to_string() } else { String::new() }
}

fn opened(rig: &common::Rig, shell: &Shell, folder: &str, photos: usize) {
    shell.open_folder(Path::new(folder));
    let s = &shell.ctl.state;
    rig.until("the folder read", Duration::from_secs(30), || s.borrow().read_done() && s.borrow().photos() == photos);
}

/// A watch of the check's own on `folder`, told when the disk has spoken of it: the app's own watch,
/// were it on that folder, would have heard by then.
fn witness(computer: &Computer, folder: &str) -> (Watch, Arc<AtomicUsize>) {
    let heard = Arc::new(AtomicUsize::new(0));
    let into = heard.clone();
    let watch = Watch::start(computer.clone(), Pace::APP, move |_| {
        into.fetch_add(1, Ordering::Relaxed);
    });
    watch.set(vec![folder.to_owned()], Vec::new());
    (watch, heard)
}

pub(crate) fn a_file_dropped_in_the_opened_folder_appears() {
    let Some(gpu) = gpu("the watch") else { return };
    let d = disk("drop", &["Home/Scans/Roll 8/A.tif", "Home/Scans/Roll 8/B.tif", "Home/Scans/Other/C.tif"]);
    let rig = rig();
    let shell = d.launch(&gpu);
    let ctl = shell.ctl.clone();
    opened(&rig, &shell, &d.path("Home/Scans/Roll 8"), 2);

    // Dropped by another app: it shows with no command, as does one taken away.
    // Dropped again until the watch, started with the folder's read, is live.
    let n = std::cell::Cell::new(0);
    rig.until("a photo dropped shows", Duration::from_secs(30), || {
        if names(&shell).len() < 3 && n.get() % 50 == 0 {
            d.drop_in(&format!("Home/Scans/Roll 8/D{}.tif", n.get() / 50));
        }
        n.set(n.get() + 1);
        names(&shell).len() >= 3 && ctl.state.borrow().read_done()
    });
    rig.settle();
    let reads = ctl.state.borrow().reads();
    std::fs::remove_file(d.root.join("Home/Scans/Roll 8/A.tif")).unwrap();
    let read = || ctl.state.borrow().reads() > reads && ctl.state.borrow().read_done();
    rig.until("a photo taken away leaves", Duration::from_secs(30), || read() && !names(&shell).contains(&"A.tif".to_string()));

    // Twin: a photo dropped in a folder not opened reads nothing, once the disk has spoken of it.
    let reads = ctl.state.borrow().reads();
    let other = d.path("Home/Scans/Other");
    let (_witness, heard) = witness(&d.computer, &other);
    let k = std::cell::Cell::new(0);
    rig.until("the witness heard", Duration::from_secs(30), || {
        if heard.load(Ordering::Relaxed) == 0 && k.get() % 50 == 0 {
            d.drop_in(&format!("Home/Scans/Other/E{}.tif", k.get() / 50));
        }
        k.set(k.get() + 1);
        heard.load(Ordering::Relaxed) > 0
    });
    rig.settle();
    assert_eq!(ctl.state.borrow().reads(), reads, "a folder not opened is not read");
    assert!(!names(&shell).iter().any(|n| n.starts_with('E')));
    drop((ctl, shell));
    let _ = std::fs::remove_dir_all(&d.root);
}

pub(crate) fn a_disk_back_reopens_the_opened_folder() {
    let Some(gpu) = gpu("a disk back") else { return };
    let d = disk("back", &["Volumes/T7/Scans/A.tif", "Volumes/T7/Scans/B.tif"]);
    let rig = rig();
    let shell = d.launch(&gpu);
    let ctl = shell.ctl.clone();
    // The volumes' reader alone: the watch would also see the disk go.
    ctl.set_watching(false);
    let scans = d.path("Volumes/T7/Scans");
    opened(&rig, &shell, &scans, 2);
    let state = || ctl.state.borrow().opened_presence();

    // Unplugged in the session, a photo added elsewhere meanwhile, then plugged back in.
    std::fs::rename(d.root.join("Volumes/T7"), d.root.join("T7 away")).unwrap();
    rig.until("the disk gone", Duration::from_secs(20), || !state().is_present());
    let reads = ctl.state.borrow().reads();
    common::plain_tiff(&d.root.join("T7 away/Scans/C.tif"), 48, 32, 0.4);
    std::fs::rename(d.root.join("T7 away"), d.root.join("Volumes/T7")).unwrap();
    rig.until("Scans read again", Duration::from_secs(30), || state() == Presence::Present && ctl.state.borrow().read_done() && names(&shell).len() == 3);
    assert_eq!(names(&shell), ["A.tif", "B.tif", "C.tif"]);
    assert_eq!(ctl.state.borrow().reads(), reads + 1, "read again once on its return");

    // Twin: another disk arriving reads the opened folder never.
    std::fs::create_dir_all(d.root.join("Volumes/T8")).unwrap();
    let n = shell.window.global::<Navigator>();
    let titles = || (0..n.get_rows().row_count()).filter_map(|i| n.get_rows().row_data(i)).map(|r| r.title.to_string()).collect::<Vec<_>>();
    rig.until("T8 mounted", Duration::from_secs(20), || titles().contains(&"T8".to_string()));
    rig.settle();
    assert_eq!(ctl.state.borrow().reads(), reads + 1, "a folder never gone is not read again");
    drop((ctl, shell));
    let _ = std::fs::remove_dir_all(&d.root);
}

pub(crate) fn focus_rereads_the_opened_folder() {
    let Some(gpu) = gpu("the window's return") else { return };
    let d = disk("focus", &["Home/Scans/Roll 8/A.tif", "Home/Scans/Roll 8/B.tif"]);
    let rig = rig();
    let shell = d.launch(&gpu);
    let ctl = shell.ctl.clone();
    // The return alone, as on a network disk whose changes no event tells.
    ctl.set_watching(false);
    opened(&rig, &shell, &d.path("Home/Scans/Roll 8"), 2);

    // Nothing moved behind the window: its return reads nothing again.
    let away_and_back = || {
        ctl.focus_changed(false);
        rig.until("the listing kept", Duration::from_secs(10), || ctl.state.borrow().listing_kept());
    };
    away_and_back();
    let (reads, returns) = (ctl.state.borrow().reads(), ctl.state.borrow().returns());
    ctl.focus_changed(true);
    rig.until("the return looked", Duration::from_secs(10), || ctl.state.borrow().returns() == returns + 1);
    rig.settle();
    assert_eq!(ctl.state.borrow().reads(), reads, "twin: nothing moved, nothing read");

    // A photo dropped while the window was behind shows on its return.
    away_and_back();
    d.drop_in("Home/Scans/Roll 8/C.tif");
    ctl.focus_changed(true);
    rig.until("C shows", Duration::from_secs(30), || names(&shell).len() == 3 && ctl.state.borrow().read_done());
    assert_eq!(ctl.state.borrow().reads(), reads + 1);
    drop((ctl, shell));
    let _ = std::fs::remove_dir_all(&d.root);
}

pub(crate) fn refresh_says_what_changed() {
    let Some(gpu) = gpu("Refresh") else { return };
    let d = disk("refresh", &["Home/Scans/Roll 8/A.tif", "Home/Scans/Roll 8/B.tif"]);
    let rig = rig();
    let shell = d.launch(&gpu);
    let ctl = shell.ctl.clone();
    ctl.set_watching(false);
    opened(&rig, &shell, &d.path("Home/Scans/Roll 8"), 2);
    let menus = shell.window.global::<oni_app::Menus>();
    assert!(menus.get_live().refresh);

    // Twin: nothing moved says so.
    ctl.act(oni_app::Act::Refresh);
    rig.until("Refresh said", Duration::from_secs(30), || !told(&shell).is_empty());
    assert_eq!(told(&shell), "Nothing changed.");
    assert_eq!(shell.window.global::<Told>().get_title(), "Refresh");
    shell.window.global::<Told>().invoke_closed();

    // Two photos added and one taken away, said in counts.
    d.drop_in("Home/Scans/Roll 8/C.tif");
    d.drop_in("Home/Scans/Roll 8/D.tif");
    std::fs::remove_file(d.root.join("Home/Scans/Roll 8/A.tif")).unwrap();
    ctl.act(oni_app::Act::Refresh);
    rig.until("Refresh said again", Duration::from_secs(30), || !told(&shell).is_empty());
    assert_eq!(told(&shell), "2 photos added. 1 photo no longer in the folder.");
    assert_eq!(names(&shell), ["B.tif", "C.tif", "D.tif"]);
    drop((ctl, shell));
    let _ = std::fs::remove_dir_all(&d.root);
}

fn row_titles(shell: &Shell) -> Vec<String> {
    let rows = shell.window.global::<Navigator>().get_rows();
    (0..rows.row_count()).filter_map(|i| rows.row_data(i)).map(|r| r.title.to_string()).collect()
}

fn rename(rig: &common::Rig, shell: &Shell, from: &str, to: &str) {
    let n = shell.window.global::<Navigator>();
    let at = |title: &str| row_titles(shell).iter().position(|t| t == title).unwrap_or_else(|| panic!("{title} not among {:?}", row_titles(shell)));
    assert!(n.invoke_row_menu(at(from) as i32));
    let lines = n.get_menu_lines();
    let item = (0..lines.row_count()).filter_map(|i| lines.row_data(i)).position(|l| l.text == "Rename…").expect("Rename… in the menu");
    n.invoke_menu_chosen(item as i32);
    rig.pump();
    n.invoke_rename_committed(at(from) as i32, to.into());
    rig.pump();
}

/// The folders in `folder` as the disk spells them.
fn on_disk(folder: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(folder).unwrap().flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    out.sort();
    out
}

pub(crate) fn a_rename_changing_only_case_renames_and_a_true_collision_is_refused() {
    let Some(gpu) = gpu("a rename") else { return };
    let d = disk("case", &["Home/Scans/roll 3/A.tif", "Home/Scans/Roll 5/B.tif"]);
    let rig = rig();
    let shell = d.launch(&gpu);
    shell.add(&d.root.join("Home/Scans/roll 3"));
    rig.until("Scans listed", Duration::from_secs(30), || row_titles(&shell).contains(&"roll 3".to_string()) && row_titles(&shell).contains(&"Roll 5".to_string()));

    // A letter's case alone: one folder, renamed, on a disk that ignores case too.
    rename(&rig, &shell, "roll 3", "Roll 3");
    rig.until("Roll 3 listed", Duration::from_secs(10), || row_titles(&shell).contains(&"Roll 3".to_string()));
    assert_eq!(on_disk(&d.root.join("Home/Scans")), ["Roll 3", "Roll 5"]);
    assert_eq!(told(&shell), "", "a case-only rename is no collision");

    // Twin: another folder's name is refused, both left as they were.
    rename(&rig, &shell, "Roll 3", "Roll 5");
    rig.until("the refusal said", Duration::from_secs(10), || !told(&shell).is_empty());
    assert_eq!(told(&shell), "The folder could not be renamed: an item of that name is already there");
    assert_eq!(on_disk(&d.root.join("Home/Scans")), ["Roll 3", "Roll 5"]);
    drop(shell);
    let _ = std::fs::remove_dir_all(&d.root);
}
