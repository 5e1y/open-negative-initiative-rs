//! A phone's step reaches the canvas whether the panel shows or is folded away.

use oni_app::app::{Options, Shell};
use oni_app::render::Engine;
use oni_app::{App, Pairing};
use oni_core::{Archive, EditorStore};
use oni_link_ffi::{Command, Param, RemoteSession, generate_identity};
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

const WAIT: Duration = Duration::from_secs(20);

fn phone<T>(call: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(call)
}

pub(crate) fn a_phone_step_reaches_the_canvas_with_the_panel_folded() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the window is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-phone-canvas-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    common::linear_tiff(&root.join("Roll").join("A.tif"), 96, 64, 0.6);
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), |_: &Path| false));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), phone: true, ..Options::default() };
    let shell = Shell::new(Box::new(store), Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    shell.open_folder(&root.join("Roll"));
    let ctl = shell.ctl.clone();
    rig.until("the library read", WAIT, || ctl.state.borrow().read_done());
    shell.window.global::<oni_app::Library>().invoke_tile_opened(0);
    rig.until("the frame rendered", WAIT, || ctl.state.borrow().renders() >= 1);

    shell.window.global::<App>().invoke_menu(oni_app::Act::PairPhone);
    let pairing = shell.window.global::<Pairing>();
    rig.until("the pairing code", WAIT, || !pairing.get_uri().is_empty());
    let port = ctl.state.borrow().phone().unwrap().bound().unwrap().port();
    let uri = format!("{},127.0.0.1:{port}", pairing.get_uri());
    let session = phone(RemoteSession::pair(uri, generate_identity(), "Test Phone".into())).expect("the phone pairs");
    ctl.act(oni_app::Act::ClosePairing);

    for folded in [false, true] {
        if folded {
            ctl.act(oni_app::Act::HidePanel);
            rig.pump();
            assert!(shell.window.global::<App>().get_panel_hidden());
        }
        let renders = ctl.state.borrow().renders();
        let applied = session.current().state.applied;
        session.press(Command::StepParam { param: Param::Exposure, by: 5 });
        rig.until("the step applied", WAIT, || session.current().state.applied > applied);
        let revision = ctl.state.borrow().store().revision();
        rig.until(&format!("the step rendered (panel folded: {folded})"), WAIT, || {
            let s = ctl.state.borrow();
            s.renders() > renders && s.rendered() >= revision
        });
    }
    session.hang_up();
    drop(pairing);
    drop(ctl);
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
