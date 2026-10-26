//! Multicast discovery: announce ourselves, answer announcements, keep a table of peers.

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use socket2::{Domain, Protocol as SockProtocol, Socket, Type};
use tokio::net::UdpSocket;
use tokio::sync::{Semaphore, broadcast};

use crate::model::{Announcement, DEFAULT_PORT, DeviceInfo};
use crate::sender::Target;

pub const MULTICAST_GROUP: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 167);
/// Datagrams get lost on Wi-Fi: the start announcement is repeated after these delays.
const ANNOUNCE_DELAYS: [Duration; 3] = [Duration::ZERO, Duration::from_millis(500), Duration::from_secs(2)];
const REGISTER_TIMEOUT: Duration = Duration::from_secs(5);
/// Peers kept at most; past it the one heard from longest ago leaves, so made-up announcements
/// cannot grow the table without end.
const PEERS: usize = 64;
/// Answers in flight at once; an announcement past them goes unanswered, so a flood of them
/// cannot turn this device into a sender of requests to wherever they name.
const ANSWERS: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peer {
    pub info: DeviceInfo,
    pub ip: IpAddr,
}

impl Peer {
    /// Where to send files, with the announced fingerprint pinned.
    pub fn target(&self) -> Target {
        Target {
            addr: SocketAddr::new(self.ip, self.info.port),
            protocol: self.info.protocol,
            fingerprint: Some(self.info.fingerprint.clone()),
        }
    }
}

/// Known devices by fingerprint. Cloning shares the table.
#[derive(Clone)]
pub struct Peers {
    own_fingerprint: String,
    table: Arc<Mutex<Table>>,
    changes: broadcast::Sender<Peer>,
}

/// Each peer with the tick it was last heard at.
#[derive(Default)]
struct Table {
    peers: HashMap<String, (Peer, u64)>,
    tick: u64,
}

impl Peers {
    pub fn new(own_fingerprint: impl Into<String>) -> Self {
        Self { own_fingerprint: own_fingerprint.into(), table: Default::default(), changes: broadcast::channel(64).0 }
    }

    /// Records a device, ignoring ourselves; subscribers hear of new or changed peers.
    pub fn insert(&self, info: DeviceInfo, ip: IpAddr) {
        if info.fingerprint.is_empty() || info.fingerprint.eq_ignore_ascii_case(&self.own_fingerprint) {
            return;
        }
        let peer = Peer { info, ip };
        let mut table = self.table.lock().unwrap();
        table.tick += 1;
        let tick = table.tick;
        let changed = table.peers.get(&peer.info.fingerprint).is_none_or(|(known, _)| *known != peer);
        table.peers.insert(peer.info.fingerprint.clone(), (peer.clone(), tick));
        if table.peers.len() > PEERS {
            let oldest = table.peers.iter().min_by_key(|(_, (_, heard))| *heard).map(|(key, _)| key.clone());
            if let Some(oldest) = oldest {
                table.peers.remove(&oldest);
            }
        }
        if changed {
            let _ = self.changes.send(peer);
        }
    }

    pub fn list(&self) -> Vec<Peer> {
        self.table.lock().unwrap().peers.values().map(|(peer, _)| peer.clone()).collect()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Peer> {
        self.changes.subscribe()
    }
}

pub struct DiscoveryConfig {
    /// Our own info, with the receiver's port.
    pub info: DeviceInfo,
    pub multicast_port: u16,
    pub group: Ipv4Addr,
    /// The interface that joins and sends; unspecified lets the OS pick its default route.
    pub interface: Ipv4Addr,
}

impl DiscoveryConfig {
    pub fn new(info: DeviceInfo) -> Self {
        Self { info, multicast_port: DEFAULT_PORT, group: MULTICAST_GROUP, interface: Ipv4Addr::UNSPECIFIED }
    }
}

/// A running discovery; dropping it stops listening.
pub struct Discovery {
    inner: Arc<Inner>,
    task: tokio::task::JoinHandle<()>,
}

struct Inner {
    info: DeviceInfo,
    socket: UdpSocket,
    group: SocketAddrV4,
    peers: Peers,
    client: reqwest::Client,
    answering: Arc<Semaphore>,
}

impl Discovery {
    /// Joins the group, starts answering and announces. Fails where the network has no multicast.
    pub async fn start(config: DiscoveryConfig, peers: Peers) -> io::Result<Self> {
        let socket =
            UdpSocket::from_std(multicast_socket(config.group, config.multicast_port, config.interface)?.into())?;
        let inner = Arc::new(Inner {
            group: SocketAddrV4::new(config.group, config.multicast_port),
            info: config.info,
            socket,
            peers,
            client: crate::tls::https_client(None).map_err(io::Error::other)?,
            answering: Arc::new(Semaphore::new(ANSWERS)),
        });
        let task = tokio::spawn(listen(inner.clone()));
        let announcer = inner.clone();
        tokio::spawn(async move {
            for delay in ANNOUNCE_DELAYS {
                tokio::time::sleep(delay).await;
                if let Err(e) = announcer.send(true).await {
                    tracing::warn!("announce failed: {e}");
                }
            }
        });
        Ok(Self { inner, task })
    }

