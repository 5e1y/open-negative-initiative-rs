//! What a gesture could not do is told wherever the hand is, by the notice over every view: a trash
//! refused in the gallery, a roll walked back in part, a folder the disk refuses, a name refused.
//! A success says nothing. Every folder lies in the check's own scratch.

use oni_app::app::{Options, Shell};
use oni_app::{App, FolderLine, Navigator, Told};
use oni_core::{Archive, EditorStore};
use oni_library::navigator::Computer;
use oni_library::paths;
use slint::{ComponentHandle, Model};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

struct Disk {
    root: PathBuf,
    computer: Computer,
}

/// A home and a volumes folder in the check's own scratch, a still TIFF at each of `files`.
fn disk(label: &str, files: &[&str]) -> Disk {
    let root = std::env::temp_dir().join(format!("oni-app-told-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for file in files {
        common::linear_tiff(&root.join(file), 48, 32, 0.5);
    }
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

/// The notice as it stands: shown, its title, its message and its action's key.
fn told(shell: &Shell) -> Option<(String, String, String)> {
    let t = shell.window.global::<Told>();
    t.get_shown().then(|| (t.get_title().to_string(), t.get_message().to_string(), t.get_action().to_string()))
}

fn opened(rig: &common::Rig, shell: &Shell, folder: &Path) {
    shell.open_folder(folder);
    let ctl = shell.ctl.clone();
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().opened().is_some());
}

/// Every tile of the gallery picked, the first ringed; past the double click, which would open it.
fn pick_all(rig: &common::Rig, shell: &Shell, count: usize) {
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    let lib = shell.window.global::<oni_app::Library>();
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(count as i32 - 1, false, true);
    rig.pump();
}

pub(crate) fn a_refused_trash_in_the_gallery_opens_a_notice() {
    let Some(gpu) = gpu("the gallery's notice") else { return };
    let d = disk("trash", &["Home/Scans/A.tif", "Home/Scans/B.tif"]);
    let scans = d.root.join("Home/Scans");
    let rig = rig();
    let shell = d.launch(&gpu);
    opened(&rig, &shell, &scans);
    assert!(shell.window.global::<App>().get_gallery(), "the gesture is made in the gallery");
    let lib = shell.window.global::<oni_app::Library>();

    // Twin: a trash that lands says nothing.
    lib.invoke_tile_clicked(0, false, false);
    rig.pump();
    shell.ctl.act(oni_app::Act::Remove);
    rig.landed(&shell.ctl);
    assert_eq!(told(&shell), None, "a success stays silent");
    let a = sidecar_of(&scans.join("A.tif"));
    assert!(oni_settings::sidecar::read(&scans.join("A.tif"), false, true).unwrap().trashed_at.is_some(), "{a}");

    // A newer ONI wrote B's settings: the trash refuses it, and the gallery says so over itself.
    std::fs::write(oni_settings::sidecar::url_for(&scans.join("B.tif")), r#"{"version": 999}"#).unwrap();
    let ctl = shell.ctl.clone();
    rig.until("B alone in the gallery", Duration::from_secs(10), || ctl.state.borrow().read_done() && ctl.state.borrow().order().len() == 1);
    lib.invoke_tile_clicked(0, false, false);
    rig.pump();
    shell.ctl.act(oni_app::Act::Remove);
    rig.landed(&shell.ctl);
    let (title, message, action) = told(&shell).expect("the refusal is told in the gallery");
    assert_eq!((title.as_str(), action.as_str()), ("Move to ONI Trash", ""));
    assert!(message.starts_with("One photo was not moved to the ONI Trash") && message.contains("newer version"), "{message}");
    assert_eq!(std::fs::read_to_string(oni_settings::sidecar::url_for(&scans.join("B.tif"))).unwrap(), r#"{"version": 999}"#, "the newer settings are left as they were");
    rig.settle();
    println!("shot {}", rig.shot("told-trash").display());
    // Its key takes it away.
    shell.window.global::<Told>().invoke_closed();
    assert_eq!(told(&shell), None);
    let _ = std::fs::remove_dir_all(&d.root);
}

fn sidecar_of(path: &Path) -> String {
    std::fs::read_to_string(oni_settings::sidecar::url_for(path)).unwrap_or_default()
}

pub(crate) fn an_undo_that_cannot_put_every_photo_back_says_how_many() {
    let Some(gpu) = gpu("a partial undo") else { return };
    let d = disk("undo", &["Home/Scans/A.tif", "Home/Scans/B.tif", "Home/Scans/C.tif"]);
    let scans = d.root.join("Home/Scans");
    let rig = rig();
    let shell = d.launch(&gpu);
    let ctl = shell.ctl.clone();
    opened(&rig, &shell, &scans);
    let roll = scans.join(oni_library::roll_folder::dated(oni_settings::Date::now()));
    let names = ["A.tif", "B.tif", "C.tif"];
    let rolled = || {
        rig.until("the photos read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().order().len() == 3);
        pick_all(&rig, &shell, 3);
        ctl.act(oni_app::Act::NewRoll);
        rig.roll_confirmed(&ctl, &shell.window);
        rig.until("the roll read", Duration::from_secs(30), || ctl.state.borrow().read_done());
        assert!(names.iter().all(|n| roll.join(n).exists()), "the three moved into the roll");
        assert_eq!(told(&shell), None, "a whole New Roll says nothing");
    };

    // Twin: an undo that puts every photo back stays silent, and the folder it made goes.
    rolled();
    ctl.act(oni_app::Act::Undo);
    rig.landed(&ctl);
    rig.until("the folder read again", Duration::from_secs(30), || ctl.state.borrow().read_done());
    assert!(names.iter().all(|n| scans.join(n).exists()) && !roll.exists(), "every photo back, the roll folder gone");
    assert_eq!(told(&shell), None, "a complete undo stays silent");

    // Two files took two of the names meanwhile: the undo puts one photo back and says how many stayed.
    rolled();
    for taken in ["A.tif", "B.tif"] {
        std::fs::write(scans.join(taken), b"another file").unwrap();
    }
    ctl.act(oni_app::Act::Undo);
    rig.landed(&ctl);
    let (title, message, action) = told(&shell).expect("the shortfall is told");
    assert_eq!((title.as_str(), action.as_str()), ("Undo", "Show in Finder"));
    assert_eq!(message, "2 of the items could not be put back: their folder moved or a file took their name.");
    assert!(scans.join("C.tif").exists() && roll.join("A.tif").exists() && roll.join("B.tif").exists(), "C back, A and B kept in the roll");
    assert_eq!(std::fs::read(scans.join("A.tif")).unwrap(), b"another file", "the file that took A's name is untouched");
    assert!(roll.is_dir(), "the roll folder holding them stays");
    rig.until("the folder read again", Duration::from_secs(30), || ctl.state.borrow().read_done());
    // `followed`: the picks go where the disk put each photo, C back, A and B still in the roll.
    let at = |folder: &Path, name: &str| (paths::canonical(folder), name.to_owned());
    let picked: HashSet<(String, String)> = ctl.state.borrow().picked().iter().map(|p| at(Path::new(&paths::parent(p)), &paths::name(p))).collect();
    assert_eq!(picked, HashSet::from([at(&roll, "A.tif"), at(&roll, "B.tif"), at(&scans, "C.tif")]), "the three photos stay selected");
    // Twin: the picks as if every photo had come back are told apart, folder by folder.
    assert_ne!(picked, names.iter().map(|n| at(&scans, n)).collect::<HashSet<_>>());
    rig.settle();
    println!("shot {}", rig.shot("told-undo").display());
    let _ = std::fs::remove_dir_all(&d.root);
}

fn titles(w: &oni_app::MainWindow) -> Vec<String> {
    let rows = w.global::<Navigator>().get_rows();
    (0..rows.row_count()).filter_map(|i| rows.row_data(i)).map(|l: FolderLine| l.title.to_string()).collect()
}

fn at(w: &oni_app::MainWindow, title: &str) -> usize {
    titles(w).iter().position(|t| t == title).unwrap_or_else(|| panic!("{title} is not among {:?}", titles(w)))
}

fn choose(rig: &common::Rig, w: &oni_app::MainWindow, title: &str, item: &str) {
    let n = w.global::<Navigator>();
    assert!(n.invoke_row_menu(at(w, title) as i32));
    let m = n.get_menu_lines();
    let i = (0..m.row_count()).position(|i| m.row_data(i).is_some_and(|l| l.text == item)).unwrap_or_else(|| panic!("{item} is not in {title}'s menu"));
    n.invoke_menu_chosen(i as i32);
    rig.pump();
}

/// The navigator unfolded down to `Scans`, its rolls listed.
fn navigator(rig: &common::Rig, shell: &Shell, d: &Disk) {
    opened(rig, shell, &d.root.join("Home/Scans/Roll 1"));
    let w = &shell.window;
    for folder in ["Home", "Scans"] {
        if !w.global::<Navigator>().get_rows().iter().any(|l| l.title == folder && l.unfolded) {
            w.global::<Navigator>().invoke_row_folded(at(w, folder) as i32);
        }
        rig.pump();
    }
    rig.until("Scans listed", Duration::from_secs(10), || titles(w).contains(&"Roll 1".to_string()));
}

#[cfg(unix)]
fn locked(path: &Path, locked: bool) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(if locked { 0o555 } else { 0o755 })).unwrap();
}

pub(crate) fn a_new_folder_refused_by_the_disk_is_said() {
    let Some(gpu) = gpu("New Folder's refusal") else { return };
    let d = disk("new-folder", &["Home/Scans/Roll 1/A.tif"]);
    let scans = d.root.join("Home/Scans");
    let rig = rig();
    let shell = d.launch(&gpu);
    navigator(&rig, &shell, &d);
    let w = &shell.window;

    #[cfg(unix)]
    {
        locked(&scans, true);
        choose(&rig, w, "Scans", "New Folder Inside");
        locked(&scans, false);
        let (title, message, _) = told(&shell).expect("the refusal is told");
        assert_eq!(title, "New Folder");
        assert_eq!(message, "The folder could not be made inside “Scans”: it cannot be written to.");
        assert!(!scans.join("New Folder").exists());
        shell.window.global::<Told>().invoke_closed();
    }
    // Twin: the disk writable, the folder is made and nothing is said.
    choose(&rig, w, "Scans", "New Folder Inside");
    assert!(scans.join("New Folder").is_dir());
    assert_eq!(told(&shell), None, "a folder made says nothing");
    let _ = std::fs::remove_dir_all(&d.root);
}

pub(crate) fn a_rename_with_a_slash_says_why() {
    let Some(gpu) = gpu("a refused name") else { return };
    let d = disk("rename", &["Home/Scans/Roll 1/A.tif"]);
    let scans = d.root.join("Home/Scans");
    let rig = rig();
    let shell = d.launch(&gpu);
    navigator(&rig, &shell, &d);
    let w = &shell.window;
    let n = w.global::<Navigator>();
    let rename = |to: &str| {
        choose(&rig, w, "Roll 1", "Rename…");
        n.invoke_rename_committed(at(w, "Roll 1") as i32, to.into());
        rig.pump();
    };

    rename("12/06/2026");
    let (title, message, _) = told(&shell).expect("the slash is told");
    assert_eq!(title, "Rename");
    assert_eq!(message, "A folder's name cannot hold “/”, which separates folders.");
    assert!(scans.join("Roll 1").is_dir() && !scans.join("12").exists(), "nothing moved");
    rename(".old");
    let (_, message, _) = told(&shell).expect("the dot is told");
    assert_eq!(message, "A name starting with a dot would hide the folder.");
    assert!(scans.join("Roll 1").is_dir() && !scans.join(".old").exists());
    w.global::<Told>().invoke_closed();

    // Twin: the same date with dashes renames the folder and says nothing.
    rename("12-06-2026");
    rig.until("the renamed folder listed", Duration::from_secs(10), || titles(w).contains(&"12-06-2026".to_string()));
    assert!(scans.join("12-06-2026").is_dir() && !scans.join("Roll 1").exists());
    assert_eq!(told(&shell), None, "a rename that lands says nothing");
    let _ = std::fs::remove_dir_all(&d.root);
}
