//! The ONI Trash and the projects are the whole library's, whatever folder is open, a disk away
//! said, and a rename carries the paths a project recorded. Every folder lies in the check's scratch.

use oni_app::app::{self, Options, Shell};
use oni_app::{Confirm, FolderLine, Library, Navigator, Told};
use oni_core::{Archive, EditorStore};
use oni_library::index::LibraryIndex;
use oni_library::navigator::Computer;
use oni_library::{paths, project};
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

struct Disk {
    root: PathBuf,
    computer: Computer,
}

/// A home and a volumes folder in the check's own scratch, a still TIFF at each of `files`.
fn disk(label: &str, files: &[&str]) -> Disk {
    let root = std::env::temp_dir().join(format!("oni-app-wide-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    // Each its own tone, so no two share a fingerprint.
    for (i, file) in files.iter().enumerate() {
        common::linear_tiff(&root.join(file), 48, 32, 0.2 + 0.1 * i as f32);
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

    fn at(&self, file: &str) -> String {
        paths::canonical(&self.root.join(file))
    }

    fn index(&self) -> LibraryIndex {
        LibraryIndex::open(&self.root.join("Caches/index.sqlite")).unwrap()
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

fn opened(rig: &common::Rig, shell: &Shell, folder: &Path) {
    shell.open_folder(folder);
    let ctl = shell.ctl.clone();
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().opened().is_some());
}

/// The tiles shown, by caption in the grid's order, each with its missing flag.
fn shown(shell: &Shell) -> Vec<(String, bool)> {
    let mut tiles = app::tiles(&shell.window);
    tiles.sort_by_key(|t| t.index);
    tiles.into_iter().map(|t| (t.caption.to_string(), t.missing)).collect()
}

/// Past the double click, so the next click on a tile is a first one.
fn rest(rig: &common::Rig) {
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    rig.pump();
}

fn click(rig: &common::Rig, shell: &Shell, caption: &str, shift: bool) {
    let i = shown(shell).iter().position(|(c, _)| c == caption).unwrap_or_else(|| panic!("{caption} in {:?}", shown(shell)));
    shell.window.global::<Library>().invoke_tile_clicked(i as i32, false, shift);
    rig.pump();
}

/// The cell menu on `caption`, then its line starting with `line`.
fn cell_menu(rig: &common::Rig, shell: &Shell, caption: &str, line: &str) {
    let lib = shell.window.global::<Library>();
    let t = app::tiles(&shell.window).into_iter().find(|t| t.caption == caption).unwrap();
    assert!(lib.invoke_context(t.x + t.width / 2.0, t.y + t.height / 2.0));
    let lines = lib.get_menu_lines();
    let i = (0..lines.row_count()).position(|i| lines.row_data(i).is_some_and(|l| l.text.starts_with(line))).unwrap_or_else(|| panic!("{line} in the menu"));
    lib.invoke_menu_chosen(i as i32);
    rig.pump();
}

fn trash_count(shell: &Shell) -> String {
    shell.window.global::<Library>().get_trash_rows().row_data(0).unwrap().count.to_string()
}

fn trashed(path: &str) -> bool {
    oni_settings::sidecar::read(Path::new(path), false, true).unwrap().trashed_at.is_some()
}

fn told(shell: &Shell) -> Option<(String, String)> {
    let t = shell.window.global::<Told>();
    t.get_shown().then(|| (t.get_title().to_string(), t.get_message().to_string()))
}

fn restore_all(rig: &common::Rig, shell: &Shell) {
    shell.window.global::<Library>().invoke_trash_row_restore(0);
    rig.pump();
    let confirm = shell.window.global::<Confirm>();
    assert!(confirm.get_shown(), "Restore All asks first");
    confirm.invoke_answered(true);
    rig.landed(&shell.ctl);
}

pub(crate) fn a_trash_made_in_a_lists_after_b_opens_and_restore_all_restores_it() {
    let Some(gpu) = gpu("the library-wide ONI Trash") else { return };
    let d = disk("trash", &["Home/A/A1.tif", "Home/A/A2.tif", "Home/B/B1.tif", "Volumes/T7/C/C1.tif"]);
    let (a1, c1) = (d.at("Home/A/A1.tif"), d.at("Volumes/T7/C/C1.tif"));
    let rig = rig();
    let shell = d.launch(&gpu);
    for (folder, photo) in [("Home/A", "A1"), ("Volumes/T7/C", "C1")] {
        opened(&rig, &shell, &d.root.join(folder));
        rest(&rig);
        click(&rig, &shell, photo, false);
        shell.ctl.act(oni_app::Act::Remove);
        rig.landed(&shell.ctl);
    }
    assert!(trashed(&a1) && trashed(&c1));

    opened(&rig, &shell, &d.root.join("Home/B"));
    assert_eq!(trash_count(&shell), "2", "B open, the trash still counts what A and C hold");
    // Twin: the same trash cut to the opened folder reads empty.
    let index = d.index();
    let b = d.at("Home/B");
    assert_eq!(index.trashed_frames().len(), 2);
    assert!(index.trashed_frames().iter().all(|e| !paths::holds(&b, &e.path)));

    // T7 goes away: its photo tiles as offline in the trash, A's as present.
    std::fs::rename(d.root.join("Volumes/T7"), d.root.join("T7 away")).unwrap();
    let lib = shell.window.global::<Library>();
    lib.invoke_trash_row_double_clicked(0);
    let ctl = shell.ctl.clone();
    rig.until("C1 marked offline", Duration::from_secs(10), || shown(&shell).contains(&("C1".to_string(), true)));
    assert!(!ctl.state.borrow().presence(&c1).is_present());
    assert!(shown(&shell).contains(&("A1".to_string(), false)), "{:?}", shown(&shell));

    // Restore All brings back what it reaches and says what it left.
    restore_all(&rig, &shell);
    assert!(!trashed(&a1), "A1 restored from B");
    let (title, message) = told(&shell).expect("the photo left behind is said");
    assert_eq!(title, "Restore");
    assert!(message.starts_with("One photo was not restored") && message.contains("cannot be found"), "{message}");
    assert_eq!(trash_count(&shell), "1");
    assert_eq!(shown(&shell), [("C1".to_string(), true)]);
    assert!(!d.root.join("Volumes/T7").exists(), "nothing is made where the disk was");
    shell.window.global::<Told>().invoke_closed();

    // Twin: the disk back, the same key restores it and says nothing.
    std::fs::rename(d.root.join("T7 away"), d.root.join("Volumes/T7")).unwrap();
    restore_all(&rig, &shell);
    assert!(!trashed(&c1));
    assert_eq!((told(&shell), trash_count(&shell)), (None, String::new()));
    let _ = std::fs::remove_dir_all(&d.root);
}

pub(crate) fn a_project_from_a_still_lists_after_b_opens() {
    let Some(gpu) = gpu("projects across folders") else { return };
    let d = disk("projects", &["Home/A/A1.tif", "Home/A/A2.tif", "Home/A/A3.tif", "Home/B/B1.tif"]);
    let rig = rig();
    let shell = d.launch(&gpu);
    let support = d.root.join("Support");
    opened(&rig, &shell, &d.root.join("Home/A"));
    rest(&rig);
    click(&rig, &shell, "A1", false);
    click(&rig, &shell, "A3", true);
    cell_menu(&rig, &shell, "A1", "New Project from");
    assert_eq!(project::load(&support).projects[0].members.len(), 3);
    // A2 goes to the ONI Trash, A3's file leaves its disk behind the app's back.
    rest(&rig);
    click(&rig, &shell, "A2", false);
    shell.ctl.act(oni_app::Act::Remove);
    rig.landed(&shell.ctl);
    std::fs::rename(d.root.join("Home/A/A3.tif"), d.root.join("A3 elsewhere.tif")).unwrap();

    opened(&rig, &shell, &d.root.join("Home/B"));
    let lib = shell.window.global::<Library>();
    lib.invoke_surface_picked(1);
    rig.pump();
    let row = lib.get_project_rows().row_data(0).expect("the project is listed");
    assert_eq!((row.title.as_str(), row.caption.as_str()), ("Project 1", "2"), "A1 and A3, B open");
    rig.until("A3 marked missing", Duration::from_secs(10), || shown(&shell) == [("A1".to_string(), false), ("A3".to_string(), true)]);
    assert_eq!(project::load(&support).projects[0].members.len(), 3, "the trashed photo stays a member");

    // Twin: the members resolved against B's rows alone list nothing.
    let file = project::load(&support);
    let b_rows = oni_app::library::shown_under(&d.index(), &d.at("Home/B"));
    assert!(project::grouped(&file.projects, &b_rows)[0].entries.is_empty());
    let _ = std::fs::remove_dir_all(&d.root);
}

fn titles(w: &oni_app::MainWindow) -> Vec<String> {
    let rows = w.global::<Navigator>().get_rows();
    (0..rows.row_count()).filter_map(|i| rows.row_data(i)).map(|l: FolderLine| l.title.to_string()).collect()
}

fn at(w: &oni_app::MainWindow, title: &str) -> usize {
    titles(w).iter().position(|t| t == title).unwrap_or_else(|| panic!("{title} is not among {:?}", titles(w)))
}

/// `folder` renamed `to` from its navigator row's menu, as a hand renames it.
fn rename(rig: &common::Rig, shell: &Shell, folder: &str, to: &str) {
    let w = &shell.window;
    for up in ["Home", "Scans"] {
        if !w.global::<Navigator>().get_rows().iter().any(|l| l.title == up && l.unfolded) {
            w.global::<Navigator>().invoke_row_clicked(at(w, up) as i32);
        }
        rig.pump();
    }
    rig.until("the rolls listed", Duration::from_secs(10), || titles(w).contains(&folder.to_string()));
    let n = w.global::<Navigator>();
    assert!(n.invoke_row_menu(at(w, folder) as i32));
    let m = n.get_menu_lines();
    let i = (0..m.row_count()).position(|i| m.row_data(i).is_some_and(|l| l.text == "Rename…")).unwrap();
    n.invoke_menu_chosen(i as i32);
    rig.pump();
    n.invoke_rename_committed(at(w, folder) as i32, to.into());
    rig.pump();
    let ctl = shell.ctl.clone();
    rig.until("the folder read again", Duration::from_secs(30), || ctl.state.borrow().read_done());
}

/// The archive tree's folder row named `title`, unfolded or not.
fn unfolded(shell: &Shell, title: &str) -> bool {
    let rows = shell.window.global::<Library>().get_rows();
    (0..rows.row_count()).filter_map(|i| rows.row_data(i)).find(|r| r.kind != 2 && r.title == title).unwrap_or_else(|| panic!("{title} in the tree")).expanded
}

pub(crate) fn a_rename_moves_the_projects_recorded_path() {
    let Some(gpu) = gpu("a rename followed").filter(|_| cfg!(unix)) else { return };
    let d = disk("rename", &["Home/Scans/Roll 1/A.tif", "Home/Scans/Roll 1/Sub/S.tif", "Home/Scans/Roll 1/Kept/K.tif", "Home/Scans/Roll 2/B.tif"]);
    let rig = rig();
    let shell = d.launch(&gpu);
    let support = d.root.join("Support");
    opened(&rig, &shell, &d.root.join("Home/Scans/Roll 1"));
    rest(&rig);
    click(&rig, &shell, "A", false);
    cell_menu(&rig, &shell, "A", "New Project from");
    let recorded = || project::load(&support).projects[0].paths.clone().unwrap_or_default().into_values().collect::<Vec<_>>();
    assert_eq!(recorded(), [d.at("Home/Scans/Roll 1/A.tif")]);
    // A hand folds Sub, and A stays selected.
    let lib = shell.window.global::<Library>();
    let rows = lib.get_rows();
    let sub = (0..rows.row_count()).position(|i| rows.row_data(i).is_some_and(|r| r.title == "Sub")).unwrap();
    lib.invoke_row_folded(sub as i32);
    rig.pump();
    assert!(!unfolded(&shell, "Sub") && unfolded(&shell, "Kept"));

    // Twin first: a rename elsewhere leaves the recorded path where it was.
    rename(&rig, &shell, "Roll 2", "Roll 7");
    assert_eq!(recorded(), [d.at("Home/Scans/Roll 1/A.tif")]);
    assert!(!unfolded(&shell, "Sub"), "a read again keeps the fold");

    rename(&rig, &shell, "Roll 1", "Roll 9");
    let moved = d.at("Home/Scans/Roll 9/A.tif");
    assert_eq!(recorded(), [moved.clone()], "projects.json follows the rename");
    assert!(shell.ctl.state.borrow().picked().contains(&moved), "the selection follows");
    assert!(!unfolded(&shell, "Sub"), "the fold a hand made stays");
    assert!(unfolded(&shell, "Kept"), "and an open folder stays open");
    lib.invoke_surface_picked(1);
    rig.pump();
    assert_eq!(shown(&shell), [("A".to_string(), false)], "the project still lists its photo");
    let _ = std::fs::remove_dir_all(&d.root);
}
