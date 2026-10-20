//! The phone's presses through `oni-link`'s Host land on a store opened on a roll #8 frame, and the
//! screen the phone draws is the store's.

use oni_core::link::{Link, Move, Outward, drive};
use oni_core::{Archive, EditorStore, Surface};
use oni_link::{Command, Display, Effect, Face, Host, Identity, Letter, Machine, PairingCode, Param, Remote, Tab};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::timeout;

async fn until(display: &mut watch::Receiver<Display>, done: impl Fn(&Display) -> bool) -> Display {
    timeout(Duration::from_secs(5), async {
        loop {
            if done(&display.borrow_and_update()) {
                return display.borrow().clone();
            }
            display.changed().await.expect("remote alive");
        }
    })
    .await
    .expect("display reached the expected state")
}

#[tokio::test]
async fn presses_from_the_phone_edit_the_open_frame_and_its_screen_comes_back() {
    let Some(source) = oni_fixtures::raw_dir("link") else { return };
    let dir = std::env::temp_dir().join(format!("oni-core-link-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(source.join("DSC_6440.NEF.json"), dir.join("DSC_6440.NEF.json")).unwrap();
    let frame = dir.join("DSC_6440.NEF").to_string_lossy().into_owned();

    let mut store = EditorStore::new(Archive::new(vec![frame.clone()], |_: &Path| true));
    store.open(&frame).unwrap();
    let start = store.settings().clone();
    let store = Arc::new(Mutex::new(store));

    let (host, events) = Host::new(Identity::generate(), "Studio".into(), []);
    let addr = host.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
    let link = Arc::new(Mutex::new(Link::new(host.clone())));
    {
        let mut link = link.lock().unwrap();
        link.frame = Some((9, 38));
        link.refresh(&store.lock().unwrap());
    }
    let walks = Arc::new(Mutex::new(Vec::new()));
    let (l, s, w) = (link.clone(), store.clone(), walks.clone());
    tokio::spawn(async move {
        drive(events, &l, &s, |out| {
            if let Outward::Walk(by) = out {
                w.lock().unwrap().push(by);
            }
        })
        .await
    });

    let code = PairingCode::parse(&host.open_pairing(vec![addr]).to_uri()).unwrap();
    let remote = Remote::pair(Remote::dial(&code.addresses).await.unwrap(), &Identity::generate(), &code, "Phone")
        .await
        .unwrap();
    remote.press(Command::Step { letter: Letter::D, by: 2 });
    remote.press(Command::Step { letter: Letter::C, by: -1 });
    remote.press(Command::StepEffect { effect: Effect::Vignette, by: 3 });
    remote.press(Command::Machine(Machine::Frontier));
    remote.press(Command::Walk { by: 1 });
    let mut display = remote.display();
    let shown = until(&mut display, |d| d.state.applied == 5).await;

    let (d, c) = (start.scanner.d as i8, start.scanner.c as i8);
    assert_eq!((shown.state.clicks.d, shown.state.clicks.c), (d + 2, c - 1));
    assert!((shown.state.vignette - 0.3).abs() < 1e-6);
    assert_eq!((shown.state.machine, shown.state.frame), (Machine::Frontier, Some((9, 38))));
    assert!(shown.state.can_undo);
    {
        let store = store.lock().unwrap();
        assert_eq!((store.settings().scanner.d, store.settings().scanner.c), (start.scanner.d + 2, start.scanner.c - 1));
        assert_eq!(store.settings().scanner_machine, oni_settings::ScannerMachine::Frontier);
        assert_eq!(shown.state.revision, store.revision(), "the screen is the store's, not the prediction");
    }
    assert_eq!(*walks.lock().unwrap(), vec![1]);

    // Twin: the phone's undo takes every press back, and the screen follows the store there.
    for _ in 0..4 {
        remote.press(Command::Undo);
    }
    let back = until(&mut display, |d| d.state.applied == 9).await;
    assert_eq!((back.state.clicks.d, back.state.clicks.c), (d, c));
    let store = store.lock().unwrap();
    assert_eq!(store.settings(), &start);
    assert!(!store.can_undo(Surface::Editor));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Exposure as a host app lays it out: one row, a tenth of a stop a step, reset to zero, its track
/// two stops either side.
struct Exposure;

impl oni_core::link::Sliders for Exposure {
    fn rows(&self, store: &EditorStore) -> Vec<oni_link::Row> {
        let value = format!("{:+.2} EV", store.settings().print.exposure);
        let row = oni_link::Row { param: Param::Exposure, tab: Tab::Color, block: "Lighting".into(), label: "Exposure".into(), value, position: 0.5, modified: false };
        vec![row]
    }

    fn command(&self, _: &EditorStore, param: Param, by: Move) -> Option<oni_core::Command> {
        (param == Param::Exposure).then(|| match by {
            Move::Step(by) => oni_core::Command::Step { knob: oni_core::Knob::Exposure, by: f32::from(by) * 0.1 },
            Move::Reset => oni_core::Command::Set { knob: oni_core::Knob::Exposure, value: 0.0 },
            Move::To(at) => oni_core::Command::Set { knob: oni_core::Knob::Exposure, value: 4.0 * at - 2.0 },
        })
    }
}

#[tokio::test]
async fn the_phone_switches_the_face_and_steps_a_slider_the_host_resolves() {
    let dir = std::env::temp_dir().join(format!("oni-core-link-panel-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let frame = dir.join("frame.tif");
    std::fs::write(&frame, [0]).unwrap();
    let frame = frame.to_string_lossy().into_owned();
    let mut store = EditorStore::new(Archive::new(vec![frame.clone()], |_: &Path| false));
    store.open(&frame).unwrap();
    let start = store.settings().print.exposure;
    let ev = |v: f32| format!("{v:+.2} EV");
    let store = Arc::new(Mutex::new(store));

    let (host, events) = Host::new(Identity::generate(), "Studio".into(), []);
    let addr = host.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
    let mut link = Link::new(host.clone());
    link.sliders = Box::new(Exposure);
    link.refresh(&store.lock().unwrap());
    let link = Arc::new(Mutex::new(link));
    let faces = Arc::new(Mutex::new(Vec::new()));
    let (l, s, f) = (link.clone(), store.clone(), faces.clone());
    tokio::spawn(async move {
        drive(events, &l, &s, |out| {
            if let Outward::Face(face) = out {
                f.lock().unwrap().push(face);
            }
        })
        .await
    });

    let code = PairingCode::parse(&host.open_pairing(vec![addr]).to_uri()).unwrap();
    let remote = Remote::pair(Remote::dial(&code.addresses).await.unwrap(), &Identity::generate(), &code, "Phone")
        .await
        .unwrap();
    let mut display = remote.display();
    let first = until(&mut display, |d| !d.state.panel.rows.is_empty()).await;
    assert_eq!((first.state.panel.face, first.state.panel.rows[0].value.clone()), (Face::Pro, ev(start)));

    remote.press(Command::Face(Face::Scanner));
    remote.press(Command::Face(Face::Scanner));
    remote.press(Command::StepParam { param: Param::Exposure, by: 3 });
    let shown = until(&mut display, |d| d.state.applied == 3).await;
    assert_eq!((shown.state.panel.face, shown.state.panel.rows[0].value.clone()), (Face::Scanner, ev(start + 0.3)));
    {
        let store = store.lock().unwrap();
        assert_eq!(store.settings().face, oni_settings::Face::Scanner, "the face is the sidecar's field");
        assert!((store.settings().print.exposure - start - 0.3).abs() < 1e-6);
    }
    // Twin: the face already on is not switched again, so the host remembers one switch.
    assert_eq!(*faces.lock().unwrap(), vec![Face::Scanner]);

    remote.press(Command::Undo);
    let undone = until(&mut display, |d| d.state.applied == 4).await;
    assert_eq!(undone.state.panel.rows[0].value, ev(start), "the step was one undo step");
    remote.press(Command::ResetParam { param: Param::Texture });
    let ignored = until(&mut display, |d| d.state.applied == 5).await;
    assert_eq!(ignored.state.revision, undone.state.revision, "a slider the host does not resolve moves nothing");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A store on a plain file with the Exposure slider, driven by a Host; the phone that pairs with it.
async fn exposure_desk(name: &str) -> (Arc<Mutex<EditorStore>>, Remote, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("oni-core-link-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let frame = dir.join("frame.tif");
    std::fs::write(&frame, [0]).unwrap();
    let frame = frame.to_string_lossy().into_owned();
    let mut store = EditorStore::new(Archive::new(vec![frame.clone()], |_: &Path| false));
    store.open(&frame).unwrap();
    let store = Arc::new(Mutex::new(store));
    let (host, events) = Host::new(Identity::generate(), "Studio".into(), []);
    let addr = host.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
    let mut link = Link::new(host.clone());
    link.sliders = Box::new(Exposure);
    link.refresh(&store.lock().unwrap());
    let link = Arc::new(Mutex::new(link));
    let (l, s) = (link.clone(), store.clone());
    tokio::spawn(async move { drive(events, &l, &s, |_| {}).await });
    let code = PairingCode::parse(&host.open_pairing(vec![addr]).to_uri()).unwrap();
    let remote = Remote::pair(Remote::dial(&code.addresses).await.unwrap(), &Identity::generate(), &code, "Phone")
        .await
        .unwrap();
    (store, remote, dir)
}

/// Longer than the history's coalescing window, so two unheld changes this far apart are two steps.
const PAST_THE_WINDOW: Duration = Duration::from_millis(500);

#[tokio::test]
async fn a_phone_s_drag_lands_every_place_and_is_one_undo_step() {
    let (store, remote, dir) = exposure_desk("drag").await;
    let start = store.lock().unwrap().settings().print.exposure;
    let mut display = remote.display();
    until(&mut display, |d| !d.state.panel.rows.is_empty()).await;
    remote.slide(Param::Exposure, 0.55);
    tokio::time::sleep(PAST_THE_WINDOW).await;
    for at in [0.6, 0.7, 0.8] {
        remote.slide(Param::Exposure, at);
    }
    remote.let_go(Param::Exposure);
    let dragged = until(&mut display, |d| d.state.can_undo && d.state.panel.rows[0].value == "+1.20 EV").await;
    assert!((store.lock().unwrap().settings().print.exposure - 1.2).abs() < 1e-5, "the last place lands");
    let applied = dragged.state.applied;
    remote.press(Command::Undo);
    let undone = until(&mut display, |d| d.state.applied == applied + 1).await;
    assert_eq!(store.lock().unwrap().settings().print.exposure, start, "one undo takes the whole drag back");
    assert!(!undone.state.can_undo);

    // Twin: the same hand unheld, as two steps that far apart, is two steps of the history.
    remote.press(Command::StepParam { param: Param::Exposure, by: 2 });
    until(&mut display, |d| d.state.applied == applied + 2).await;
    tokio::time::sleep(PAST_THE_WINDOW).await;
    remote.press(Command::StepParam { param: Param::Exposure, by: 2 });
    remote.press(Command::Undo);
    let stepped = until(&mut display, |d| d.state.applied == applied + 4).await;
    assert!((store.lock().unwrap().settings().print.exposure - start - 0.2).abs() < 1e-5);
    assert!(stepped.state.can_undo);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_drag_cut_by_another_key_or_a_lost_phone_still_records_its_step() {
    let (store, remote, dir) = exposure_desk("cut").await;
    let start = store.lock().unwrap().settings().print.exposure;
    let mut display = remote.display();
    until(&mut display, |d| !d.state.panel.rows.is_empty()).await;
    remote.slide(Param::Exposure, 0.75);
    let held = until(&mut display, |d| d.state.applied == 1).await;
    assert_eq!(held.state.panel.rows[0].value, "+1.00 EV");
    assert!(!held.state.can_undo, "held, the drag is not a step yet");
    // A key with no release before it ends the drag, which becomes a step of its own.
    remote.press(Command::Step { letter: Letter::D, by: 1 });
    let cut = until(&mut display, |d| d.state.applied == 2).await;
    assert!(cut.state.can_undo);
    remote.press(Command::Undo);
    remote.press(Command::Undo);
    until(&mut display, |d| d.state.applied == 4).await;
    assert_eq!(store.lock().unwrap().settings().print.exposure, start, "two undos: the key, then the drag");

    // A phone gone mid-drag leaves no hold on the store.
    remote.slide(Param::Exposure, 0.25);
    until(&mut display, |d| d.state.applied == 5).await;
    drop(remote);
    let released = || store.lock().unwrap().can_undo(Surface::Editor);
    timeout(Duration::from_secs(15), async {
        while !released() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the hold is released once the phone is gone");
    let _ = std::fs::remove_dir_all(&dir);
}
