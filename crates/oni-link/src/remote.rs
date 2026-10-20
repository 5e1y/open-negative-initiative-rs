//! The phone's side: it shows a press at once, sends it, and settles on the desktop's screen.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch};
use tokio::time::timeout;

use crate::Error;
use crate::noise::{self, Identity, PublicKey};
use crate::pairing::PairingCode;
use crate::panel::Param;
use crate::scanner::{Command, State};
use crate::wire::{Import, PROTOCOL, ToHost, ToPhone};

const PING: Duration = Duration::from_secs(2);
const DIAL: Duration = Duration::from_secs(2);
const HANDSHAKE: Duration = Duration::from_secs(5);
/// The desktop answers every ping, so this long without a word means a dead link, even one a Wi-Fi
/// drop left half open, where TCP reports nothing for minutes while presses fall into it.
pub const QUIET: Duration = Duration::from_secs(6);

/// What the phone draws: the desktop's last screen with the presses it has not applied yet.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Display {
    pub state: State,
    pub connected: bool,
    /// The last round trip, desktop and back.
    pub round_trip: Option<Duration>,
    /// A new frame from the desktop, when the transport carries previews.
    pub preview: Option<Arc<Vec<u8>>>,
    /// A press sent and not yet answered: what `state` shows of it is the phone's guess.
    pub unanswered: bool,
}

pub struct Remote {
    host: PublicKey,
    host_name: String,
    import: Option<Import>,
    outgoing: mpsc::UnboundedSender<ToHost>,
    shared: Arc<Mutex<Pending>>,
    display: watch::Sender<Display>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Drop for Remote {
    fn drop(&mut self) {
        self.tasks.iter().for_each(|task| task.abort());
    }
}

#[derive(Default)]
struct Pending {
    seq: u64,
    confirmed: State,
    presses: Vec<(u64, Command)>,
    /// The last knob place sent and not yet applied, so a drag keeps one on the wire.
    flight: Option<u64>,
    /// The latest place the hand asked for while one flies, sent once it lands.
    parked: Option<(Param, f32)>,
}

impl Pending {
    fn unanswered(&self) -> bool {
        !self.presses.is_empty() || self.parked.is_some()
    }

    fn shown(&self) -> State {
        let parked = self.parked.map(|(param, position)| Command::SetParam { param, position });
        let presses = self.presses.iter().map(|(_, command)| *command).chain(parked);
        presses.fold(self.confirmed.clone(), |state, command| state.predicted(command))
    }

    /// Numbers a press and queues it under the same lock, so the wire carries presses in their order.
    fn send(&mut self, outgoing: &mpsc::UnboundedSender<ToHost>, command: Command) -> u64 {
        self.seq += 1;
        self.presses.push((self.seq, command));
        let _ = outgoing.send(ToHost::Press { seq: self.seq, command });
        self.seq
    }

    /// The parked place, once the one in flight has landed.
    fn unpark(&mut self, outgoing: &mpsc::UnboundedSender<ToHost>) {
        if self.flight.is_some_and(|seq| seq <= self.confirmed.applied) {
            self.flight = None;
        }
        if self.flight.is_none()
            && let Some((param, position)) = self.parked.take()
        {
            self.flight = Some(self.send(outgoing, Command::SetParam { param, position }));
        }
    }
}

/// A desktop that hangs up after the handshake refused the phone; one that is slow or cut off did
/// not, and is dialled again.
fn refusal(error: Error) -> Error {
    use std::io::ErrorKind::{BrokenPipe, ConnectionReset, UnexpectedEof};
    match error {
        Error::Io(e) if matches!(e.kind(), UnexpectedEof | BrokenPipe | ConnectionReset) => Error::Refused,
        Error::Io(_) => Error::Unreachable,
        _ => Error::Refused,
    }
}

/// The link is down: presses the desktop never answered are no longer shown as made.
fn hung_up(pending: &Mutex<Pending>, display: &watch::Sender<Display>) {
    let mut pending = pending.lock().expect("pending lock");
    pending.presses.clear();
    pending.parked = None;
    display.send_modify(|display| {
        display.connected = false;
        display.state = pending.confirmed.clone();
        display.unanswered = false;
    });
}

impl Remote {
    /// The first meeting: the QR code's secret proves the phone was shown the desktop's screen.
    /// A desktop that hangs up here refuses the code (spent, expired, closed or wrong), never the phone.
    pub async fn pair<S>(stream: S, me: &Identity, code: &PairingCode, name: &str) -> Result<Remote, Error>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let opened = Self::open(stream, me, code.host, Some(code.secret), name).await;
        opened.map_err(|error| if matches!(error, Error::Refused) { Error::Spent } else { error })
    }

