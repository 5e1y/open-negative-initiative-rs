//! The phone's tabs end to end, headless, through `oni-link-ffi`: it reads each tab's sliders as the
//! panel writes them, steps one into the sidecar as one undo step, switches the face both ways, and
//! drags one as the panel's drag, one undo step for the whole drag.

use oni_app::app::{Options, Shell};
use oni_app::render::{Engine, Shared};
use oni_app::{App, Bar, Lab, Pairing, Store};
use oni_core::{Archive, EditorStore};
use oni_link_ffi::{Command, DisplayState, Face, PanelTab, Param, RemoteSession, generate_identity};
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

const WAIT: Duration = Duration::from_secs(20);

fn launch(rig: &common::Rig, engine: &Shared, root: &Path) -> Shell {
    let store = EditorStore::new(Archive::new(Vec::new(), |_: &Path| true));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine.clone(), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 2.0 });
    rig.window.set_size(slint::PhysicalSize::new(2 * 1280, 2 * 800));
    shell.window.show().unwrap();
    shell.open_folder(&root.join("Roll"));
    let ctl = shell.ctl.clone();
    rig.until("the library read", WAIT, || ctl.state.borrow().read_done());
    shell.window.global::<oni_app::Library>().invoke_tile_opened(0);
    shell
}

fn phone<T>(call: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(call)
}

fn until_shown(rig: &common::Rig, session: &RemoteSession, what: &str, done: impl Fn(&DisplayState) -> bool) -> DisplayState {
    rig.until(what, WAIT, || done(&session.current()));
    session.current()
}

fn tab(d: &DisplayState, tab: PanelTab) -> Vec<Param> {
    d.panel.rows.iter().filter(|r| r.tab == tab).map(|r| r.param).collect()
}

/// Longer than the history's coalescing window, so two unheld changes this far apart are two steps.
const PAST_THE_WINDOW: Duration = Duration::from_millis(500);

fn position(d: &DisplayState, param: Param) -> f32 {
    d.panel.rows.iter().find(|r| r.param == param).map(|r| r.position).unwrap()
}

fn value(d: &DisplayState, param: Param) -> Option<String> {
    d.panel.rows.iter().find(|r| r.param == param).map(|r| r.value.clone())
}

