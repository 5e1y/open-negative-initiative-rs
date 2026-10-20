//! The panel over the wire: phones from protocol 2 on read the face and the tabs' sliders and move
//! them, from protocol 3 on by dragging, while a protocol 1 phone keeps reading the scanner's screen.

use std::net::SocketAddr;
use std::time::Duration;

use oni_link::noise;
use oni_link::wire::{self, OLDEST, PROTOCOL, ToHost, ToPhone};
use oni_link::{
    Channel, Clicks, Command, Display, Error, Face, Host, HostEvent, Identity, Machine, Panel, PairingCode, Param,
    Remote, Row, State, Tab,
};
use serde::Deserialize;
use tokio::sync::{mpsc, watch};
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(5);

fn row(param: Param, tab: Tab, value: &str) -> Row {
    Row {
        param,
        tab,
        block: "Lighting".into(),
        label: "Exposure".into(),
        value: value.into(),
        position: 0.5,
        modified: false,
    }
}

fn screen() -> State {
    let panel = Panel {
        face: Face::Pro,
        rows: vec![row(Param::Exposure, Tab::Color, "+0.00 EV"), row(Param::Texture, Tab::Details, "+0.00×")],
    };
    State { frame: Some((3, 12)), chrome: Some(0.0), panel, ..State::default() }
}

/// A desktop whose app answers a face or a step on Exposure by publishing the panel moved.
async fn desktop() -> (Host, SocketAddr, mpsc::UnboundedReceiver<Command>) {
    let (host, mut events) = Host::new(Identity::generate(), "Studio".into(), []);
    let addr = host.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
    let (seen, log) = mpsc::unbounded_channel();
    let app = host.clone();
    tokio::spawn(async move {
        let mut state = screen();
        app.publish(state.clone());
        while let Some(event) = events.next().await {
            let HostEvent::Command { command, .. } = event else { continue };
            match command {
                Command::Face(face) => state.panel.face = face,
                Command::StepParam { param: Param::Exposure, by } => {
                    state.panel.rows[0].value = format!("{:+.2} EV", f32::from(by) * 0.1);
                }
                _ => {}
            }
            state.revision += 1;
            app.publish(state.clone());
            let _ = seen.send(command);
        }
    });
    (host, addr, log)
}

