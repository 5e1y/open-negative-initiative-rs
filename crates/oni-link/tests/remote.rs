use std::net::SocketAddr;
use std::time::Duration;

use oni_link::noise;
use oni_link::wire::{self, PROTOCOL, ToHost, ToPhone};
use oni_link::{
    Command, Display, Effect, Error, Host, HostEvent, Identity, Letter, PairingCode, PublicKey, Remote, State,
};
use tokio::sync::{mpsc, watch};
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(5);

/// A desktop whose app applies every command as the phone predicts it, then publishes.
async fn desktop() -> (Host, SocketAddr, mpsc::UnboundedReceiver<HostEvent>) {
    let (host, mut events) = Host::new(Identity::generate(), "Studio Mac".into(), []);
    let addr = host.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
    let (seen, log) = mpsc::unbounded_channel();
    let app = host.clone();
    tokio::spawn(async move {
        let mut state = State { frame: Some((7, 36)), chrome: Some(0.0), ..State::default() };
        app.publish(state.clone());
        while let Some(event) = events.next().await {
            if let HostEvent::Command { command, .. } = &event {
                state = state.predicted(*command);
                state.revision += 1;
                app.publish(state.clone());
            }
            let _ = seen.send(event);
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

async fn pair(host: &Host, addr: SocketAddr, phone: &Identity) -> Result<Remote, Error> {
    let code = PairingCode::parse(&host.open_pairing(vec![addr]).to_uri())?;
    Remote::pair(Remote::dial(&code.addresses).await?, phone, &code, "iPhone").await
}

#[tokio::test]
async fn presses_reach_the_desktop_and_its_screen_comes_back() {
    let (host, addr, mut log) = desktop().await;
    let phone = Identity::generate();
    let remote = pair(&host, addr, &phone).await.unwrap();
    assert_eq!(remote.host_name(), "Studio Mac");
    assert!(matches!(log.recv().await, Some(HostEvent::Paired { phone: key, .. }) if key == phone.public()));

    for _ in 0..3 {
        remote.press(Command::Step { letter: Letter::D, by: 1 });
    }
    remote.press(Command::Step { letter: Letter::C, by: -3 });
    remote.press(Command::StepEffect { effect: Effect::Vignette, by: 3 });

    let mut display = remote.display();
    let shown = until(&mut display, |d| d.state.applied == 5).await;
    assert_eq!((shown.state.clicks.d, shown.state.clicks.c), (3, -3));
    assert!((shown.state.vignette - 0.3).abs() < 1e-6);
    assert_eq!(shown.state.revision, 5, "the screen is the desktop's, not the prediction");
    assert_eq!(shown.state.frame, Some((7, 36)));
}

#[tokio::test]
async fn a_press_shows_before_the_desktop_answers() {
    let (host, _events) = Host::new(Identity::generate(), "Mute".into(), []);
    let addr = host.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
    let remote = pair(&host, addr, &Identity::generate()).await.unwrap();
    // No app reads the events, so nothing is ever published: the count is the prediction alone.
    let shown = remote.press(Command::Step { letter: Letter::Y, by: 3 });
    assert_eq!(shown.clicks.y, 3);
    assert_eq!(remote.display().borrow().state.clicks.y, 3);
}

#[tokio::test]
async fn counts_stop_at_the_keypad_ends() {
    let state = (0..20).fold(State::default(), |s, _| s.predicted(Command::Step { letter: Letter::D, by: 3 }));
    assert_eq!(state.clicks.d, 48, "24 units of the desktop's screen, in steps");
    let state = (0..20).fold(State::default(), |s, _| s.predicted(Command::Step { letter: Letter::W, by: -3 }));
    assert_eq!(state.clicks.w, -20);
    // Twin: inside the ends a press of three is three steps, held by nothing.
    assert_eq!(State::default().predicted(Command::Step { letter: Letter::D, by: 3 }).clicks.d, 3);
    let mono = State { channel: oni_link::Channel::Monochrome, chrome: None, ..State::default() };
    let state = mono.predicted(Command::StepEffect { effect: Effect::Chrome, by: 1 });
    assert_eq!(state.chrome, None, "black and white has no Chrome to move");
}

#[tokio::test]
async fn a_tampered_secret_is_refused() {
    let (host, addr, _log) = desktop().await;
    let mut code = host.open_pairing(vec![addr]);
    code.secret[0] ^= 1;
    let stream = Remote::dial(&code.addresses).await.unwrap();
    assert!(Remote::pair(stream, &Identity::generate(), &code, "Intruder").await.is_err());
    // The twin: the untampered code from the same window still pairs, so the refusal was the secret.
    code.secret[0] ^= 1;
    let stream = Remote::dial(&code.addresses).await.unwrap();
    assert!(Remote::pair(stream, &Identity::generate(), &code, "iPhone").await.is_ok());
}

#[tokio::test]
async fn a_code_pairs_one_phone_only() {
    let (host, addr, _log) = desktop().await;
    let code = host.open_pairing(vec![addr]);
    let first = Remote::pair(Remote::dial(&[addr]).await.unwrap(), &Identity::generate(), &code, "One").await;
    let second = Remote::pair(Remote::dial(&[addr]).await.unwrap(), &Identity::generate(), &code, "Two").await;
    assert!(first.is_ok());
    assert!(second.is_err());
}

/// Two phones finish the handshake on one code before either says Hello: the second hears no Welcome.
#[tokio::test]
async fn a_phone_on_a_spent_code_learns_it_was_refused() {
    let (host, addr, mut log) = desktop().await;
    let code = host.open_pairing(vec![addr]);
    let mut opened = Vec::new();
    for _ in 0..2 {
        let stream = Remote::dial(&[addr]).await.unwrap();
        opened.push(noise::initiate(stream, &Identity::generate(), code.host, Some(code.secret)).await.unwrap());
    }
    let mut answers = Vec::new();
    for (reader, writer) in &mut opened {
        writer.send(&ToHost::Hello { protocol: PROTOCOL, name: "Phone".into() }).await.unwrap();
        answers.push(timeout(WAIT, reader.recv::<ToPhone>()).await.expect("an answer or a hang-up"));
    }
    // Twin: the first phone on the code is welcomed, so the check sees a Welcome when one comes.
    assert!(matches!(answers[0], Ok(ToPhone::Welcome { .. })));
    assert!(answers[1].is_err(), "the second phone is hung up on before any Welcome");
    let mut paired = 0;
    while let Ok(Some(event)) = timeout(Duration::from_millis(300), log.recv()).await {
        paired += usize::from(matches!(event, HostEvent::Paired { .. }));
    }
    assert_eq!(paired, 1, "the desktop trusts one phone");
}

#[tokio::test]
async fn a_closed_window_refuses_pairing() {
    let (host, addr, _log) = desktop().await;
    let code = host.open_pairing(vec![addr]);
    host.close_pairing();
    let stream = Remote::dial(&[addr]).await.unwrap();
    let late = Remote::pair(stream, &Identity::generate(), &code, "Late").await;
    assert!(matches!(late, Err(Error::Spent)), "a pairing refused reads as the code's fault");
}

#[tokio::test]
async fn a_paired_phone_reconnects_and_a_stranger_does_not() {
    let (host, addr, _log) = desktop().await;
    let phone = Identity::generate();
    let host_key = pair(&host, addr, &phone).await.unwrap().host();

    let again = Remote::connect(Remote::dial(&[addr]).await.unwrap(), &phone, host_key, "iPhone").await;
    assert!(again.is_ok());
    let stranger =
        Remote::connect(Remote::dial(&[addr]).await.unwrap(), &Identity::generate(), host_key, "Stranger").await;
    assert!(matches!(stranger, Err(Error::Refused)), "twin: a phone refused by key is not told its code is spent");
    let wrong_host = PublicKey(Identity::generate().public().0);
    let impostor = Remote::connect(Remote::dial(&[addr]).await.unwrap(), &phone, wrong_host, "iPhone").await;
    assert!(impostor.is_err(), "a phone checks the desktop's key as much as the reverse");
}

#[tokio::test]
async fn a_reconnected_phone_counts_from_one_again() {
    let (host, addr, _log) = desktop().await;
    let phone = Identity::generate();
    let first = pair(&host, addr, &phone).await.unwrap();
    for _ in 0..4 {
        first.press(Command::Step { letter: Letter::M, by: 1 });
    }
    until(&mut first.display(), |d| d.state.applied == 4).await;
    let host_key = first.host();
    drop(first);

    let second = Remote::connect(Remote::dial(&[addr]).await.unwrap(), &phone, host_key, "iPhone").await.unwrap();
    let mut display = second.display();
    let fresh = until(&mut display, |d| d.state.clicks.m == 4).await;
    assert_eq!(fresh.state.applied, 0, "the old connection's count does not leak into this one");
    second.press(Command::Step { letter: Letter::M, by: 1 });
    let shown = until(&mut display, |d| d.state.applied == 1).await;
    assert_eq!(shown.state.clicks.m, 5);
}

/// Another hand publishes while a press is read but not yet applied: the phone keeps showing it.
#[tokio::test]
async fn a_press_read_but_not_applied_stays_shown_through_another_hand_s_screen() {
    // Twin: marking the press taken on read, before it is applied, drops it from the phone.
    for marked_on_read in [false, true] {
        let (host, mut events) = Host::new(Identity::generate(), "Studio".into(), []);
        let addr = host.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let remote = pair(&host, addr, &Identity::generate()).await.unwrap();
        remote.press(Command::Step { letter: Letter::D, by: 1 });
        let receipt = loop {
            match timeout(WAIT, events.receive()).await.unwrap().unwrap() {
                (HostEvent::Command { .. }, receipt) => break receipt,
                (_, receipt) => receipt.applied(),
            }
        };
        let pending = if marked_on_read {
            receipt.applied();
            None
        } else {
            Some(receipt)
        };
        host.publish(State { revision: 1, ..State::default() });
        let mut display = remote.display();
        let shown = until(&mut display, |d| d.state.revision == 1).await;
        assert_eq!(shown.state.clicks.d, if marked_on_read { 0 } else { 1 }, "marked on read: {marked_on_read}");
        if let Some(receipt) = pending {
            receipt.applied();
            let mut applied = State { revision: 2, ..State::default() };
            applied.clicks.d = 1;
            host.publish(applied);
            let settled = until(&mut display, |d| d.state.applied == 1).await;
            assert_eq!(settled.state.clicks.d, 1, "the press counted once");
        }
    }
}

#[tokio::test]
async fn revoking_a_phone_cuts_it_at_once() {
    let (host, addr, _log) = desktop().await;
    let phone = Identity::generate();
    let remote = pair(&host, addr, &phone).await.unwrap();
    let mut display = remote.display();
    host.revoke(phone.public());
    until(&mut display, |d| !d.connected).await;
    let again = Remote::connect(Remote::dial(&[addr]).await.unwrap(), &phone, remote.host(), "iPhone").await;
    assert!(again.is_err());
}

#[tokio::test]
async fn round_trip_is_measured() {
    let (host, addr, _log) = desktop().await;
    let remote = pair(&host, addr, &Identity::generate()).await.unwrap();
    let shown = timeout(Duration::from_secs(5), async {
        let mut display = remote.display();
        until(&mut display, |d| d.round_trip.is_some()).await
    })
    .await
    .unwrap();
    assert!(shown.round_trip.unwrap() < Duration::from_millis(500));
}

#[test]
fn pairing_codes_survive_their_uri() {
    let code = PairingCode {
        host: Identity::generate().public(),
        secret: [7; 32],
        name: "Rémi's Mac & co".into(),
        addresses: vec!["192.168.1.20:53318".parse().unwrap(), "[fe80::1]:53318".parse().unwrap()],
    };
    assert_eq!(PairingCode::parse(&code.to_uri()).unwrap(), code);
    let future = code.to_uri().replace("v=1", "v=9");
    assert!(matches!(PairingCode::parse(&future), Err(Error::Version(9))));
    assert!(PairingCode::parse("https://example.com").is_err());
}

#[test]
fn a_message_larger_than_a_frame_is_refused() {
    let small = ToPhone::Preview { revision: 1, jpeg: vec![0; 60_000] };
    assert!(wire::encode(&small).is_ok());
    let large = ToPhone::Preview { revision: 1, jpeg: vec![0; 70_000] };
    assert!(matches!(wire::encode(&large), Err(Error::TooLarge(_))));
}

#[test]
fn identities_survive_their_bytes() {
    let me = Identity::generate();
    assert_eq!(Identity::from_bytes(&me.to_bytes()).unwrap().public(), me.public());
    assert!(Identity::from_bytes(&[0; 10]).is_err());
}

#[tokio::test]
async fn a_paired_phone_learns_where_to_send_photos_and_its_own_pin() {
    let (host, addr, _log) = desktop().await;
    host.offer_import("ab12cd".into(), 53317);
    let phone = Identity::generate();
    let remote = pair(&host, addr, &phone).await.unwrap();
    let import = remote.import().cloned().expect("the desktop offers import");
    assert_eq!((import.fingerprint.as_str(), import.port), ("ab12cd", 53317));
    assert!(host.accepts_import_pin(&import.pin));

    let other = pair(&host, addr, &Identity::generate()).await.unwrap();
    assert_ne!(other.import().unwrap().pin, import.pin, "each phone holds its own secret");
    assert!(!host.accepts_import_pin("00000000000000000000000000000000"));
    host.revoke(phone.public());
    assert!(!host.accepts_import_pin(&import.pin), "a forgotten phone's secret dies with it");
}
