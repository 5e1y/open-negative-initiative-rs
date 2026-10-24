//! The explorer's gestures on the headless window, in the check's own scratch: the gallery's bar on
//! the selection alone, a drop from several folders said, Return renaming where a double click does not.

use oni_app::app::{Options, Shell};
use oni_app::{App, Bar, Confirm, FolderLine, Library, Menus, Navigator, Told, TreeRow};
use oni_core::{Archive, EditorStore};
use oni_library::navigator::Computer;
use oni_library::paths;
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
    let root = std::env::temp_dir().join(format!("oni-app-explorer-{label}-{}", std::process::id()));
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

fn opened(rig: &common::Rig, shell: &Shell, folder: &Path) {
    shell.open_folder(folder);
    let ctl = shell.ctl.clone();
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().opened().is_some());
}

fn trashed(path: &Path) -> bool {
    oni_settings::sidecar::read(path, false, true).is_ok_and(|l| l.trashed_at.is_some())
}

fn told(shell: &Shell) -> Option<(String, String)> {
    let t = shell.window.global::<Told>();
    t.get_shown().then(|| (t.get_title().to_string(), t.get_message().to_string()))
}

/// The person edits a frame of one roll, opens another in the gallery and clicks the ground: the
/// bar's Trash, Reset and Export reach nothing, so no frame off screen is touched.
pub(crate) fn the_gallery_bar_acts_on_the_selection_only() {
    let Some(gpu) = gpu("the gallery's bar") else { return };
    let d = disk("bar", &["Home/Roll 8/A.tif", "Home/Roll 9/B.tif", "Home/Roll 9/C.tif"]);
    let (roll8, roll9) = (d.root.join("Home/Roll 8"), d.root.join("Home/Roll 9"));
    let rig = rig();
    let shell = d.launch(&gpu);
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, app, bar, menus) = (w.global::<Library>(), w.global::<App>(), w.global::<Bar>(), w.global::<Menus>());
    opened(&rig, &shell, &roll8);
    lib.invoke_tile_opened(0);
    rig.until("A in the editor", Duration::from_secs(60), || ctl.state.borrow().open_path().is_some());
    assert!(!app.get_gallery());
    lib.invoke_tree_empty_clicked();
    rig.pump();
    // Beside the photo, the bar acts on it.
    assert_eq!(bar.get_targets(), 1, "beside a photo the open frame is the target");
    ctl.act(oni_app::Act::ToggleMode);
    rig.pump();
    opened(&rig, &shell, &roll9);
    assert!(app.get_gallery() && ctl.state.borrow().open_path().is_some_and(|o| o.ends_with("A.tif")), "A stays open behind the gallery");
    lib.invoke_tree_empty_clicked();
    rig.pump();
    assert_eq!(bar.get_targets(), 0, "in the gallery with nothing picked, the bar reaches nothing");
    assert!(!menus.get_live().remove && !menus.get_live().reset && !menus.get_live().export, "nor do the menus");
    ctl.act(oni_app::Act::Remove);
    rig.landed(&ctl);
    assert!(!trashed(&roll8.join("A.tif")), "the frame off screen is never trashed");
    // Twin: a photo picked is the target, and the Trash takes it alone.
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    lib.invoke_tile_clicked(0, false, false);
    rig.pump();
    assert_eq!(bar.get_targets(), 1);
    ctl.act(oni_app::Act::Remove);
    rig.landed(&ctl);
    assert!(trashed(&roll9.join("B.tif")) && !trashed(&roll9.join("C.tif")) && !trashed(&roll8.join("A.tif")));
    let _ = std::fs::remove_dir_all(&d.root);
}

