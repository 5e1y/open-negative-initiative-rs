//! Pair Phone end to end, headless, through `oni-link-ffi`, the code the phone app runs: the sheet's
//! QR pairs a loopback phone whose presses step the open frame, then it reconnects after a restart.

use oni_app::app::{Options, Shell};
use oni_app::render::{Engine, Shared};
use oni_app::{App, Pairing};
use oni_core::{Archive, EditorStore};
use oni_link::PairingCode;
use oni_link_ffi::{Command, Frame, LinkError, RemoteSession, generate_identity, parse_pairing_code};
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

const WAIT: Duration = Duration::from_secs(20);

/// The app as launched: the roll's folder read and its first frame opened.
fn launch(rig: &common::Rig, engine: &Shared, root: &Path) -> Shell {
    let store = EditorStore::new(Archive::new(Vec::new(), |_: &Path| true));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), phone: true, ..Options::default() };
    let shell = Shell::new(Box::new(store), engine.clone(), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 2.0 });
    rig.window.set_size(slint::PhysicalSize::new(2 * 1280, 2 * 800));
    shell.window.show().unwrap();
    shell.open_folder(&root.join("Roll"));
    let ctl = shell.ctl.clone();
    rig.until("the library read", WAIT, || ctl.state.borrow().read_done());
    shell.window.global::<oni_app::Library>().invoke_tile_opened(0);
    assert!(ctl.state.borrow().open_path().unwrap().ends_with("A.NEF"));
    shell
}

/// Runs one of the phone's calls, which await the link's own runtime, to its end.
fn phone<T>(call: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(call)
}

/// Pumps the window until the phone's screen satisfies `done`.
fn until_shown(rig: &common::Rig, session: &RemoteSession, what: &str, done: impl Fn(&oni_link_ffi::DisplayState) -> bool) -> oni_link_ffi::DisplayState {
    rig.until(what, WAIT, || done(&session.current()));
    session.current()
}

/// The sheet's code with loopback last, for a machine whose network address refuses itself.
fn dialable(uri: &str, port: u16) -> String {
    format!("{uri},127.0.0.1:{port}")
}

