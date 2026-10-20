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
use crate::scanner::{Command, State};
use crate::wire::{Import, PROTOCOL, ToHost, ToPhone};

const PING: Duration = Duration::from_secs(2);
const DIAL: Duration = Duration::from_secs(2);
const HANDSHAKE: Duration = Duration::from_secs(5);

/// What the phone draws: the desktop's last screen with the presses it has not applied yet.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Display {
    pub state: State,
    pub connected: bool,
    /// The last round trip, desktop and back.
    pub round_trip: Option<Duration>,
    /// A new frame from the desktop, when the transport carries previews.
    pub preview: Option<Arc<Vec<u8>>>,
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
}

impl Pending {
    fn shown(&self) -> State {
        self.presses.iter().fold(self.confirmed.clone(), |state, (_, command)| state.predicted(*command))
    }
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
        writer.send(&ToHost::Hello { protocol: PROTOCOL, name: name.to_owned() }).await?;
        let welcome = timeout(HANDSHAKE, reader.recv::<ToPhone>()).await.map_err(|_| Error::Refused)?;
        let (host_name, import) = match welcome.map_err(|_| Error::Refused)? {
            ToPhone::Welcome { protocol, name, import } if protocol == PROTOCOL => (name, import),
            ToPhone::Welcome { protocol, .. } => return Err(Error::Version(protocol)),
            _ => return Err(Error::Refused),
        };

        let shared = Arc::new(Mutex::new(Pending::default()));
        let display = watch::channel(Display { connected: true, ..Display::default() }).0;
        let (outgoing, mut queue) = mpsc::unbounded_channel::<ToHost>();
        let epoch = Instant::now();

        let write_task = tokio::spawn(async move {
            while let Some(message) = queue.recv().await {
                if writer.send(&message).await.is_err() {
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
        let (reading, screen) = (shared.clone(), display.clone());
        let read_task = tokio::spawn(async move {
            while let Ok(message) = reader.recv::<ToPhone>().await {
                match message {
                    ToPhone::State(state) => {
                        let shown = {
                            let mut pending = reading.lock().expect("pending lock");
                            pending.presses.retain(|(seq, _)| *seq > state.applied);
                            pending.confirmed = state;
                            pending.shown()
                        };
                        screen.send_modify(|display| display.state = shown);
                    }
                    ToPhone::Pong { sent } => {
                        let trip = epoch.elapsed().saturating_sub(Duration::from_micros(sent));
                        screen.send_modify(|display| display.round_trip = Some(trip));
                    }
                    ToPhone::Preview { jpeg, .. } => {
                        screen.send_modify(|display| display.preview = Some(Arc::new(jpeg)));
                    }
                    ToPhone::Welcome { .. } => {}
                }
            }
            screen.send_modify(|display| display.connected = false);
        });

        let tasks = vec![write_task, ping_task, read_task];
        Ok(Remote { host, host_name, import, outgoing, shared, display, tasks })
    }

    /// Sends a key and returns the screen as it will read once the desktop applies it.
    pub fn press(&self, command: Command) -> State {
        let (seq, shown) = {
            let mut pending = self.shared.lock().expect("pending lock");
            pending.seq += 1;
            let seq = pending.seq;
            pending.presses.push((seq, command));
            (seq, pending.shown())
        };
        self.display.send_modify(|display| display.state = shown.clone());
        let _ = self.outgoing.send(ToHost::Press { seq, command });
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
