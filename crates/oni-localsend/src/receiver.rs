//! The upload API: an HTTPS server that accepts one session at a time and streams files to disk.

use std::collections::{HashMap, HashSet};
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{ConnectInfo, DefaultBodyLimit, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use futures_util::StreamExt;
use futures_util::future::BoxFuture;
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_rustls::TlsAcceptor;
use tokio_util::sync::CancellationToken;

use crate::discovery::Peers;
use crate::identity::Identity;
use crate::inbox::{PartFile, refused_name, sanitize_file_name};
use crate::model::*;

mod routes;
mod serving;

use routes::router;
use serving::{Limits, serve};

/// A policy that has not decided by then rejects, so a stranger's question never holds the slot.
const DECIDE: Duration = Duration::from_secs(120);
/// A session that receives no byte for this long is cancelled, so a vanished sender frees the slot.
const SESSION_IDLE: Duration = Duration::from_secs(60);
const PROGRESS_STEP: u64 = 1 << 20;
const METADATA_LIMIT: usize = 8 << 20;

/// What the receiver does with an incoming transfer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    AcceptAll,
    /// Accept only these file ids.
    Accept(HashSet<String>),
    Reject,
}

/// Decides on each `prepare-upload`; the sender waits for the answer, so a policy may ask a person.
/// `pin` is the sender's `?pin=`, carried inside TLS, so a policy may trust a secret it handed out.
pub trait AcceptPolicy: Send + Sync + 'static {
    fn decide<'a>(
        &'a self,
        sender: &'a DeviceInfo,
        files: &'a [FileMeta],
        pin: Option<&'a str>,
    ) -> BoxFuture<'a, Decision>;
}

pub struct AcceptAll;

impl AcceptPolicy for AcceptAll {
    fn decide<'a>(&'a self, _: &'a DeviceInfo, _: &'a [FileMeta], _: Option<&'a str>) -> BoxFuture<'a, Decision> {
        Box::pin(async { Decision::AcceptAll })
    }
}

/// Accepts senders whose fingerprint is in the set, rejects the others.
/// Over HTTPS the sender's fingerprint is only what its JSON claims: no client certificate proves it.
pub struct AcceptFingerprints(pub HashSet<String>);