pub(crate) fn the_phone_s_tabs_step_the_panel_s_sliders_and_switch_the_face() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the window is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-phone-tabs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("Roll")).unwrap();
    let frame = root.join("Roll").join("A.NEF");
    std::fs::write(&frame, b"not a raw").unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let engine = Engine::shared(&gpu);
    let shell = launch(&rig, &engine, &root);
    let ctl = shell.ctl.clone();

    shell.window.global::<App>().invoke_menu("pair-phone".into());
    let pairing = shell.window.global::<Pairing>();
    rig.until("the pairing code", WAIT, || !pairing.get_uri().is_empty());
    let port = ctl.state.borrow().phone().unwrap().bound().unwrap().port();
    let uri = format!("{},127.0.0.1:{port}", pairing.get_uri());
    let session = phone(RemoteSession::pair(uri, generate_identity(), "Test Phone".into())).expect("the phone pairs");

    // The three tabs list the Pro face's sliders, each readout the panel's own text.
    let first = until_shown(&rig, &session, "the panel", |d| !d.panel.rows.is_empty());
    assert_eq!(first.panel.face, Face::Pro);
    assert!(tab(&first, PanelTab::Color).contains(&Param::Exposure));
    assert_eq!(tab(&first, PanelTab::Details).last(), Some(&Param::Texture));
    assert!(tab(&first, PanelTab::Roll).contains(&Param::DensityCeiling));
    let store = shell.window.global::<Store>();
    assert_eq!(value(&first, Param::Exposure).unwrap(), store.get_exposure_value().as_str());

    // A step lands as the panel's write: the window's readout, one undo step, the sidecar.
    let start = ctl.state.borrow().store().settings().print.exposure;
    session.press(Command::StepParam { param: Param::Exposure, by: 5 });
    let stepped = until_shown(&rig, &session, "the step", |d| d.state.applied == 1);
    let moved = ctl.state.borrow().store().settings().print.exposure;
    let (low, high) = oni_app::fields::Field::Exposure.spec().range;
    assert!((moved - start - 5.0 * (high - low) / 100.0).abs() < 1e-5, "{start} → {moved}");
    rig.pump();
    assert_eq!(value(&stepped, Param::Exposure).unwrap(), store.get_exposure_value().as_str(), "the phone reads what the panel reads");
    assert!(stepped.state.can_undo);
    rig.until("the sidecar written", WAIT, || {
        oni_settings::sidecar::read(&frame, true, false).is_ok_and(|l| (l.settings.print.exposure - moved).abs() < 1e-6)
    });
    session.press(Command::Undo);
    let undone = until_shown(&rig, &session, "the undo", |d| d.state.applied == 2);
    assert_eq!(ctl.state.borrow().store().settings().print.exposure, start);
    assert_eq!(value(&undone, Param::Exposure), value(&first, Param::Exposure));

    // The Scanner face from the phone: the bar and the Lab follow, the face is kept, Color is the keypad's.
    session.press(Command::SetFace { face: Face::Scanner });
    let scanner = until_shown(&rig, &session, "the Scanner face", |d| d.state.applied == 3);
    rig.pump();
    assert_eq!(scanner.panel.face, Face::Scanner);
    assert_eq!((shell.window.global::<Bar>().get_face(), shell.window.global::<Lab>().get_shown()), (1, true));
    assert_eq!(std::fs::read_to_string(root.join("Support").join("lastFaceScanner")).unwrap(), "1");
    assert!(!tab(&scanner, PanelTab::Color).contains(&Param::Exposure));
    assert_eq!(tab(&scanner, PanelTab::Details), tab(&first, PanelTab::Details));
    // Twin: a slider the Scanner face hides does not move from the phone.
    session.press(Command::StepParam { param: Param::Exposure, by: 5 });
    until_shown(&rig, &session, "the hidden step", |d| d.state.applied == 4);
    assert_eq!(ctl.state.borrow().store().settings().print.exposure, start);
    rig.settle();
    println!("shot {}", rig.shot("phone-scanner-face").display());

    // The bar's switch back to Pro reaches the phone, as any hand's change does.
    shell.window.global::<Bar>().invoke_act("face-pro".into());
    let pro = until_shown(&rig, &session, "Pro from the desktop", |d| d.panel.face == Face::Pro);
    assert!(tab(&pro, PanelTab::Color).contains(&Param::Exposure));

    // A drag from the phone: every place lands as the panel's drag puts it, the whole drag one undo
    // step though it outlasts the history's coalescing, and the phone reads the panel's readout.
    let f = oni_app::fields::Field::Exposure;
    let track = |s: &oni_settings::PipelineSettings| oni_app::slider_track::track_on(f, s);
    let at = position(&pro, Param::Exposure);
    session.slide(Param::Exposure, at + 0.05);
    until_shown(&rig, &session, "the first place", |d| !d.unanswered);
    std::thread::sleep(PAST_THE_WINDOW);
    for step in 1..=4 {
        session.slide(Param::Exposure, at + 0.05 + 0.04 * step as f32);
    }
    session.let_go(Param::Exposure);
    let dragged = until_shown(&rig, &session, "the drag", |d| !d.unanswered && d.state.can_undo);
    let landed = ctl.state.borrow().store().settings().print.exposure;
    let wanted = oni_app::slider_track::stored_on(f, at + 0.21, &track(ctl.state.borrow().store().settings()));
    assert!((landed - wanted).abs() < 1e-5, "{landed} where the panel's drag puts {wanted}");
    rig.pump();
    assert_eq!(value(&dragged, Param::Exposure).unwrap(), store.get_exposure_value().as_str(), "the panel's readout");
    session.press(Command::Undo);
    until_shown(&rig, &session, "the drag undone", |d| !d.unanswered);
    assert_eq!(ctl.state.borrow().store().settings().print.exposure, start, "one undo takes the whole drag back");
    // Twin: two steps as far apart are two steps, so one undo leaves the first.
    session.press(Command::StepParam { param: Param::Exposure, by: 5 });
    until_shown(&rig, &session, "a first step", |d| !d.unanswered);
    std::thread::sleep(PAST_THE_WINDOW);
    session.press(Command::StepParam { param: Param::Exposure, by: 5 });
    session.press(Command::Undo);
    until_shown(&rig, &session, "a second step undone", |d| !d.unanswered);
    assert_ne!(ctl.state.borrow().store().settings().print.exposure, start);

    session.hang_up();
    drop(store);
    drop(pairing);
    drop(ctl);
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