    pub async fn connect<S>(stream: S, me: &Identity, host: PublicKey, name: &str) -> Result<Remote, Error>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        Self::open(stream, me, host, None, name).await
    }

    /// The first address that answers, in the order given.
    pub async fn dial(addresses: &[SocketAddr]) -> Result<TcpStream, Error> {
        for address in addresses {
            if let Ok(Ok(stream)) = timeout(DIAL, TcpStream::connect(address)).await {
                let _ = stream.set_nodelay(true);
                return Ok(stream);
            }
        }
        Err(Error::Unreachable)
    }

    async fn open<S>(
        stream: S,
        me: &Identity,
        host: PublicKey,
        secret: Option<[u8; 32]>,
        name: &str,
    ) -> Result<Remote, Error>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (mut reader, mut writer) =
            timeout(HANDSHAKE, noise::initiate(stream, me, host, secret)).await.map_err(|_| Error::Unreachable)??;
        let hello = ToHost::Hello { protocol: PROTOCOL, name: name.to_owned() };
        writer.send(&hello).await.map_err(refusal)?;
        let welcome = timeout(HANDSHAKE, reader.recv::<ToPhone>()).await.map_err(|_| Error::Unreachable)?;
        let (host_name, import) = match welcome.map_err(refusal)? {
            ToPhone::Welcome { protocol, name, import } if protocol == PROTOCOL => (name, import),
            ToPhone::Welcome { protocol, .. } => return Err(Error::Version(protocol)),
            _ => return Err(Error::Refused),
        };

        let shared = Arc::new(Mutex::new(Pending::default()));
        let display = watch::channel(Display { connected: true, ..Display::default() }).0;
        let (outgoing, mut queue) = mpsc::unbounded_channel::<ToHost>();
        let epoch = Instant::now();

        let (writing, cut) = (shared.clone(), display.clone());
        let write_task = tokio::spawn(async move {
            while let Some(message) = queue.recv().await {
                if writer.send(&message).await.is_err() {
                    hung_up(&writing, &cut);
                    break;
                }
            }
        });
        let pinger = outgoing.clone();
        let ping_task = tokio::spawn(async move {
            let mut tick = tokio::time::interval(PING);
            loop {
                tick.tick().await;
                let sent = epoch.elapsed().as_micros() as u64;
                if pinger.send(ToHost::Ping { sent }).is_err() {
                    break;
                }
            }
        });
        let (reading, screen, unparking) = (shared.clone(), display.clone(), outgoing.clone());
        let read_task = tokio::spawn(async move {
            // `recv` is not cancel-safe, but a timeout ends the session, so no frame is read again.
            while let Ok(Ok(message)) = timeout(QUIET, reader.recv::<ToPhone>()).await {
                let message = match message {
                    ToPhone::Screen { mut state, panel } => {
                        state.panel = panel;
                        ToPhone::State(state)
                    }
                    other => other,
                };
                match message {
                    ToPhone::State(state) => {
                        // Under the lock, so a press racing this screen never leaves a stale flag.
                        let mut pending = reading.lock().expect("pending lock");
                        pending.presses.retain(|(seq, _)| *seq > state.applied);
                        pending.confirmed = state;
                        pending.unpark(&unparking);
                        screen.send_modify(|display| {
                            display.state = pending.shown();
                            display.unanswered = pending.unanswered();
                        });
                    }
                    ToPhone::Pong { sent } => {
                        let trip = epoch.elapsed().saturating_sub(Duration::from_micros(sent));
                        screen.send_modify(|display| display.round_trip = Some(trip));
                    }
                    ToPhone::Preview { jpeg, .. } => {
                        screen.send_modify(|display| display.preview = Some(Arc::new(jpeg)));
                    }
                    ToPhone::Welcome { .. } | ToPhone::Screen { .. } => {}
                }
            }
            // Presses the desktop never answered are not shown as made.
            hung_up(&reading, &screen);
        });

        let tasks = vec![write_task, ping_task, read_task];
        Ok(Remote { host, host_name, import, outgoing, shared, display, tasks })
    }

    /// Sends a key and returns the screen as it will read once the desktop applies it; a link
    /// already down sends nothing and shows the desktop's last screen.
    pub fn press(&self, command: Command) -> State {
        self.change(|pending, outgoing| {
            pending.send(outgoing, command);
        })
    }

    /// A held slider's knob at `position` on its track: at most one place flies to the desktop, the
    /// latest waiting behind it, so a fast hand never queues a backlog.
    pub fn slide(&self, param: Param, position: f32) -> State {
        self.change(|pending, outgoing| {
            pending.parked = Some((param, position));
            pending.unpark(outgoing);
        })
    }

    /// The hand off the slider: its last place goes first, then the release that makes one step.
    pub fn let_go(&self, param: Param) -> State {
        self.change(|pending, outgoing| {
            if let Some((param, position)) = pending.parked.take() {
                pending.flight = Some(pending.send(outgoing, Command::SetParam { param, position }));
            }
            pending.send(outgoing, Command::ReleaseParam { param });
        })
    }

    /// One change of the presses, shown under the same lock the desktop's screens land under.
    fn change(&self, edit: impl FnOnce(&mut Pending, &mpsc::UnboundedSender<ToHost>)) -> State {
        let mut pending = self.shared.lock().expect("pending lock");
        if !self.display.borrow().connected {
            return self.display.borrow().state.clone();
        }
        edit(&mut pending, &self.outgoing);
        let shown = pending.shown();
        self.display.send_modify(|display| {
            display.state = shown.clone();
            display.unanswered = pending.unanswered();
        });
        shown
    }

    pub fn display(&self) -> watch::Receiver<Display> {
        self.display.subscribe()
    }

    pub fn host(&self) -> PublicKey {
        self.host
    }

    pub fn host_name(&self) -> &str {
        &self.host_name
    }

    /// Where and how to send photos to this desktop, when it receives them.
    pub fn import(&self) -> Option<&Import> {
        self.import.as_ref()
    }
}
