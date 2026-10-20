//! The phone side of ONI Link for Swift and Kotlin: identity, pairing code, and a remote session
//! whose screen reaches the app through one listener.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use oni_link::{Identity, Import, PairingCode, PublicKey, Remote};
use oni_localsend::{DeviceInfo, DeviceType, Protocol, SendError, Target};
use tokio::runtime::Runtime;
use tokio::sync::watch;
use tokio::task::JoinHandle;

uniffi::setup_scaffolding!();

/// One runtime for every session: the foreign side awaits its join handles, never drives tokio itself.
fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("oni-link")
            .enable_all()
            .build()
            .expect("the link runtime starts")
    })
}

// MARK: Errors

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum LinkError {
    #[error("bad {what}")]
    BadCode { what: String },
    #[error("the desktop speaks protocol {peer}")]
    Version { peer: u16 },
    #[error("refused by the desktop")]
    Refused,
    #[error("no address answered")]
    Unreachable,
    #[error("the desktop does not take photos")]
    ImportUnavailable,
    #[error("the desktop refused the photos")]
    ImportRefused,
    #[error("{reason}")]
    Failed { reason: String },
}

impl From<oni_link::Error> for LinkError {
    fn from(error: oni_link::Error) -> Self {
        match error {
            oni_link::Error::BadCode(what) => LinkError::BadCode { what: what.to_owned() },
            oni_link::Error::Version(peer) => LinkError::Version { peer },
            oni_link::Error::Refused => LinkError::Refused,
            oni_link::Error::Unreachable => LinkError::Unreachable,
            other => LinkError::Failed { reason: other.to_string() },
        }
    }
}

impl From<SendError> for LinkError {
    fn from(error: SendError) -> Self {
        match error {
            SendError::PinRequired | SendError::Rejected => LinkError::ImportRefused,
            other => LinkError::Failed { reason: other.to_string() },
        }
    }
}

fn bad(what: &str) -> LinkError {
    LinkError::BadCode { what: what.to_owned() }
}

// MARK: Identity and pairing code

/// A new key pair as 64 bytes, private then public, for the app's Keychain.
#[uniffi::export]
pub fn generate_identity() -> Vec<u8> {
    Identity::generate().to_bytes()
}

/// The public half of stored identity bytes; fails on bytes that are not an identity.
#[uniffi::export]
pub fn identity_public_key(identity: Vec<u8>) -> Result<Vec<u8>, LinkError> {
    Ok(Identity::from_bytes(&identity)?.public().0.to_vec())
}

/// What the QR code says about the desktop; its one-time secret stays inside the URI.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct PairingInfo {
    pub host_key: Vec<u8>,
    pub name: String,
    pub addresses: Vec<String>,
}

#[uniffi::export]
pub fn parse_pairing_code(uri: String) -> Result<PairingInfo, LinkError> {
    let code = PairingCode::parse(&uri)?;
    Ok(PairingInfo {
        host_key: code.host.0.to_vec(),
        name: code.name,
        addresses: code.addresses.iter().map(SocketAddr::to_string).collect(),
    })
}

fn host_key(bytes: &[u8]) -> Result<PublicKey, LinkError> {
    Ok(PublicKey(bytes.try_into().map_err(|_| bad("key"))?))
}

fn socket_addresses(addresses: &[String]) -> Result<Vec<SocketAddr>, LinkError> {
    addresses.iter().map(|a| a.parse().map_err(|_| bad("address"))).collect()
}

// MARK: The screens

mod panel;
mod scanner;
pub use panel::{Face, Panel, PanelRow, PanelTab, Param};
pub use scanner::*;

/// The files the desktop took, and those it declined, as the paths given.
#[derive(Clone, Debug, Default, PartialEq, uniffi::Record)]
pub struct SendResult {
    pub sent: Vec<String>,
    pub skipped: Vec<String>,
}

// MARK: The session

/// Called on a link thread after every change of the screen, the connection or the round trip.
#[uniffi::export(callback_interface)]
pub trait DisplayListener: Send + Sync {
    fn on_display(&self, display: DisplayState);
}

#[derive(uniffi::Object)]
pub struct RemoteSession {
    remote: Mutex<Option<Remote>>,
    display: watch::Receiver<oni_link::Display>,
    listening: Mutex<Option<JoinHandle<()>>>,
    host_key: Vec<u8>,
    host_name: String,
    /// The desktop's address as dialed: its LocalSend receiver listens on the same host.
    peer: SocketAddr,
    import: Option<Import>,
    sender: DeviceInfo,
}

#[uniffi::export]
impl RemoteSession {
    /// The first meeting, from the QR code's URI: the desktop then trusts this identity.
    #[uniffi::constructor]
    pub async fn pair(code_uri: String, identity: Vec<u8>, phone_name: String) -> Result<Arc<Self>, LinkError> {
        let code = PairingCode::parse(&code_uri)?;
        let me = Identity::from_bytes(&identity)?;
        let sender = sender(&me, &phone_name);
        let opened = runtime().spawn(async move {
            let stream = Remote::dial(&code.addresses).await?;
            let peer = stream.peer_addr()?;
            Ok((Remote::pair(stream, &me, &code, &phone_name).await?, peer))
        });
        Self::wrap(opened, sender).await
    }

