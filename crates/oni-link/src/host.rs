//! The desktop's side: it accepts paired phones, hands their commands to the app in order,
//! and sends each phone the screen the app publishes.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};
use tokio::time::timeout;

use crate::Error;
use crate::noise::{self, Identity, Mode, PublicKey};
use crate::pairing::PairingCode;
use crate::scanner::{Command, State};
use crate::wire::{Import, PROTOCOL, ToHost, ToPhone};

/// A QR code stays valid this long, and for one phone only.
const PAIRING_WINDOW: Duration = Duration::from_secs(120);
/// A phone pings every two seconds; this much silence closes its connection.
const SILENCE: Duration = Duration::from_secs(10);
const HANDSHAKE: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, PartialEq)]
pub enum HostEvent {
    /// A new phone passed the QR code: the app stores its key to trust it next time.
    Paired {
        phone: PublicKey,
        name: String,
    },
    Connected {
        phone: PublicKey,
        name: String,
    },
    Command {
        phone: PublicKey,
        command: Command,
    },
    Disconnected {
        phone: PublicKey,
    },
}

#[derive(Clone, Copy)]
struct Taken {
    connection: u64,
    seq: u64,
}

#[derive(Clone)]
pub struct Host {
    shared: Arc<Shared>,
}

struct Shared {
    identity: Identity,
    name: String,
    trusted: Mutex<HashSet<PublicKey>>,
    window: Mutex<Option<([u8; 32], Instant)>>,
    /// The screen, with each connection's last press the app had taken when it published.
    screen: watch::Sender<(State, HashMap<u64, u64>)>,
    taken: Mutex<HashMap<u64, u64>>,
    connections: AtomicU64,
    revoked: watch::Sender<u64>,
    import: Mutex<Option<(String, u16)>>,
    events: mpsc::UnboundedSender<(HostEvent, Option<Taken>)>,
}

/// The app's inbox. Reading an event marks its press as taken, so a later `publish` covers it.
pub struct HostEvents {
    rx: mpsc::UnboundedReceiver<(HostEvent, Option<Taken>)>,
    shared: Arc<Shared>,
}

/// A press read but not yet applied; `applied` marks it taken, so a screen published in between,
/// by another hand, never tells the phone its press is in.
#[must_use]
pub struct Receipt {
    taken: Option<Taken>,
    shared: Arc<Shared>,
}

impl Receipt {
    pub fn applied(self) {
        if let Some(Taken { connection, seq }) = self.taken {
            self.shared.taken.lock().expect("taken lock").insert(connection, seq);
        }
    }
}

impl HostEvents {
    pub async fn next(&mut self) -> Option<HostEvent> {
        let (event, receipt) = self.receive().await?;
        receipt.applied();
        Some(event)
    }

    /// The next event with its receipt, for an app that applies presses away from this task.
    pub async fn receive(&mut self) -> Option<(HostEvent, Receipt)> {
        let (event, taken) = self.rx.recv().await?;
        Some((event, Receipt { taken, shared: self.shared.clone() }))
    }
}

impl Host {
    pub fn new(identity: Identity, name: String, trusted: impl IntoIterator<Item = PublicKey>) -> (Host, HostEvents) {
        let (events, rx) = mpsc::unbounded_channel();
        let shared = Arc::new(Shared {
            identity,
            name,
            trusted: Mutex::new(trusted.into_iter().collect()),
            window: Mutex::new(None),
            screen: watch::channel((State::default(), HashMap::new())).0,
            taken: Mutex::new(HashMap::new()),
            connections: AtomicU64::new(0),
            revoked: watch::channel(0).0,
            import: Mutex::new(None),
            events,
        });
        (Host { shared: shared.clone() }, HostEvents { rx, shared })
    }

    /// Accepts phones on TCP until the host is dropped; returns the bound address.
    pub async fn listen(&self, addr: SocketAddr) -> Result<SocketAddr, Error> {
        let listener = TcpListener::bind(addr).await?;
        let local = listener.local_addr()?;
        let weak = Arc::downgrade(&self.shared);
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let Some(shared) = weak.upgrade() else { break };
                let _ = stream.set_nodelay(true);
                tokio::spawn(serve(shared, stream));
            }
        });
        Ok(local)
    }

    /// Serves one phone over any byte stream, such as a Bluetooth channel.
    pub fn serve<S>(&self, stream: S)
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        tokio::spawn(serve(self.shared.clone(), stream));
    }

    pub fn open_pairing(&self, addresses: Vec<SocketAddr>) -> PairingCode {
        let secret: [u8; 32] = rand::random();
        *self.shared.window.lock().expect("window lock") = Some((secret, Instant::now() + PAIRING_WINDOW));
        PairingCode { host: self.shared.identity.public(), secret, name: self.shared.name.clone(), addresses }
    }

    pub fn close_pairing(&self) {
        *self.shared.window.lock().expect("window lock") = None;
    }

    /// Forgets a phone and closes its connection at once.
    pub fn revoke(&self, phone: PublicKey) {
        self.shared.trusted.lock().expect("trusted lock").remove(&phone);
        self.shared.revoked.send_modify(|generation| *generation += 1);
    }

    /// Tells phones where the LocalSend receiver listens, by its certificate's fingerprint.
    pub fn offer_import(&self, fingerprint: String, port: u16) {
        *self.shared.import.lock().expect("import lock") = Some((fingerprint, port));
    }

    /// True for the secret of a phone still trusted: its files need no question.
    pub fn accepts_import_pin(&self, pin: &str) -> bool {
        let trusted = self.shared.trusted.lock().expect("trusted lock");
        trusted.iter().any(|phone| self.shared.import_pin(phone) == pin)
    }

    /// The scanner's screen after a change, from whatever hand; every phone receives it.
    pub fn publish(&self, mut state: State) {
        state.applied = 0;
        let taken = self.shared.taken.lock().expect("taken lock").clone();
        self.shared.screen.send_replace((state, taken));
    }
}

