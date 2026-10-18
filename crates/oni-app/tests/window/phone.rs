//! Pair Phone end to end, headless: the sheet's QR payload pairs a loopback phone, its presses step
//! the open frame through the window's store, its undo returns the value, its walk opens the next frame.

use oni_app::app::{Options, Shell};
use oni_app::{App, Pairing};
use oni_core::{Archive, EditorStore};
use oni_link::{Command, Display, Identity, Letter, PairingCode, Remote};
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::watch;

use crate::common;

/// Pumps the window until the phone's screen satisfies `done`.
fn until_shown(rig: &common::Rig, display: &mut watch::Receiver<Display>, what: &str, done: impl Fn(&Display) -> bool) -> Display {
    rig.until(what, Duration::from_secs(20), || done(&display.borrow()));
    display.borrow().clone()
}

pub(crate) fn a_phone_pairs_from_the_sheet_and_steps_the_open_frame() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the window is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-phone-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support) = (root.join("Roll"), root.join("Support"));
    std::fs::create_dir_all(&folder).unwrap();
    // Two frames LibRaw refuses: the store opens them all the same, and the walk has a neighbour.
    for name in ["A.NEF", "B.NEF"] {
        std::fs::write(folder.join(name), b"not a raw").unwrap();
    }
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), |_: &std::path::Path| true));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support: support.clone(), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 2.0 });
    rig.window.set_size(slint::PhysicalSize::new(2 * 1280, 2 * 800));
    shell.window.show().unwrap();
    shell.open_folder(&folder);
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    rig.until("the library read", Duration::from_secs(20), || ctl.state.borrow().read_done());
    w.global::<oni_app::Library>().invoke_tile_opened(0);
    assert!(ctl.state.borrow().open_path().unwrap().ends_with("A.NEF"));
    assert!(ctl.state.borrow().phone().is_none(), "nothing listens before the menu is used");

    // File > Pair Phone…: the sheet, then the QR payload once the Host listens.
    w.global::<App>().invoke_menu("pair-phone".into());
    let pairing = w.global::<Pairing>();
    assert!(pairing.get_shown());
    rig.until("the pairing code", Duration::from_secs(20), || !pairing.get_uri().is_empty());
    let code = PairingCode::parse(&pairing.get_uri()).unwrap();
    assert!(pairing.get_code().size().width > 0, "the sheet draws the code");
    rig.settle();
    println!("shot {}", rig.shot("pair-phone").display());
    let port = ctl.state.borrow().phone().unwrap().bound().unwrap().port();
    assert!(code.addresses.iter().all(|a| a.port() == port), "{:?}", code.addresses);

    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
    // The payload's addresses first, then loopback on its port, for a machine whose network address refuses itself.
    let mut addresses = code.addresses.clone();
    addresses.push(SocketAddr::from(([127, 0, 0, 1], port)));
    let remote = runtime
        .block_on(async { Remote::pair(Remote::dial(&addresses).await?, &Identity::generate(), &code, "Test Phone").await })
        .expect("the phone pairs from the payload");
    let mut display = remote.display();
    let first = until_shown(&rig, &mut display, "the first screen", |d| d.state.frame.is_some());
    assert_eq!(first.state.frame, Some((1, 2)));
    rig.until("the trusted key", Duration::from_secs(5), || oni_app::phone::has_trusted(&support));

    // A step on D moves the open frame's value through the window's store, as one undo step.
    let start = ctl.state.borrow().store().settings().scanner.d;
    let revision = ctl.state.borrow().store().revision();
    runtime.block_on(async { remote.press(Command::Step { letter: Letter::D, by: 3 }) });
    let stepped = until_shown(&rig, &mut display, "the step", |d| d.state.applied == 1);
    assert_eq!(ctl.state.borrow().store().settings().scanner.d, start + 3);
    assert_eq!(i64::from(stepped.state.clicks.d), start + 3, "the phone reads the store's value");
    assert!(stepped.state.can_undo && ctl.state.borrow().store().revision() > revision);
    // Twin: the phone's undo returns it, and the screen follows.
    runtime.block_on(async { remote.press(Command::Undo) });
    let undone = until_shown(&rig, &mut display, "the undo", |d| d.state.applied == 2);
    assert_eq!(ctl.state.borrow().store().settings().scanner.d, start);
    assert_eq!(i64::from(undone.state.clicks.d), start);

    // A walk opens the next frame and selects it; past the end it stops.
    runtime.block_on(async { remote.press(Command::Walk { by: 1 }) });
    let walked = until_shown(&rig, &mut display, "the walk", |d| d.state.frame == Some((2, 2)));
    assert!(ctl.state.borrow().open_path().unwrap().ends_with("B.NEF"), "{:?}", walked.state.frame);
    runtime.block_on(async { remote.press(Command::Walk { by: 1 }) });
    until_shown(&rig, &mut display, "the walk past the end", |d| d.state.applied == 4);
    assert!(ctl.state.borrow().open_path().unwrap().ends_with("B.NEF"));

    // Closing the sheet spends the code: a second phone on it is refused, the first stays connected.
    pairing.invoke_close();
    rig.pump();
    assert!(!pairing.get_shown());
    let late = Instant::now();
    let refused = runtime.block_on(async { Remote::pair(Remote::dial(&addresses).await?, &Identity::generate(), &code, "Late").await });
    assert!(refused.is_err(), "a closed sheet's code pairs nobody ({:?})", late.elapsed());
    assert!(display.borrow().connected);

    drop(remote);
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
