//! What is there, told off the interface thread: a lay reads only what was published, a remembered
//! folder on a disk away is kept and read again on its return, a path mistyped opens nothing. The
//! disks are folders under a volumes folder of the check's own, never `/Volumes`.

use oni_app::app::{self, Options, Shell};
use oni_app::preferences::Preferences;
use oni_app::{Canvas, Library, Navigator};
use oni_core::{Archive, EditorStore};
use oni_library::navigator::Computer;
use oni_library::paths;
use oni_library::presence::{self, Away, Presence};
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crate::common;

struct Disk {
    root: PathBuf,
    support: PathBuf,
    caches: PathBuf,
    computer: Computer,
}

/// A home and a volumes folder in the check's own scratch, `files` made under it.
fn disk(label: &str, files: &[&str]) -> Disk {
    let root = std::env::temp_dir().join(format!("oni-app-presence-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for file in files {
        common::plain_tiff(&root.join(file), 48, 32, 0.5);
    }
    std::fs::create_dir_all(root.join("Home")).unwrap();
    std::fs::create_dir_all(root.join("Volumes")).unwrap();
    let root = PathBuf::from(paths::canonical(&root));
    let computer = Computer { home: paths::canonical(&root.join("Home")), volumes: paths::canonical(&root.join("Volumes")) };
    Disk { support: root.join("Support"), caches: root.join("Caches"), root, computer }
}

impl Disk {
    fn launch(&self, gpu: &oni_app::render::Device) -> Shell {
        let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
        let options = Options {
            support: self.support.clone(),
            caches: self.caches.clone(),
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

fn missing(shell: &Shell, caption: &str) -> bool {
    app::tiles(&shell.window).into_iter().find(|t| t.caption == caption).unwrap_or_else(|| panic!("{caption} is shown")).missing
}

fn status(shell: &Shell) -> String {
    shell.window.global::<Canvas>().get_status().to_string()
}

pub(crate) fn lay_grid_touches_no_disk() {
    let Some(gpu) = gpu("the lay") else { return };
    let d = disk("lay", &["Home/Scans/A.tif", "Home/Scans/B.tif"]);
    let rig = rig();
    let shell = d.launch(&gpu);
    let ctl = shell.ctl.clone();
    shell.open_folder(&d.root.join("Home/Scans"));
    rig.until("Scans read", Duration::from_secs(30), || ctl.state.borrow().read_done() && app::tiles(&shell.window).len() == 2);
    let b = d.path("Home/Scans/B.tif");
    rig.until("the read's looks landed", Duration::from_secs(10), || ctl.state.borrow().presence(&b).is_present());

    // Any look at the disk on this thread fails the check; those behind it are counted.
    let interface = std::thread::current().id();
    let behind = Arc::new(AtomicUsize::new(0));
    let counted = behind.clone();
    presence::on_look(Some(Box::new(move |path| {
        assert!(std::thread::current().id() != interface, "{path} looked at on the interface thread");
        counted.fetch_add(1, Ordering::Relaxed);
    })));
    std::fs::remove_file(d.root.join("Home/Scans/B.tif")).unwrap();
    for _ in 0..4 {
        ctl.act(oni_app::Act::CycleFilter);
        rig.pump();
    }
    assert!(ctl.state.borrow().filter().is_none(), "the filter came round");
    assert!(!missing(&shell, "B"), "a lay reads the state last published, never the file");
    assert!(ctl.state.borrow().presence(&b).is_present());

    // Twin: the looks behind the interface find the file removed, and the tile shows it.
    ctl.probe_presence();
    rig.until("B found missing", Duration::from_secs(10), || missing(&shell, "B"));
    assert_eq!(ctl.state.borrow().presence(&b), Presence::Missing);
    assert!(!missing(&shell, "A"), "twin: the photo still there is not");
    assert!(behind.load(Ordering::Relaxed) >= 2, "the looks ran, off the interface thread");
    presence::on_look(None);
    drop(shell);
    let _ = std::fs::remove_dir_all(&d.root);
}

pub(crate) fn a_remembered_folder_on_a_disk_away_is_kept_and_reopens_on_return() {
    let Some(gpu) = gpu("a disk away") else { return };
    let d = disk("away", &["Volumes/T7/Scans/Roll 12/A.tif", "Volumes/T7/Scans/Roll 12/B.tif"]);
    let film = oni_settings::Film::new("Portra Trip");
    oni_library::folders::set_roll(Some(&film), &d.path("Volumes/T7/Scans/Roll 12")).unwrap();
    let scans = d.path("Volumes/T7/Scans");
    let rig = rig();
    let shell = d.launch(&gpu);
    let ctl = shell.ctl.clone();
    shell.open_folder(Path::new(&scans));
    rig.until("Scans read and drawn", Duration::from_secs(60), || ctl.state.borrow().read_done() && ctl.state.borrow().loaded() >= 2);
    let title = |shell: &Shell| shell.window.global::<Library>().get_roll_title().to_string();
    assert_eq!(title(&shell), "Portra Trip");
    drop((ctl, shell));

    // The disk unplugged, the next launch keeps the folder, its rows, its roll and its place.
    std::fs::rename(d.root.join("Volumes/T7"), d.root.join("T7 away")).unwrap();
    let shell = d.launch(&gpu);
    let ctl = shell.ctl.clone();
    assert!(shell.open_remembered());
    rig.until("Scans read away", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    let state = || ctl.state.borrow().opened_presence();
    assert_eq!(ctl.state.borrow().opened(), Some(scans.as_str()), "the folder remembered is the one opened");
    assert_eq!(state(), Presence::Offline(Away::Disconnected));
    assert_eq!(ctl.state.borrow().row_presence(&scans), Presence::Offline(Away::Disconnected));
    assert_eq!(Preferences::load(&d.support).string("openedFolder"), Some(scans.as_str()), "nothing replaced it");
    assert_eq!(ctl.state.borrow().photos(), 2, "its rows are kept");
    assert!(missing(&shell, "A") && missing(&shell, "B"), "its tiles show their originals away");
    assert_eq!(title(&shell), "Portra Trip", "laid as last seen, its roll's name included");
    assert!(!status(&shell).contains("decode"), "no thumbnail is tried on a disk away: {}", status(&shell));
    let reads = ctl.state.borrow().reads();

    // Plugged back in: read again once, there again.
    std::fs::rename(d.root.join("T7 away"), d.root.join("Volumes/T7")).unwrap();
    rig.until("Scans back", Duration::from_secs(20), || state().is_present() && ctl.state.borrow().read_done());
    rig.until("its tiles back", Duration::from_secs(10), || !missing(&shell, "A") && !missing(&shell, "B"));
    assert_eq!(ctl.state.borrow().reads(), reads + 1, "read again once on its return");

    // Twin: a disk that comes for another folder reads the opened one again never.
    std::fs::create_dir_all(d.root.join("Volumes/T8")).unwrap();
    let n = shell.window.global::<Navigator>();
    let titles = || (0..n.get_rows().row_count()).filter_map(|i| n.get_rows().row_data(i)).map(|r| r.title.to_string()).collect::<Vec<_>>();
    rig.until("T8 mounted", Duration::from_secs(10), || titles().contains(&"T8".to_string()));
    rig.settle();
    assert_eq!(ctl.state.borrow().reads(), reads + 1, "a folder never gone is not read again");
    drop((ctl, shell));
    let _ = std::fs::remove_dir_all(&d.root);
}

pub(crate) fn a_missing_path_on_the_command_line_opens_nothing_and_says_so() {
    let Some(gpu) = gpu("the command line") else { return };
    let d = disk("argv", &["Home/Scans/A.tif", "Home/loose.tif"]);
    let before = d.path("Home/Elsewhere");
    std::fs::create_dir_all(&d.support).unwrap();
    Preferences::load(&d.support).set("openedFolder", serde_json::Value::String(before.clone())).unwrap();
    let rig = rig();
    let shell = d.launch(&gpu);
    let ctl = shell.ctl.clone();
    let remembered = || Preferences::load(&d.support).string("openedFolder").map(str::to_owned);

    // Mistyped: nothing opens, the home it lies in least of all, and nothing is remembered.
    shell.add(&d.root.join("Home/Scnas"));
    rig.until("the refusal said", Duration::from_secs(10), || status(&shell).starts_with("Not found:"));
    assert_eq!(status(&shell), format!("Not found: {}", d.path("Home/Scnas")));
    assert_eq!((ctl.state.borrow().opened(), remembered()), (None, Some(before.clone())));
    // On a disk away: the same, never the volumes' folder.
    shell.add(&d.root.join("Volumes/T7/Roll"));
    rig.until("the second refusal", Duration::from_secs(10), || status(&shell).contains("Volumes/T7/Roll"));
    assert!(status(&shell).starts_with("Not found:"));
    // A file at the top of the home never opens the whole home.
    shell.add(&d.root.join("Home/loose.tif"));
    rig.until("the wide refusal", Duration::from_secs(10), || status(&shell).contains("whole disk or the home folder"));
    assert_eq!((ctl.state.borrow().opened(), remembered()), (None, Some(before)));

    // Twin: a folder there opens and is remembered.
    shell.add(&d.root.join("Home/Scans"));
    let scans = d.path("Home/Scans");
    rig.until("Scans opened", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().opened() == Some(scans.as_str()));
    assert_eq!(remembered(), Some(scans));
    drop((ctl, shell));
    let _ = std::fs::remove_dir_all(&d.root);
}