impl AcceptPolicy for AcceptFingerprints {
    fn decide<'a>(&'a self, sender: &'a DeviceInfo, _: &'a [FileMeta], _: Option<&'a str>) -> BoxFuture<'a, Decision> {
        let known = self.0.iter().any(|f| f.eq_ignore_ascii_case(&sender.fingerprint));
        Box::pin(async move { if known { Decision::AcceptAll } else { Decision::Reject } })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// Accepted files only.
    SessionStarted {
        session_id: String,
        sender: DeviceInfo,
        address: IpAddr,
        files: Vec<FileMeta>,
    },
    FileProgress {
        session_id: String,
        file_id: String,
        received: u64,
        size: u64,
    },
    FileDone {
        session_id: String,
        file_id: String,
        path: PathBuf,
    },
    FileFailed {
        session_id: String,
        file_id: String,
        reason: String,
    },
    /// Every accepted file is done or failed.
    SessionFinished {
        session_id: String,
    },
    SessionCancelled {
        session_id: String,
    },
}

pub struct ReceiverConfig {
    pub alias: String,
    pub device_model: Option<String>,
    pub device_type: DeviceType,
    pub identity: Identity,
    pub bind: IpAddr,
    /// 0 picks a free port.
    pub port: u16,
    /// Received files land here, under their sanitised names, and nowhere else.
    pub destination: PathBuf,
    pub pin: Option<String>,
    pub policy: Arc<dyn AcceptPolicy>,
    /// Devices calling `register` are recorded here.
    pub peers: Option<Peers>,
}

impl ReceiverConfig {
    pub fn new(alias: impl Into<String>, identity: Identity, destination: impl Into<PathBuf>) -> Self {
        Self {
            alias: alias.into(),
            device_model: None,
            device_type: DeviceType::Desktop,
            identity,
            bind: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: DEFAULT_PORT,
            destination: destination.into(),
            pin: None,
            policy: Arc::new(AcceptAll),
            peers: None,
        }
    }
}

/// A running receiver; dropping it stops the server.
pub struct Receiver {
    shared: Arc<Shared>,
    local_addr: SocketAddr,
    shutdown: CancellationToken,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl Receiver {
    pub async fn start(config: ReceiverConfig) -> io::Result<(Self, mpsc::UnboundedReceiver<Event>)> {
        tokio::fs::create_dir_all(&config.destination).await?;
        let tls = crate::tls::server_config(&config.identity).map_err(io::Error::other)?;
        let listener = TcpListener::bind((config.bind, config.port)).await?;
        let local_addr = listener.local_addr()?;
        let (events, rx) = mpsc::unbounded_channel();
        let shared = Arc::new(Shared {
            info: DeviceInfo {
                alias: config.alias,
                version: PROTOCOL_VERSION.into(),
                device_model: config.device_model,
                device_type: Some(config.device_type),
                fingerprint: config.identity.fingerprint().into(),
                port: local_addr.port(),
                protocol: Protocol::Https,
                download: false,
            },
            destination: config.destination,
            pin: config.pin,
            policy: config.policy,
            peers: config.peers,
            events,
            slot: Mutex::new(Slot::Free),
            rename_lock: Arc::new(Mutex::new(())),
        });
        let shutdown = CancellationToken::new();
        let acceptor = TlsAcceptor::from(Arc::new(tls));
        let task = tokio::spawn(serve(listener, acceptor, router(shared.clone()), shutdown.clone(), Limits::DEFAULT));
        Ok((Self { shared, local_addr, shutdown, task: Some(task) }, rx))
    }

    /// Our device info, with the bound port: what discovery announces and `register` answers.
    pub fn info(&self) -> &DeviceInfo {
        &self.shared.info
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Cancels the active session, deleting its unfinished files.
    pub fn cancel_session(&self) {
        let id = match &*self.shared.slot.lock().unwrap() {
            Slot::Active(s) => s.id.clone(),
            _ => return,
        };
        self.shared.cancel(&id);
    }

    pub async fn shutdown(mut self) {
        self.shutdown.cancel();
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

struct Shared {
    info: DeviceInfo,
    destination: PathBuf,
    pin: Option<String>,
    policy: Arc<dyn AcceptPolicy>,
    peers: Option<Peers>,
    events: mpsc::UnboundedSender<Event>,
    slot: Mutex<Slot>,
    rename_lock: Arc<Mutex<()>>,
}

enum Slot {
    Free,
    /// The policy is deciding; the slot is already taken.
    Deciding,
    Active(Arc<Session>),
}

struct Session {
    id: String,
    sender_ip: IpAddr,
    files: Mutex<HashMap<String, FileState>>,
    cancel: CancellationToken,
    started: Instant,
    last_activity_ms: AtomicU64,
}

struct FileState {
    meta: FileMeta,
    token: String,
    status: Status,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Waiting,
    Receiving,
    Done,
    Failed,
}

impl Session {
    fn touch(&self) {
        self.last_activity_ms.store(self.started.elapsed().as_millis() as u64, Ordering::Relaxed);
    }

    fn idle(&self) -> Duration {
        self.started.elapsed().saturating_sub(Duration::from_millis(self.last_activity_ms.load(Ordering::Relaxed)))
    }
}

impl Shared {
    fn emit(&self, event: Event) {
        let _ = self.events.send(event);
    }

    fn active(&self, id: &str) -> Option<Arc<Session>> {
        match &*self.slot.lock().unwrap() {
            Slot::Active(s) if s.id == id => Some(s.clone()),
            _ => None,
        }
    }

    /// Frees the slot if it still holds session `id`, returning that session.
    fn release(&self, id: &str) -> Option<Arc<Session>> {
        let mut slot = self.slot.lock().unwrap();
        match &*slot {
            Slot::Active(s) if s.id == id => {
                let Slot::Active(s) = std::mem::replace(&mut *slot, Slot::Free) else { unreachable!() };
                Some(s)
            }
            _ => None,
        }
    }

    fn cancel(&self, id: &str) {
        if let Some(session) = self.release(id) {
            session.cancel.cancel();
            self.emit(Event::SessionCancelled { session_id: session.id.clone() });
        }
    }

    fn finish_if_complete(&self, session: &Session) {
        let complete =
            session.files.lock().unwrap().values().all(|f| matches!(f.status, Status::Done | Status::Failed));
        if complete && self.release(&session.id).is_some() {
            self.emit(Event::SessionFinished { session_id: session.id.clone() });
        }
    }
}

#[cfg(test)]
mod tests;