pub(crate) fn a_phone_pairs_from_the_sheet_and_steps_the_open_frame() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the window is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-phone-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let support = root.join("Support");
    std::fs::create_dir_all(root.join("Roll")).unwrap();
    // Two frames LibRaw refuses: the store opens them all the same, and the walk has a neighbour.
    for name in ["A.NEF", "B.NEF"] {
        std::fs::write(root.join("Roll").join(name), b"not a raw").unwrap();
    }
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let engine = Engine::shared(&gpu);
    let shell = launch(&rig, &engine, &root);
    let ctl = shell.ctl.clone();
    assert!(ctl.state.borrow().phone().is_none(), "nothing listens before the menu is used");

    // File > Pair Phone…: the sheet, then the code once the Host listens; its pixels are the URI's QR.
    shell.window.global::<App>().invoke_menu(oni_app::Act::PairPhone);
    let pairing = shell.window.global::<Pairing>();
    assert!(pairing.get_shown());
    rig.until("the pairing code", WAIT, || !pairing.get_uri().is_empty());
    let uri = pairing.get_uri().to_string();
    let drawn = pairing.get_code().to_rgba8().expect("the sheet draws the code in RGBA8");
    let (pixels, side) = oni_app::phone::qr(&uri).unwrap();
    assert_eq!((drawn.width() as usize, drawn.as_bytes()), (side, &pixels[..]), "the sheet's QR encodes its URI");
    // Twin: another code's pixels differ from the drawn ones.
    assert_ne!(oni_app::phone::qr(&uri.replace("v=1", "v=2")).unwrap().0, drawn.as_bytes());
    rig.settle();
    println!("shot {}", rig.shot("pair-phone").display());

    // The phone reads the code as its scanner hands it over.
    let info = parse_pairing_code(uri.clone()).expect("the phone parses the sheet's code");
    let port = ctl.state.borrow().phone().unwrap().bound().unwrap().port();
    assert!(!info.addresses.is_empty() && info.addresses.iter().all(|a| a.ends_with(&format!(":{port}"))), "{:?}", info.addresses);
    assert_eq!(info.name, oni_app::phone::device_name());

    // Twin first: a code whose secret is wrong pairs nothing and leaves the code open.
    let mut wrong = PairingCode::parse(&uri).unwrap();
    wrong.secret[0] ^= 1;
    let identity = generate_identity();
    let tampered = phone(RemoteSession::pair(dialable(&wrong.to_uri(), port), identity.clone(), "Test Phone".into()));
    assert!(tampered.is_err() && !oni_app::phone::has_trusted(&support), "a wrong secret is refused");

    let session = phone(RemoteSession::pair(dialable(&uri, port), identity.clone(), "Test Phone".into())).expect("the phone pairs from the code");
    assert_eq!((session.host_name(), session.host_key()), (info.name.clone(), info.host_key.clone()));
    let first = until_shown(&rig, &session, "the first screen", |d| d.state.frame.is_some());
    assert_eq!(first.state.frame, Some(Frame { index: 1, count: 2 }));
    rig.until("the trusted key", WAIT, || oni_app::phone::has_trusted(&support));
    rig.until("the sheet names the phone", WAIT, || pairing.get_phones().contains("Test Phone"));
    assert!(pairing.get_can_forget());

    // A step on D moves the open frame's value through the window's store, as one undo step.
    let start = ctl.state.borrow().store().settings().scanner.d;
    let revision = ctl.state.borrow().store().revision();
    session.press(Command::Step { letter: oni_link_ffi::Letter::D, by: 3 });
    let stepped = until_shown(&rig, &session, "the step", |d| d.state.applied == 1);
    assert_eq!(ctl.state.borrow().store().settings().scanner.d, start + 3);
    assert_eq!(i64::from(stepped.state.clicks.d), start + 3, "the phone reads the store's value");
    assert!(stepped.state.can_undo && ctl.state.borrow().store().revision() > revision);
    // Twin: the phone's undo returns it, and the screen follows.
    session.press(Command::Undo);
    let undone = until_shown(&rig, &session, "the undo", |d| d.state.applied == 2);
    assert_eq!(ctl.state.borrow().store().settings().scanner.d, start);
    assert_eq!(i64::from(undone.state.clicks.d), start);

    // A walk opens the next frame and selects it; past the end it stops.
    session.press(Command::Walk { by: 1 });
    until_shown(&rig, &session, "the walk", |d| d.state.frame == Some(Frame { index: 2, count: 2 }));
    assert!(ctl.state.borrow().open_path().unwrap().ends_with("B.NEF"));
    session.press(Command::Walk { by: 1 });
    until_shown(&rig, &session, "the walk past the end", |d| d.state.applied == 4);
    assert!(ctl.state.borrow().open_path().unwrap().ends_with("B.NEF"));

    // Closing the sheet spends the code: a second phone on it is refused, the first stays connected.
    pairing.invoke_close();
    rig.pump();
    assert!(!pairing.get_shown());
    let late = phone(RemoteSession::pair(dialable(&uri, port), generate_identity(), "Late".into()));
    assert!(late.is_err(), "a closed sheet's code pairs nobody");
    assert!(session.current().connected);

    // A restart: the paired phone finds the desktop listening at launch, on the code's port, without a code.
    session.hang_up();
    drop(pairing);
    drop(ctl);
    drop(shell);
    let shell = launch(&rig, &engine, &root);
    let ctl = shell.ctl.clone();
    assert!(ctl.state.borrow().phone().is_some(), "a phone paired before starts the link at launch");
    rig.until("the link listening", WAIT, || ctl.state.borrow().phone().and_then(|p| p.bound()).is_some());
    assert_eq!(ctl.state.borrow().phone().unwrap().bound().unwrap().port(), port);
    let mut addresses = info.addresses.clone();
    addresses.push(format!("127.0.0.1:{port}"));
    let again = phone(RemoteSession::connect(addresses.clone(), info.host_key.clone(), identity.clone(), "Test Phone".into()))
        .expect("the trusted phone reconnects without the code");
    until_shown(&rig, &again, "the screen after the restart", |d| d.connected && d.state.frame == Some(Frame { index: 1, count: 2 }));
    again.press(Command::Step { letter: oni_link_ffi::Letter::D, by: 1 });
    until_shown(&rig, &again, "the step after the restart", |d| d.state.applied == 1);
    assert_eq!(ctl.state.borrow().store().settings().scanner.d, start + 1);
    // Twin: a phone the desktop never paired is refused at the same address.
    let stranger = phone(RemoteSession::connect(addresses.clone(), info.host_key.clone(), generate_identity(), "Stranger".into()));
    assert!(matches!(stranger, Err(LinkError::Refused)), "a stranger is refused");

    // Forget Phones cuts the paired phone at once; it then needs a new code.
    shell.window.global::<App>().invoke_menu(oni_app::Act::PairPhone);
    let pairing = shell.window.global::<Pairing>();
    rig.until("the sheet's forget key", WAIT, || pairing.get_can_forget());
    pairing.invoke_forget();
    until_shown(&rig, &again, "the phone cut", |d| !d.connected);
    assert!(!oni_app::phone::has_trusted(&support) && !pairing.get_can_forget());
    let forgotten = phone(RemoteSession::connect(addresses, info.host_key.clone(), identity, "Test Phone".into()));
    assert!(matches!(forgotten, Err(LinkError::Refused)), "a forgotten phone is refused");

    drop(again);
    drop(pairing);
    drop(ctl);
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}

/// The switch off hides the link's key, menu and Settings page and starts nothing, a phone paired
/// before included; on, each is back and the paired phone finds the desktop listening.
pub(crate) fn the_switch_off_hides_the_link_and_listens_nowhere() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the window is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-phone-switch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let support = root.join("Support");
    oni_app::phone::trust(&support, oni_link::Identity::generate().public()).unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let engine = Engine::shared(&gpu);
    let shell = |phone: bool| {
        let store = EditorStore::new(Archive::new(Vec::new(), |_: &Path| true));
        let options = Options { support: support.clone(), caches: root.join("Caches"), picker: Box::new(|| None), phone, ..Options::default() };
        let shell = Shell::new(Box::new(store), engine.clone(), options).unwrap();
        shell.window.show().unwrap();
        rig.pump();
        shell
    };

    let off = shell(false);
    let pairing = off.window.global::<Pairing>();
    assert!(!pairing.get_available(), "no key, menu item or Settings page names the phone");
    assert!(off.ctl.state.borrow().phone().is_none(), "a paired phone starts nothing while the switch is off");
    off.window.global::<App>().invoke_menu(oni_app::Act::PairPhone);
    rig.pump();
    assert!(!pairing.get_shown() && off.ctl.state.borrow().phone().is_none(), "Pair Phone opens nothing");
    drop(pairing);
    drop(off);

    // Twin: on, the entries are back and the paired phone finds the desktop listening at launch.
    let on = shell(true);
    assert!(on.window.global::<Pairing>().get_available());
    let ctl = on.ctl.clone();
    rig.until("the link listening", WAIT, || ctl.state.borrow().phone().and_then(|p| p.bound()).is_some());
    drop(ctl);
    drop(on);
    let _ = std::fs::remove_dir_all(&root);
}
