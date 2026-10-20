use std::future::Future;
use std::net::{IpAddr, Ipv4Addr};
use std::pin::Pin;
use std::time::Duration;

use oni_link::{Host, HostEvent, HostEvents, State};
use oni_localsend::{AcceptPolicy, Decision, Event, FileMeta, Receiver, ReceiverConfig};
use tokio::sync::mpsc;
use tokio::time::timeout;

use super::*;

const WAIT: Duration = Duration::from_secs(5);

struct Recorder(mpsc::UnboundedSender<DisplayState>);

impl DisplayListener for Recorder {
    fn on_display(&self, display: DisplayState) {
        let _ = self.0.send(display);
    }
}

/// The desktop app: applies every command to its screen, walks frames itself, and publishes.
fn desk(host: Host, mut events: HostEvents) -> mpsc::UnboundedReceiver<HostEvent> {
    let (seen, heard) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let row = oni_link::Row {
            param: oni_link::Param::Exposure,
            tab: oni_link::Tab::Color,
            block: "Lighting".into(),
            label: "Exposure".into(),
            value: "+0.00 EV".into(),
            position: 0.5,
            modified: false,
        };
        let panel = oni_link::Panel { face: oni_link::Face::Pro, rows: vec![row] };
        let mut state = State { frame: Some((7, 36)), chrome: Some(0.0), panel, ..State::default() };
        host.publish(state.clone());
        while let Some(event) = events.next().await {
            if let HostEvent::Command { command, .. } = &event {
                state = state.predicted(*command);
                if let oni_link::Command::StepParam { by, .. } = command {
                    state.panel.rows[0].value = format!("{:+.2} EV", f32::from(*by) / 10.0);
                }
                if let oni_link::Command::Walk { by } = command {
                    state.frame =
                        state.frame.map(|(index, count)| (index.saturating_add_signed(i16::from(*by)), count));
                }
                state.revision += 1;
                host.publish(state.clone());
            }
            let _ = seen.send(event);
        }
    });
    heard
}

async fn start() -> (Host, PairingCode, mpsc::UnboundedReceiver<HostEvent>) {
    let (host, events) = Host::new(Identity::generate(), "Darkroom".into(), []);
    let address = host.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
    let code = host.open_pairing(vec![address]);
    let heard = desk(host.clone(), events);
    (host, code, heard)
}

async fn until<T>(rx: &mut mpsc::UnboundedReceiver<T>, mut wanted: impl FnMut(&T) -> bool) -> T {
    timeout(WAIT, async {
        loop {
            let item = rx.recv().await.expect("channel open");
            if wanted(&item) {
                return item;
            }
        }
    })
    .await
    .expect("arrived in time")
}

#[test]
fn parses_the_desktops_code() {
    let code = PairingCode {
        host: Identity::generate().public(),
        secret: [7; 32],
        name: "Darkroom".into(),
        addresses: vec!["192.168.1.20:47700".parse().unwrap(), "[fe80::1]:47700".parse().unwrap()],
    };
    let info = parse_pairing_code(code.to_uri()).unwrap();
    assert_eq!(info.host_key, code.host.0.to_vec());
    assert_eq!(info.name, "Darkroom");
    assert_eq!(info.addresses, ["192.168.1.20:47700", "[fe80::1]:47700"]);
    let truncated = code.to_uri().replace("&k=", "&k=AAAA");
    assert!(matches!(parse_pairing_code(truncated), Err(LinkError::BadCode { .. })));
    assert!(matches!(parse_pairing_code("https://example.com".into()), Err(LinkError::BadCode { .. })));
}