/// A drop of items from two folders opens the first one's and says what it left out; the
/// home dropped whole asks before any read.
pub(crate) fn a_drop_from_two_folders_says_what_it_left() {
    let Some(gpu) = gpu("a drop from two folders") else { return };
    let d = disk("drop", &["Home/A/1.tif", "Home/A/2.tif", "Home/B/3.tif", "Home/B/4.tif"]);
    let (a, b) = (d.root.join("Home/A"), d.root.join("Home/B"));
    let rig = rig();
    let shell = d.launch(&gpu);
    let ctl = shell.ctl.clone();
    let opened = || ctl.state.borrow().opened().map(str::to_owned);
    assert!(ctl.dropped(&[b.clone()]));
    rig.until("B read", Duration::from_secs(30), || ctl.state.borrow().read_done() && opened().is_some_and(|o| o.ends_with("/B")));
    assert_eq!(told(&shell), None, "twin: one folder leaves nothing out, and says nothing");
    // A folder first, then two photos of another: the folder opens, the photos are said left out.
    assert!(ctl.dropped(&[a.clone(), b.join("3.tif"), b.join("4.tif")]));
    rig.until("A read", Duration::from_secs(30), || ctl.state.borrow().read_done() && opened().is_some_and(|o| o.ends_with("/A")));
    let (title, message) = told(&shell).expect("what the drop left out is said");
    assert_eq!(title, "Open");
    assert_eq!(message, "2 items from other folders were left out: ONI opens one folder at a time.");
    assert!(shell.window.global::<App>().get_gallery(), "no photo of another folder opens in the editor");
    rig.settle();
    println!("shot {}", rig.shot("explorer-drop-left").display());
    shell.window.global::<Told>().invoke_closed();
    // The home dropped whole asks first; Cancel reads nothing.
    let confirm = shell.window.global::<Confirm>();
    assert!(ctl.dropped(&[d.root.join("Home")]));
    rig.until("the home asked about", Duration::from_secs(10), || confirm.get_shown());
    assert_eq!(confirm.get_title().as_str(), "Show Every Photo on “Home”?");
    rig.settle();
    println!("shot {}", rig.shot("explorer-wide-ask").display());
    confirm.invoke_answered(false);
    rig.pump();
    assert!(opened().is_some_and(|o| o.ends_with("/A")), "twin: cancelled, A stays opened");
    assert!(ctl.dropped(&[d.root.join("Home")]));
    rig.until("the home asked about", Duration::from_secs(10), || confirm.get_shown());
    confirm.invoke_answered(true);
    rig.until("the home opened", Duration::from_secs(30), || ctl.state.borrow().read_done() && opened().is_some_and(|o| o.ends_with("/Home")));
    assert_eq!(ctl.state.borrow().photos(), 4, "the whole home read, once asked");
    let _ = std::fs::remove_dir_all(&d.root);
}

fn rows(w: &oni_app::MainWindow) -> Vec<FolderLine> {
    w.global::<Navigator>().get_rows().iter().collect()
}

fn at(w: &oni_app::MainWindow, title: &str) -> i32 {
    rows(w).iter().position(|l| l.title == title).unwrap_or_else(|| panic!("{title} is not among the rows")) as i32
}

fn tree(lib: &Library) -> Vec<TreeRow> {
    lib.get_rows().iter().collect()
}

/// Return renames the picked row, as the Finder's sidebar does; a double click on a folder,
/// in the navigator or the tree beside a photo, never types over its name.
pub(crate) fn return_renames_and_double_click_does_not() {
    let Some(gpu) = gpu("Return and the double click") else { return };
    let d = disk("rename", &["Home/Scans/Roll 1/A.tif", "Home/Scans/Roll 2/B.tif"]);
    let scans = d.root.join("Home/Scans");
    let rig = rig();
    let shell = d.launch(&gpu);
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let n = w.global::<Navigator>();
    shell.add(&scans.join("Roll 1"));
    rig.until("Roll 1 read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().opened().is_some());
    rig.until("Roll 1 among the rows", Duration::from_secs(10), || rows(w).iter().any(|l| l.title == "Roll 1"));
    // A double click opens and renames nothing.
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    n.invoke_row_clicked(at(w, "Roll 2"));
    n.invoke_row_clicked(at(w, "Roll 2"));
    rig.until("Roll 2 opened", Duration::from_secs(30), || ctl.state.borrow().opened().is_some_and(|o| o.ends_with("Roll 2")));
    assert!(rows(w).iter().all(|l| !l.renaming), "a double click renames nothing");
    // Return types over the picked row; Escape puts it back.
    rig.key(&slint::SharedString::from(slint::platform::Key::Return));
    assert!(rows(w)[at(w, "Roll 2") as usize].renaming, "Return renames the picked row");
    rig.settle();
    println!("shot {}", rig.shot("explorer-return-renames").display());
    n.invoke_rename_cancelled();
    rig.pump();
    assert!(rows(w).iter().all(|l| !l.renaming) && scans.join("Roll 2").is_dir());
    // Twin: once the grid takes a click, Return is the grid's again.
    shell.window.global::<Library>().invoke_tile_clicked(0, false, false);
    rig.pump();
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    rig.key(&slint::SharedString::from(slint::platform::Key::Return));
    rig.until("B in the editor", Duration::from_secs(60), || ctl.state.borrow().open_path().is_some_and(|o| o.ends_with("B.tif")));
    assert!(rows(w).iter().all(|l| !l.renaming), "Return on the grid opens the photo, renaming no folder");
    // Beside the photo, a double click on the tree's folder row renames nothing either.
    let lib = w.global::<Library>();
    let folder = tree(&lib).iter().position(|r| r.kind != 2).expect("a folder row in the tree") as i32;
    lib.invoke_row_double_clicked(folder);
    rig.pump();
    assert!(tree(&lib).iter().all(|r| !r.renaming), "the tree's double click renames nothing");
    let _ = std::fs::remove_dir_all(&d.root);
}