impl Shared {
    fn window_secret(&self) -> Option<[u8; 32]> {
        let window = self.window.lock().expect("window lock");
        window.filter(|(_, until)| Instant::now() < *until).map(|(secret, _)| secret)
    }

    fn take_window(&self, secret: Option<[u8; 32]>) -> bool {
        let mut window = self.window.lock().expect("window lock");
        let open = window.is_some_and(|(open, until)| Some(open) == secret && Instant::now() < until);
        if open {
            *window = None;
        }
        open
    }

    /// Derived from both keys, so it needs no storage and dies with the phone's trust.
    fn import_pin(&self, phone: &PublicKey) -> String {
        let digest = Sha256::new()
            .chain_update(b"oni-link import pin")
            .chain_update(self.identity.private())
            .chain_update(phone.0)
            .finalize();
        digest[..16].iter().map(|b| format!("{b:02x}")).collect()
    }

    fn trusts(&self, phone: &PublicKey) -> bool {
        self.trusted.lock().expect("trusted lock").contains(phone)
    }

    fn emit(&self, event: HostEvent, taken: Option<Taken>) {
        let _ = self.events.send((event, taken));
    }
}

async fn serve<S>(shared: Arc<Shared>, stream: S)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let secret = shared.window_secret();
    let Ok(Ok((mode, phone, mut reader, mut writer))) =
        timeout(HANDSHAKE, noise::respond(stream, &shared.identity, secret)).await
    else {
        return;
    };
    if mode == Mode::Paired && !shared.trusts(&phone) {
        return;
    }
    let Ok(Ok(ToHost::Hello { protocol, name })) = timeout(HANDSHAKE, reader.recv::<ToHost>()).await else {
        return;
    };
    // A second phone on the same code finds it spent and hears no Welcome, so it reads a refusal.
    if protocol == PROTOCOL && mode == Mode::Pairing && !shared.take_window(secret) {
        return;
    }
    let import = shared.import.lock().expect("import lock").clone();
    let import = import.map(|(fingerprint, port)| Import { fingerprint, port, pin: shared.import_pin(&phone) });
    if writer.send(&ToPhone::Welcome { protocol: PROTOCOL, name: shared.name.clone(), import }).await.is_err()
        || protocol != PROTOCOL
    {
        return;
    }
    if mode == Mode::Pairing {
        shared.trusted.lock().expect("trusted lock").insert(phone);
        shared.emit(HostEvent::Paired { phone, name: name.clone() }, None);
    }
    // Presses are counted per connection, so a reconnected phone starting from one is never ahead.
    let connection = shared.connections.fetch_add(1, Ordering::Relaxed);
    shared.emit(HostEvent::Connected { phone, name }, None);

    // `recv` is not cancel-safe, so it runs on its own task and the loop selects on its channel.
    let (incoming, mut messages) = mpsc::channel(64);
    let read_task = tokio::spawn(async move {
        while let Ok(message) = reader.recv::<ToHost>().await {
            if incoming.send(message).await.is_err() {
                break;
            }
        }
    });
    let mut screen = shared.screen.subscribe();
    let mut revoked = shared.revoked.subscribe();
    screen.mark_changed();
    loop {
        tokio::select! {
            message = timeout(SILENCE, messages.recv()) => match message {
                Ok(Some(ToHost::Press { seq, command })) => shared.emit(HostEvent::Command { phone, command }, Some(Taken { connection, seq })),
                Ok(Some(ToHost::Ping { sent })) => {
                    if writer.send(&ToPhone::Pong { sent }).await.is_err() { break }
                }
                Ok(Some(ToHost::Hello { .. })) => {}
                Ok(None) | Err(_) => break,
            },
            changed = screen.changed() => {
                if changed.is_err() { break }
                let (mut state, taken) = screen.borrow_and_update().clone();
                state.applied = taken.get(&connection).copied().unwrap_or(0);
                if writer.send(&ToPhone::State(state)).await.is_err() { break }
            }
            _ = revoked.changed() => {
                if !shared.trusts(&phone) { break }
            }
        }
    }
    read_task.abort();
    shared.taken.lock().expect("taken lock").remove(&connection);
    shared.emit(HostEvent::Disconnected { phone }, None);
}
