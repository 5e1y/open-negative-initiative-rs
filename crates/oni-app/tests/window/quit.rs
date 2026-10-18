//! `QuitGuard` on the headless window: closing writes the open frame's last settings at once, and a
//! write that fails asks before quitting, Return choosing Cancel.

use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{Confirm, Library, Store as StoreGlobal};
use oni_core::{Archive, EditorStore};
use slint::platform::WindowEvent;
use slint::ComponentHandle;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

fn written_exposure(path: &Path) -> Option<f32> {
    let sidecar = oni_settings::sidecar::url_for(path);
    sidecar.exists().then(|| Field::Exposure.knob().get(&oni_settings::sidecar::read(path, false, true).unwrap().settings))
}

pub(crate) fn closing_writes_the_last_settings_and_a_failed_write_asks_first() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the quit guard is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-quit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    common::linear_tiff(&folder.join("A.tif"), 96, 64, 0.7);
    let a = PathBuf::from(oni_library::paths::canonical(&folder.join("A.tif")));
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    let lib = w.global::<Library>();
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A open", Duration::from_secs(60), || ctl.state.borrow().open_path().is_some());
    let drag = |by: f32| {
        let g = w.global::<StoreGlobal>();
        let field = Field::Exposure.index();
        g.invoke_press(field, false);
        g.invoke_drag(field, by, 300.0, false);
        g.invoke_release(field);
        rig.pump();
        ctl.state.borrow().store().get(Field::Exposure)
    };

    // A drag, then the window closed at once: the write is not left to the pause.
    let moved = drag(30.0);
    assert!(written_exposure(&a).is_none_or(|e| (e - moved).abs() > 1e-4), "twin: the pause has not written it yet");
    rig.window.dispatch_event(WindowEvent::CloseRequested);
    rig.pump();
    assert!((written_exposure(&a).unwrap() - moved).abs() < 1e-4, "closing wrote the last value");
    assert!(!w.global::<Confirm>().get_shown());

    // The folder locked, a new value cannot be written: the close asks, and Return keeps the app open.
    let again = drag(60.0);
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o555)).unwrap();
    rig.window.dispatch_event(WindowEvent::CloseRequested);
    rig.pump();
    let confirm = w.global::<Confirm>();
    let shown = confirm.get_shown();
    let title = confirm.get_title().to_string();
    rig.key("\r");
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(shown, "a failed write asks before quitting");
    assert_eq!(title, "Your latest settings could not be saved");
    assert!(!confirm.get_shown(), "Return chose Cancel");
    assert!((ctl.state.borrow().store().get(Field::Exposure) - again).abs() < 1e-6, "the work stays open");
    let _ = std::fs::remove_dir_all(&root);
}
