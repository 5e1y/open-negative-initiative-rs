//! The face turned is a view and never a step: an undo reaches past a turn to the hand's last step
//! and shows it on the face it was taken on, the face then kept for the next launch.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{Bar, Lab, Library, Menus, Store};
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

pub(crate) fn an_undo_reaches_past_a_face_turn_and_shows_its_step_on_its_own_face() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the face turn is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-face-turn-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Roll"), root.join("Support"), root.join("Caches"));
    for (i, n) in ["A", "B"].iter().enumerate() {
        common::graded_tiff(&folder.join(format!("{n}.tif")), 150, 100, 0.3 + 0.2 * i as f32);
    }
    let folder = PathBuf::from(oni_library::paths::canonical(&folder));
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1728, 1017));
    let store = EditorStore::new(Archive::new(Vec::new(), |_: &Path| false));
    let options = Options { support: support.clone(), caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    shell.window.show().unwrap();
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    w.global::<Library>().invoke_tile_opened(0);
    rig.pump();
    let (bar, lab, menus) = (w.global::<Bar>(), w.global::<Lab>(), w.global::<Menus>());
    let settings = || ctl.state.borrow().store().settings().clone();
    let face = |name: &str| common::click(&rig, w, common::the(w, Role::Tab, name).middle());
    let remembered = || std::fs::read_to_string(support.join("lastFaceScanner")).unwrap_or_default();

    // Pro's Contrast moved, then the face turned to the Scanner and back: the undo still names it.
    let rest = settings().print.contrast;
    w.global::<Store>().invoke_step(Field::Contrast.index(), 1);
    rig.pump();
    assert_ne!(settings().print.contrast, rest, "the step moved nothing, so the check sees nothing");
    face("Scanner");
    assert!(lab.get_shown() && bar.get_face() == 1);
    face("Pro");
    assert!(!lab.get_shown() && bar.get_face() == 0);
    assert_eq!(menus.get_titles().undo.as_str(), "Undo Contrast", "the turns opened no step");
    bar.invoke_act(oni_app::Act::Undo);
    rig.pump();
    assert_eq!(settings().print.contrast, rest, "one undo reaches the Contrast past both turns");
    // Twin: a turn as a step would be the one taken back, with the Contrast still to undo.
    assert!(!bar.get_can_undo());

    // A Scanner key, then Pro again: the undo shows its step on the face it was pressed on.
    face("Scanner");
    lab.invoke_press(0, 3);
    rig.pump();
    assert_eq!(settings().scanner.d, 3);
    face("Pro");
    assert_eq!(remembered(), "0");
    bar.invoke_act(oni_app::Act::Undo);
    rig.pump();
    assert_eq!((settings().scanner.d, bar.get_face(), lab.get_shown()), (0, 1, true), "the view turns to the step's face");
    assert_eq!(remembered(), "1", "the face shown is the one the next launch opens on");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
