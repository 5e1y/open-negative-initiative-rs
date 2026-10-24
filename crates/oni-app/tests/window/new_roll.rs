//! New Roll moves originals, so it asks first: the folder it makes named, the files it moves counted
//! with what other apps keep beside them; photos of two folders are refused with the reason.

use oni_app::app::{Options, Shell};
use oni_app::{Confirm, Told};
use oni_core::{Archive, EditorStore};
use oni_library::navigator::Computer;
use oni_library::paths;
use slint::ComponentHandle;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

fn gpu(what: &str) -> Option<oni_app::render::Device> {
    oni_app::render::Device::open().map_err(|e| eprintln!("{e}: {what} is not exercised")).ok()
}

fn told(shell: &Shell) -> Option<String> {
    let t = shell.window.global::<Told>();
    t.get_shown().then(|| t.get_message().to_string())
}

fn listed(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder).map(|l| l.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    names.sort();
    names
}

pub(crate) fn a_new_roll_asks_names_the_folder_and_counts_files() {
    let Some(gpu) = gpu("New Roll's confirm") else { return };
    let root = std::env::temp_dir().join(format!("oni-app-new-roll-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for file in ["Home/Scans/A.tif", "Home/Scans/B.tif", "Home/Scans/Roll 2/C.tif"] {
        common::linear_tiff(&root.join(file), 48, 32, 0.5);
    }
    std::fs::write(root.join("Home/Scans/A.tif.xmp"), b"darktable").unwrap();
    std::fs::write(root.join("Home/Scans/B.tif.pp3"), b"rawtherapee").unwrap();
    std::fs::create_dir_all(root.join("Volumes")).unwrap();
    let root = PathBuf::from(paths::canonical(&root));
    let scans = root.join("Home/Scans");
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let computer = Computer { home: paths::canonical(&root.join("Home")), volumes: paths::canonical(&root.join("Volumes")) };
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), computer, ..Options::default() };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    shell.open_folder(&scans);
    rig.until("the photos read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().order().len() == 3);
    let lib = shell.window.global::<oni_app::Library>();
    let confirm = shell.window.global::<Confirm>();
    let at = |name: &str| ctl.state.borrow().order().iter().position(|p| p.ends_with(&format!("/{name}"))).unwrap() as i32;
    let dated = oni_library::roll_folder::dated(oni_settings::Date::now());

    // Photos of two folders: refused before anything is asked or moved, the reason said.
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(2, false, true);
    rig.pump();
    ctl.act(oni_app::Act::NewRoll);
    rig.pump();
    let refused = told(&shell).expect("the refusal is said");
    assert!(refused.contains("these lie in 2 folders"), "{refused}");
    assert!(!confirm.get_shown() && !scans.join(&dated).exists(), "nothing asked, nothing made");
    shell.window.global::<Told>().invoke_closed();

    // A and B of one folder: the confirm names the folder it makes and counts four files.
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    lib.invoke_tile_clicked(at("A.tif"), false, false);
    lib.invoke_tile_clicked(at("B.tif"), true, false);
    rig.pump();
    ctl.act(oni_app::Act::NewRoll);
    rig.until("New Roll's confirm", Duration::from_secs(10), || confirm.get_shown());
    assert_eq!(confirm.get_title().as_str(), "Move 2 Photos into a New Roll?");
    let message = confirm.get_message().to_string();
    assert!(message.contains(&format!("“{dated}” in “Scans”")) && message.contains("moves 4 files into it"), "{message}");
    assert_eq!(confirm.get_key().as_str(), "MOVE 2 PHOTOS");
    rig.settle();
    println!("shot {}", rig.shot("new-roll-confirm").display());
    // Twin: Cancel moves nothing and makes no folder.
    confirm.invoke_answered(false);
    rig.settle();
    assert!(!scans.join(&dated).exists() && scans.join("A.tif").exists(), "cancelled, nothing moved");

    ctl.act(oni_app::Act::NewRoll);
    rig.roll_confirmed(&ctl, &shell.window);
    let roll = scans.join(&dated);
    assert_eq!(listed(&roll), [".oni-roll.json", "A.tif", "A.tif.xmp", "B.tif", "B.tif.pp3"]);
    assert!(scans.join("Roll 2/C.tif").exists() && listed(&scans) == [dated.as_str(), "Roll 2"], "{:?}", listed(&scans));
    assert_eq!(told(&shell), None, "a whole New Roll says nothing");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