async fn until(display: &mut watch::Receiver<Display>, done: impl Fn(&Display) -> bool) -> Display {
    timeout(WAIT, async {
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
async fn the_phone_reads_the_panel_switches_the_face_and_steps_a_slider() {
    let (host, addr, mut log) = desktop().await;
    let code = PairingCode::parse(&host.open_pairing(vec![addr]).to_uri()).unwrap();
    let remote = Remote::pair(Remote::dial(&code.addresses).await.unwrap(), &Identity::generate(), &code, "Phone")
        .await
        .unwrap();
    let mut display = remote.display();
    let first = until(&mut display, |d| !d.state.panel.rows.is_empty()).await;
    assert_eq!(first.state.panel, screen().panel);
    assert_eq!(first.state.panel.tab(Tab::Details).map(|r| r.param).collect::<Vec<_>>(), [Param::Texture]);

    let predicted = remote.press(Command::Face(Face::Scanner));
    assert_eq!(predicted.panel.face, Face::Scanner, "the face shows before the desktop answers");
    let stepped = remote.press(Command::StepParam { param: Param::Exposure, by: 3 });
    assert_eq!(stepped.panel.rows[0].value, "+0.00 EV", "a readout is the desktop's to write");
    let settled = until(&mut display, |d| d.state.applied == 2).await;
    assert_eq!((settled.state.panel.face, settled.state.panel.rows[0].value.as_str()), (Face::Scanner, "+0.30 EV"));
    assert_eq!(log.recv().await, Some(Command::Face(Face::Scanner)));
    assert_eq!(log.recv().await, Some(Command::StepParam { param: Param::Exposure, by: 3 }));
}

/// The first protocol's messages, as a phone built before the panel decodes them.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
enum OldToPhone {
    Welcome { protocol: u16, name: String, import: Option<wire::Import> },
    State(OldState),
    Preview { revision: u64, jpeg: Vec<u8> },
    Pong { sent: u64 },
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OldState {
    revision: u64,
    applied: u64,
    frame: Option<(u16, u16)>,
    machine: Machine,
    channel: Channel,
    clicks: Clicks,
    contrast: f32,
    vignette: f32,
    chrome: Option<f32>,
    remeasure: bool,
    can_undo: bool,
    can_redo: bool,
}

/// Pairs over a raw session saying `protocol`; the Welcome and the next message as `T` decodes them.
async fn hello<T: serde::de::DeserializeOwned>(protocol: u16) -> (Result<ToPhone, Error>, Result<T, Error>) {
    let (host, addr, _log) = desktop().await;
    let code = host.open_pairing(vec![addr]);
    let stream = Remote::dial(&[addr]).await.unwrap();
    let secret = Some(code.secret);
    let (mut reader, mut writer) = noise::initiate(stream, &Identity::generate(), code.host, secret).await.unwrap();
    writer.send(&ToHost::Hello { protocol, name: "Old Phone".into() }).await.unwrap();
    let welcome = timeout(WAIT, reader.recv::<ToPhone>()).await.unwrap();
    let next = timeout(WAIT, reader.recv::<T>()).await.unwrap();
    (welcome, next)
}

#[tokio::test]
async fn a_protocol_1_phone_is_answered_in_its_own_version_without_the_panel() {
    assert_eq!((OLDEST, PROTOCOL), (1, 3));
    let (welcome, state) = hello::<OldToPhone>(1).await;
    assert!(matches!(welcome, Ok(ToPhone::Welcome { protocol: 1, .. })), "{welcome:?}");
    let Ok(OldToPhone::State(state)) = state else { panic!("the old phone reads its screen: {state:?}") };
    assert_eq!(state.frame, Some((3, 12)));
    // Twin: a phone of this protocol is sent the panel, which the old decoding cannot read as a screen.
    let (welcome, screen) = hello::<ToPhone>(PROTOCOL).await;
    assert!(matches!(welcome, Ok(ToPhone::Welcome { protocol: 3, .. })));
    assert!(matches!(screen, Ok(ToPhone::Screen { panel, .. }) if panel.rows.len() == 2));
    // A protocol 2 phone, which steps its sliders and never drags them, still reads the panel.
    let (welcome, screen) = hello::<ToPhone>(2).await;
    assert!(matches!(welcome, Ok(ToPhone::Welcome { protocol: 2, .. })));
    assert!(matches!(screen, Ok(ToPhone::Screen { panel, .. }) if panel.rows.len() == 2));
    let (_, old) = hello::<OldToPhone>(PROTOCOL).await;
    assert!(!matches!(old, Ok(OldToPhone::State(_))));
}

#[tokio::test]
async fn a_phone_older_than_the_oldest_protocol_is_turned_away() {
    let (welcome, next) = hello::<ToPhone>(0).await;
    assert!(matches!(welcome, Ok(ToPhone::Welcome { protocol: PROTOCOL, .. })), "it learns the desktop's version");
    assert!(next.is_err(), "and is hung up on");
}

#[test]
fn the_screen_s_encoding_leaves_the_panel_out() {
    let bare = State { frame: Some((3, 12)), chrome: Some(0.0), ..State::default() };
    assert_eq!(wire::encode(&screen()).unwrap(), wire::encode(&bare).unwrap());
    // Twin: the screen message of protocol 2 does carry it.
    let with = ToPhone::Screen { state: screen(), panel: screen().panel };
    let without = ToPhone::Screen { state: bare, panel: Panel::default() };
    assert_ne!(wire::encode(&with).unwrap(), wire::encode(&without).unwrap());
}

/// A desktop that answers each press only once the test opens its gate, as a slow one would.
async fn gated() -> (Remote, mpsc::UnboundedSender<()>, mpsc::UnboundedReceiver<Command>) {
    let (host, mut events) = Host::new(Identity::generate(), "Studio".into(), []);
    let addr = host.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
    let (gate, mut opened) = mpsc::unbounded_channel::<()>();
    let (seen, log) = mpsc::unbounded_channel();
    let app = host.clone();
    tokio::spawn(async move {
        let mut state = screen();
        app.publish(state.clone());
        while let Some(event) = events.next().await {
            let HostEvent::Command { command, .. } = event else { continue };
            let _ = seen.send(command);
            if opened.recv().await.is_none() {
                break;
            }
            state = state.predicted(command);
            state.revision += 1;
            app.publish(state.clone());
        }
    });
    let code = PairingCode::parse(&host.open_pairing(vec![addr]).to_uri()).unwrap();
    let remote = Remote::pair(Remote::dial(&code.addresses).await.unwrap(), &Identity::generate(), &code, "Phone")
        .await
        .unwrap();
    (remote, gate, log)
}

async fn next(log: &mut mpsc::UnboundedReceiver<Command>) -> Command {
    timeout(WAIT, log.recv()).await.expect("a press arrived").expect("desktop alive")
}

fn place(command: Command) -> Option<f32> {
    match command {
        Command::SetParam { param: Param::Exposure, position } => Some(position),
        _ => None,
    }
}

#[tokio::test]
async fn a_drag_keeps_one_place_on_the_wire_and_its_release_follows_the_last() {
    let (remote, gate, mut log) = gated().await;
    let mut display = remote.display();
    until(&mut display, |d| !d.state.panel.rows.is_empty()).await;
    for at in [0.1, 0.2, 0.3] {
        remote.slide(Param::Exposure, at);
    }
    let shown = remote.slide(Param::Exposure, 0.4);
    assert_eq!(shown.panel.rows[0].position, 0.4, "the knob follows the hand before the desktop answers");
    assert_eq!(place(next(&mut log).await), Some(0.1));
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(log.try_recv().is_err(), "nothing more flies while the first place is unanswered");
    gate.send(()).unwrap();
    assert_eq!(place(next(&mut log).await), Some(0.4), "the latest place, the ones between skipped");
    remote.slide(Param::Exposure, 0.5);
    remote.let_go(Param::Exposure);
    for _ in 0..3 {
        gate.send(()).unwrap();
    }
    assert_eq!(place(next(&mut log).await), Some(0.5), "the last place goes before the release");
    assert_eq!(next(&mut log).await, Command::ReleaseParam { param: Param::Exposure });
    let settled = until(&mut display, |d| d.state.applied == 4).await;
    assert_eq!(settled.state.panel.rows[0].position, 0.5);

    // Twin: pressed as keys, the same places all fly, one per press.
    for at in [0.6, 0.7, 0.8] {
        remote.press(Command::SetParam { param: Param::Exposure, position: at });
        gate.send(()).unwrap();
    }
    for at in [0.6, 0.7, 0.8] {
        assert_eq!(place(next(&mut log).await), Some(at));
    }
}