#[test]
fn identity_round_trips_and_refuses_junk() {
    let identity = generate_identity();
    assert_eq!(identity.len(), 64);
    assert_eq!(identity_public_key(identity.clone()).unwrap(), identity[32..].to_vec());
    assert!(identity_public_key(identity[..63].to_vec()).is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn pairs_presses_and_hears_the_desktop() {
    let (_host, code, mut heard) = start().await;
    let identity = generate_identity();
    let session = RemoteSession::pair(code.to_uri(), identity.clone(), "Phone".into()).await.unwrap();
    assert_eq!(session.host_name(), "Darkroom");
    assert_eq!(session.host_key(), code.host.0.to_vec());

    let (tx, mut shown) = mpsc::unbounded_channel();
    session.set_listener(Box::new(Recorder(tx)));
    let paired = until(&mut heard, |e| matches!(e, HostEvent::Paired { .. })).await;
    assert_eq!(
        paired,
        HostEvent::Paired { phone: PublicKey(identity[32..].try_into().unwrap()), name: "Phone".into() }
    );
    until(&mut shown, |d| d.connected && d.state.frame == Some(Frame { index: 7, count: 36 })).await;

    let predicted = session.press(Command::Step { letter: Letter::C, by: 3 });
    assert_eq!(predicted.clicks.c, 3, "the press shows before the desktop answers");
    session.press(Command::Walk { by: 1 });
    let predicted = session.press(Command::StepEffect { effect: Effect::Contrast, by: 2 });
    assert!((predicted.contrast - 0.1).abs() < 1e-6);
    assert_eq!(predicted.frame, Some(Frame { index: 7, count: 36 }), "a walk is the desktop's to show");

    let settled = until(&mut shown, |d| d.state.applied == 3).await;
    assert_eq!(settled.state.frame, Some(Frame { index: 8, count: 36 }));
    assert_eq!(settled.state.clicks, Clicks { c: 3, ..Clicks::default() });
    assert_eq!(settled.state.revision, 3);
    assert!((settled.state.contrast - 0.1).abs() < 1e-6);
    assert_eq!(session.current().state, settled.state);
    assert!(!session.can_send_files());
    assert!(matches!(session.send_files(vec![]).await, Err(LinkError::ImportUnavailable)));

    session.hang_up();
    until(&mut heard, |e| matches!(e, HostEvent::Disconnected { .. })).await;
    assert!(!session.current().connected);
    assert_eq!(session.press(Command::Undo), settled.state, "a closed session sends nothing");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_paired_phone_reconnects_by_key_and_a_stranger_does_not() {
    let (_host, code, mut heard) = start().await;
    let identity = generate_identity();
    let first = RemoteSession::pair(code.to_uri(), identity.clone(), "Phone".into()).await.unwrap();
    let key = first.host_key();
    first.hang_up();
    until(&mut heard, |e| matches!(e, HostEvent::Disconnected { .. })).await;

    let addresses = vec![code.addresses[0].to_string()];
    let again = RemoteSession::connect(addresses.clone(), key.clone(), identity, "Phone".into()).await.unwrap();
    until(&mut heard, |e| matches!(e, HostEvent::Connected { .. })).await;
    assert!(again.current().connected);

    let stranger = RemoteSession::connect(addresses, key, generate_identity(), "Other".into()).await;
    assert!(stranger.is_err(), "an identity the desktop never paired is turned away");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tampered_secret_is_refused() {
    let (_host, code, mut heard) = start().await;
    let mut tampered = code.clone();
    tampered.secret[0] ^= 1;
    let refused = RemoteSession::pair(tampered.to_uri(), generate_identity(), "Thief".into()).await;
    assert!(refused.is_err(), "a code with one flipped bit pairs nothing");
    assert!(timeout(Duration::from_millis(300), heard.recv()).await.is_err(), "the desktop reports nothing");

    // The genuine code still pairs, so the refusal came from the secret and not a closed window.
    RemoteSession::pair(code.to_uri(), generate_identity(), "Phone".into()).await.unwrap();
    until(&mut heard, |e| matches!(e, HostEvent::Paired { .. })).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_spent_code_tells_the_phone_it_is_not_paired() {
    let (_host, code, mut heard) = start().await;
    let first = RemoteSession::pair(code.to_uri(), generate_identity(), "Phone".into()).await.unwrap();
    until(&mut heard, |e| matches!(e, HostEvent::Paired { .. })).await;
    let again = RemoteSession::pair(code.to_uri(), generate_identity(), "Second".into()).await;
    let spent = oni_link::Error::Spent.to_string();
    assert!(matches!(&again, Err(LinkError::Failed { reason }) if *reason == spent), "{:?}", again.err());

    // Twin: a stranger dialing by key is still `Refused`, so only a pairing reads as a spent code.
    let addresses = vec![code.addresses[0].to_string()];
    let stranger = RemoteSession::connect(addresses, first.host_key(), generate_identity(), "Other".into()).await;
    assert!(matches!(stranger, Err(LinkError::Refused)));
}

/// The desktop's receiver policy: files pass when their pin is a trusted phone's, and only then.
struct PinPolicy(Host);

impl AcceptPolicy for PinPolicy {
    fn decide<'a>(
        &'a self,
        _: &'a DeviceInfo,
        _: &'a [FileMeta],
        pin: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Decision> + Send + 'a>> {
        let known = pin.is_some_and(|pin| self.0.accepts_import_pin(pin));
        Box::pin(async move { if known { Decision::AcceptAll } else { Decision::Reject } })
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn sends_scans_to_the_desktop_and_a_strangers_pin_is_refused() {
    let (inbox, source) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (host, code, _heard) = start().await;
    let mut config = ReceiverConfig::new("ONI", oni_localsend::Identity::generate().unwrap(), inbox.path());
    config.bind = IpAddr::V4(Ipv4Addr::LOCALHOST);
    config.port = 0;
    config.policy = Arc::new(PinPolicy(host.clone()));
    let (receiver, mut received) = Receiver::start(config).await.unwrap();
    host.offer_import(receiver.info().fingerprint.clone(), receiver.local_addr().port());

    let scans: Vec<(String, Vec<u8>)> =
        (1..=2).map(|i| (format!("P10724{i:02}.DNG"), (0..3_000_000u32).map(|b| (b * i) as u8).collect())).collect();
    let paths: Vec<String> = scans
        .iter()
        .map(|(name, bytes)| {
            let path = source.path().join(name);
            std::fs::write(&path, bytes).unwrap();
            path.to_string_lossy().into_owned()
        })
        .collect();

    let session = RemoteSession::pair(code.to_uri(), generate_identity(), "Phone".into()).await.unwrap();
    assert!(session.can_send_files());
    let result = session.send_files(paths.clone()).await.unwrap();
    assert_eq!(result.sent.len(), 2);
    assert!(result.skipped.is_empty());
    until(&mut received, |e| matches!(e, Event::SessionFinished { .. })).await;
    for (name, bytes) in &scans {
        assert_eq!(&std::fs::read(inbox.path().join(name)).unwrap(), bytes, "{name} lands byte for byte");
    }

    // Another desktop's phone, pointed at this receiver, carries a pin this desktop never derived.
    let (other, events) = Host::new(Identity::generate(), "Elsewhere".into(), []);
    let address = other.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
    other.offer_import(receiver.info().fingerprint.clone(), receiver.local_addr().port());
    let stranger_code = other.open_pairing(vec![address]);
    let _desk = desk(other, events);
    let stranger = RemoteSession::pair(stranger_code.to_uri(), generate_identity(), "Thief".into()).await.unwrap();
    assert!(matches!(stranger.send_files(paths).await, Err(LinkError::ImportRefused)));
    assert_eq!(std::fs::read_dir(inbox.path()).unwrap().count(), 2, "nothing more landed");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_panel_reaches_the_phone_and_its_keys_reach_the_desktop() {
    let (_host, code, mut heard) = start().await;
    let session = RemoteSession::pair(code.to_uri(), generate_identity(), "Phone".into()).await.unwrap();
    let (tx, mut shown) = mpsc::unbounded_channel();
    session.set_listener(Box::new(Recorder(tx)));
    let first = until(&mut shown, |d| !d.panel.rows.is_empty()).await;
    let row = &first.panel.rows[0];
    assert_eq!((first.panel.face, row.param, row.tab), (Face::Pro, Param::Exposure, PanelTab::Color));
    assert_eq!((row.block.as_str(), row.label.as_str(), row.value.as_str()), ("Lighting", "Exposure", "+0.00 EV"));

    session.press(Command::SetFace { face: Face::Scanner });
    assert_eq!(session.current().panel.face, Face::Scanner, "the face shows before the desktop answers");
    session.press(Command::StepParam { param: Param::Exposure, by: 3 });
    session.press(Command::ResetParam { param: Param::Texture });
    let settled = until(&mut shown, |d| d.state.applied == 3).await;
    assert_eq!((settled.panel.face, settled.panel.rows[0].value.as_str()), (Face::Scanner, "+0.30 EV"));
    let commands: Vec<oni_link::Command> = [
        until(&mut heard, |e| matches!(e, HostEvent::Command { .. })).await,
        until(&mut heard, |e| matches!(e, HostEvent::Command { .. })).await,
        until(&mut heard, |e| matches!(e, HostEvent::Command { .. })).await,
    ]
    .into_iter()
    .filter_map(|e| if let HostEvent::Command { command, .. } = e { Some(command) } else { None })
    .collect();
    assert_eq!(
        commands,
        [
            oni_link::Command::Face(oni_link::Face::Scanner),
            oni_link::Command::StepParam { param: oni_link::Param::Exposure, by: 3 },
            oni_link::Command::ResetParam { param: oni_link::Param::Texture },
        ]
    );
}

#[test]
fn every_slider_crosses_the_bindings_under_its_own_name() {
    for param in oni_link::Param::ALL {
        let mirrored = Param::from(param);
        assert_eq!(oni_link::Param::from(mirrored), param);
        assert_eq!(format!("{mirrored:?}"), format!("{param:?}"));
    }
    // Twin: a neighbour's name is another slider's, so the check tells two of them apart.
    assert_ne!(format!("{:?}", Param::from(oni_link::Param::GainRed)), format!("{:?}", oni_link::Param::GainGreen));
}
