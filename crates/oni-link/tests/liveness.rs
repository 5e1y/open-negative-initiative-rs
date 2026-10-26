//! The link stays up while nothing is pressed, and a link cut without a word, as a Wi-Fi drop
//! leaves it, is seen by both ends within their deadlines; on the desktop's clock, paused and run
//! ahead, so two minutes take no time.

use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use oni_link::remote::QUIET;
use oni_link::{Command, Error, Face, Host, HostEvent, Identity, Letter, Panel, Param, Remote, Row, State, Tab, noise};
use tokio::io::{AsyncRead, AsyncWrite, DuplexStream, ReadBuf};
use tokio::sync::mpsc;

/// Longer than every drop the phone showed on a real network, 20 to 60 s apart.
const IDLE: Duration = Duration::from_secs(120);

/// The phone's end of the network; once cut, what it writes is lost and nothing reaches it, not
/// even the desktop's hang-up.
struct Wire {
    inner: DuplexStream,
    cut: Arc<AtomicBool>,
}

impl AsyncRead for Wire {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        if self.cut.load(Ordering::Relaxed) {
            return Poll::Pending;
        }
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for Wire {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        if self.cut.load(Ordering::Relaxed) {
            return Poll::Ready(Ok(buf.len()));
        }
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

async fn paired() -> (Remote, mpsc::UnboundedReceiver<HostEvent>, Arc<AtomicBool>) {
    let (host, mut events) = Host::new(Identity::generate(), "Studio".into(), []);
    let code = host.open_pairing(Vec::new());
    let (seen, log) = mpsc::unbounded_channel();
    let app = host.clone();
    tokio::spawn(async move {
        let mut state = State::default();
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
    let (desk, phone) = tokio::io::duplex(1 << 16);
    host.serve(desk);
    let cut = Arc::new(AtomicBool::new(false));
    let wire = Wire { inner: phone, cut: cut.clone() };
    let remote = Remote::pair(wire, &Identity::generate(), &code, "Phone").await.unwrap();
    (remote, log, cut)
}

/// The desktop's events so far, without waiting.
fn heard(log: &mut mpsc::UnboundedReceiver<HostEvent>) -> Vec<HostEvent> {
    std::iter::from_fn(|| log.try_recv().ok()).collect()
}

#[tokio::test(start_paused = true)]
async fn an_idle_link_outlives_the_drops_a_phone_saw_and_a_silent_cut_is_seen() {
    let (remote, mut events, cut) = paired().await;
    tokio::time::sleep(IDLE).await;
    assert!(remote.display().borrow().connected, "two idle minutes later the phone is still connected");
    let idle = heard(&mut events);
    assert!(!idle.iter().any(|e| matches!(e, HostEvent::Disconnected { .. })), "{idle:?}");
    remote.press(Command::Step { letter: Letter::D, by: 1 });
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert_eq!(remote.display().borrow().state.clicks.d, 1);
    assert!(!remote.display().borrow().unanswered, "the desktop answered the press");

    // Twin: the same link cut without a word is seen as down by the phone, then by the desktop.
    cut.store(true, Ordering::Relaxed);
    let shown = remote.press(Command::Step { letter: Letter::D, by: 1 });
    assert_eq!((shown.clicks.d, remote.display().borrow().unanswered), (2, true), "a guess, flagged unanswered");
    tokio::time::sleep(QUIET + Duration::from_secs(1)).await;
    let display = remote.display().borrow().clone();
    assert!(!display.connected, "the phone no longer claims a link it cannot hear");
    assert_eq!(display.state.clicks.d, 1, "the unanswered press is not shown as made");
    assert_eq!(remote.press(Command::Step { letter: Letter::D, by: 1 }).clicks.d, 1, "a dead link takes no press");
    tokio::time::sleep(Duration::from_secs(10)).await;
    assert!(heard(&mut events).iter().any(|e| matches!(e, HostEvent::Disconnected { .. })));
}

/// A desktop that finishes the handshake, then either hangs up or says nothing.
async fn silent_or_closing(hang_up: bool) -> Result<Remote, Error> {
    let desktop = Identity::generate();
    let (desk, phone) = tokio::io::duplex(1 << 16);
    let key = desktop.public();
    tokio::spawn(async move {
        let Ok((_, _, reader, writer)) = noise::respond(desk, &desktop, None).await else { return };
        if !hang_up {
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
        drop((reader, writer));
    });
    Remote::connect(phone, &Identity::generate(), key, "Phone").await
}

#[tokio::test(start_paused = true)]
async fn only_a_desktop_that_hangs_up_refuses_the_phone() {
    assert!(matches!(silent_or_closing(false).await, Err(Error::Unreachable)), "a silent desktop is only unreachable");
    // Twin: a desktop that hangs up after the handshake is a refusal.
    assert!(matches!(silent_or_closing(true).await, Err(Error::Refused)));
}

/// A phone paired over a cuttable wire to a desktop that publishes a screen of some 5 kB every
/// 200 ms, as a hand editing on the desktop would, so the phone's end fills in seconds once unread.
async fn busy_desktop() -> (Remote, mpsc::UnboundedReceiver<HostEvent>, Arc<AtomicBool>) {
    let (host, mut events) = Host::new(Identity::generate(), "Studio".into(), []);
    let code = host.open_pairing(Vec::new());
    let (seen, log) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(event) = events.next().await {
            let _ = seen.send(event);
        }
    });
    let row = Row {
        param: Param::Exposure,
        tab: Tab::Color,
        block: "Lighting".into(),
        label: "x".repeat(100),
        value: "+0.00 EV".into(),
        position: 0.5,
        modified: false,
    };
    let app = host.clone();
    tokio::spawn(async move {
        let mut state = State { panel: Panel { face: Face::Pro, rows: vec![row; 50] }, ..State::default() };
        loop {
            state.revision += 1;
            app.publish(state.clone());
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    });
    let (desk, phone) = tokio::io::duplex(1 << 16);
    host.serve(desk);
    let cut = Arc::new(AtomicBool::new(false));
    let wire = Wire { inner: phone, cut: cut.clone() };
    let remote = Remote::pair(wire, &Identity::generate(), &code, "Phone").await.unwrap();
    (remote, log, cut)
}

#[tokio::test(start_paused = true)]
async fn a_vanished_phone_is_let_go_while_the_desktop_keeps_publishing() {
    let (_remote, mut events, cut) = busy_desktop().await;
    cut.store(true, Ordering::Relaxed);
    tokio::time::sleep(Duration::from_secs(25)).await;
    let gone = heard(&mut events);
    assert!(gone.iter().any(|e| matches!(e, HostEvent::Disconnected { .. })), "{gone:?}");

    // Twin: the same desktop with a phone that still reads and pings keeps it connected.
    let (remote, mut events, _cut) = busy_desktop().await;
    tokio::time::sleep(IDLE).await;
    assert!(!heard(&mut events).iter().any(|e| matches!(e, HostEvent::Disconnected { .. })));
    assert!(remote.display().borrow().connected);
}