    pub fn peers(&self) -> &Peers {
        &self.inner.peers
    }
}

impl Drop for Discovery {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Inner {
    async fn send(&self, announce: bool) -> io::Result<()> {
        let datagram = serde_json::to_vec(&Announcement { info: self.info.clone(), announce })?;
        self.socket.send_to(&datagram, self.group).await.map(|_| ())
    }

    /// Answers by `register` over HTTP, and by a multicast datagram when that fails.
    async fn answer(&self, peer: &DeviceInfo, ip: IpAddr) {
        let url = format!("{}://{}/api/localsend/v2/register", peer.protocol.scheme(), SocketAddr::new(ip, peer.port));
        let posted = self.client.post(url).json(&self.info).timeout(REGISTER_TIMEOUT).send().await;
        if !posted.is_ok_and(|r| r.status().is_success())
            && let Err(e) = self.send(false).await
        {
            tracing::warn!("answer failed: {e}");
        }
    }
}

async fn listen(inner: Arc<Inner>) {
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let (len, from) = match inner.socket.recv_from(&mut buf).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("multicast receive failed: {e}");
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        let Ok(message) = serde_json::from_slice::<Announcement>(&buf[..len]) else { continue };
        if message.info.fingerprint.eq_ignore_ascii_case(&inner.info.fingerprint) {
            continue;
        }
        inner.peers.insert(message.info.clone(), from.ip());
        if message.announce
            && let Ok(permit) = inner.answering.clone().try_acquire_owned()
        {
            let inner = inner.clone();
            tokio::spawn(async move {
                inner.answer(&message.info, from.ip()).await;
                drop(permit);
            });
        }
    }
}

/// Shares the port with another LocalSend app on the same machine where the OS allows it.
fn multicast_socket(group: Ipv4Addr, port: u16, interface: Ipv4Addr) -> io::Result<Socket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(SockProtocol::UDP))?;
    socket.set_reuse_address(true)?;
    #[cfg(all(unix, not(any(target_os = "solaris", target_os = "illumos"))))]
    socket.set_reuse_port(true)?;
    socket.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port).into())?;
    socket.join_multicast_v4(&group, &interface)?;
    if !interface.is_unspecified() {
        socket.set_multicast_if_v4(&interface)?;
    }
    socket.set_multicast_loop_v4(true)?;
    socket.set_nonblocking(true)?;
    Ok(socket)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DeviceType, PROTOCOL_VERSION, Protocol};

    fn device(fingerprint: &str) -> DeviceInfo {
        DeviceInfo {
            alias: "Phone".into(),
            version: PROTOCOL_VERSION.into(),
            device_model: None,
            device_type: Some(DeviceType::Mobile),
            fingerprint: fingerprint.into(),
            port: DEFAULT_PORT,
            protocol: Protocol::Https,
            download: false,
        }
    }

    #[test]
    fn a_flood_of_announcements_keeps_the_table_bounded_and_the_peers_heard_lately() {
        let peers = Peers::new("me");
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20));
        peers.insert(device("phone"), ip);
        peers.insert(device("tablet"), ip);
        for n in 0..PEERS * 4 {
            peers.insert(device(&format!("made-up {n}")), ip);
            if n % 10 == 0 {
                peers.insert(device("phone"), ip);
            }
        }
        let known: Vec<String> = peers.list().into_iter().map(|p| p.info.fingerprint).collect();
        assert_eq!(known.len(), PEERS);
        assert!(known.contains(&"phone".to_string()), "a peer that keeps announcing stays");
        // Twin: a peer silent through the flood is the one that leaves.
        assert!(!known.contains(&"tablet".to_string()));
    }
}