    /// A later meeting with a desktop that already trusts this identity.
    #[uniffi::constructor]
    pub async fn connect(
        addresses: Vec<String>,
        host_key: Vec<u8>,
        identity: Vec<u8>,
        phone_name: String,
    ) -> Result<Arc<Self>, LinkError> {
        let addresses = socket_addresses(&addresses)?;
        let host = self::host_key(&host_key)?;
        let me = Identity::from_bytes(&identity)?;
        let sender = sender(&me, &phone_name);
        let opened = runtime().spawn(async move {
            let stream = Remote::dial(&addresses).await?;
            let peer = stream.peer_addr()?;
            Ok((Remote::connect(stream, &me, host, &phone_name).await?, peer))
        });
        Self::wrap(opened, sender).await
    }

    /// Sends a key and returns the screen as it will read once the desktop applies it.
    pub fn press(&self, command: Command) -> ScannerState {
        match self.remote.lock().expect("remote lock").as_ref() {
            Some(remote) => remote.press(command.into()).into(),
            None => self.display.borrow().state.clone().into(),
        }
    }

    pub fn current(&self) -> DisplayState {
        let mut display = DisplayState::from(&*self.display.borrow());
        display.connected &= self.remote.lock().expect("remote lock").is_some();
        display
    }

    /// Feeds `listener` the current screen, then every change; a second call replaces the first.
    pub fn set_listener(&self, listener: Box<dyn DisplayListener>) {
        let mut display = self.display.clone();
        let task = runtime().spawn(async move {
            loop {
                let shown = DisplayState::from(&*display.borrow_and_update());
                listener.on_display(shown);
                if display.changed().await.is_err() {
                    break;
                }
            }
        });
        if let Some(previous) = self.listening.lock().expect("listener lock").replace(task) {
            previous.abort();
        }
    }

    /// The last frame preview the desktop sent, as JPEG bytes.
    pub fn preview(&self) -> Option<Vec<u8>> {
        self.display.borrow().preview.as_ref().map(|jpeg| jpeg.to_vec())
    }

    pub fn host_key(&self) -> Vec<u8> {
        self.host_key.clone()
    }

    /// True when the desktop announced a receiver for photos.
    pub fn can_send_files(&self) -> bool {
        self.import.is_some()
    }

    /// Sends files byte for byte to the desktop's LocalSend receiver, which takes them without asking.
    pub async fn send_files(&self, paths: Vec<String>) -> Result<SendResult, LinkError> {
        let import = self.import.clone().ok_or(LinkError::ImportUnavailable)?;
        let target = Target {
            addr: SocketAddr::new(self.peer.ip(), import.port),
            protocol: Protocol::Https,
            fingerprint: Some(import.fingerprint),
        };
        let sender = self.sender.clone();
        let files: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        let sending = runtime()
            .spawn(async move { oni_localsend::send_files(&sender, &target, &files, Some(&import.pin)).await });
        let report = sending.await.map_err(|error| LinkError::Failed { reason: error.to_string() })??;
        let shown = |files: Vec<PathBuf>| files.into_iter().map(|p| p.to_string_lossy().into_owned()).collect();
        Ok(SendResult { sent: shown(report.sent), skipped: shown(report.skipped) })
    }

    pub fn host_name(&self) -> String {
        self.host_name.clone()
    }

    /// Hangs up and stops the listener; the session cannot be reopened.
    pub fn hang_up(&self) {
        if let Some(task) = self.listening.lock().expect("listener lock").take() {
            task.abort();
        }
        drop(self.remote.lock().expect("remote lock").take());
    }
}

impl RemoteSession {
    async fn wrap(
        opened: JoinHandle<Result<(Remote, SocketAddr), oni_link::Error>>,
        sender: DeviceInfo,
    ) -> Result<Arc<Self>, LinkError> {
        let (remote, peer) = opened.await.map_err(|error| LinkError::Failed { reason: error.to_string() })??;
        Ok(Arc::new(RemoteSession {
            display: remote.display(),
            host_key: remote.host().0.to_vec(),
            host_name: remote.host_name().to_owned(),
            import: remote.import().cloned(),
            peer,
            sender,
            listening: Mutex::new(None),
            remote: Mutex::new(Some(remote)),
        }))
    }
}

/// The phone as LocalSend names it. No client certificate proves a fingerprint, so the Noise key's hex
/// serves: stable per phone, and nothing more to store.
fn sender(me: &Identity, phone_name: &str) -> DeviceInfo {
    DeviceInfo {
        alias: phone_name.to_owned(),
        version: oni_localsend::PROTOCOL_VERSION.to_owned(),
        device_model: None,
        device_type: Some(DeviceType::Mobile),
        fingerprint: me.public().0.iter().map(|b| format!("{b:02x}")).collect(),
        port: oni_localsend::DEFAULT_PORT,
        protocol: Protocol::Https,
        download: false,
    }
}

impl Drop for RemoteSession {
    fn drop(&mut self) {
        self.hang_up();
    }
}

#[cfg(test)]
mod tests;
