use super::*;
use crate::remote::Remote;
use tokio::io::DuplexStream;

/// An accept that hands out what the test queues, an error as the kernel would return it included.
fn queued() -> (mpsc::UnboundedSender<io::Result<DuplexStream>>, impl FnMut() -> std::pin::Pin<Box<dyn Future<Output = io::Result<DuplexStream>> + Send>>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let rx = Arc::new(tokio::sync::Mutex::new(rx));
    let accept = move || {
        let rx = rx.clone();
        Box::pin(async move { rx.lock().await.recv().await.unwrap_or_else(|| Err(io::Error::other("closed"))) })
            as std::pin::Pin<Box<dyn Future<Output = io::Result<DuplexStream>> + Send>>
    };
    (tx, accept)
}

#[tokio::test]
async fn a_failed_accept_leaves_the_listener_serving_the_next_phone() {
    let (host, events) = Host::new(Identity::generate(), "Studio".into(), []);
    let (queue, accept) = queued();
    let weak = Arc::downgrade(&host.shared);
    tokio::spawn(accepting(weak.clone(), accept));
    queue.send(Err(io::Error::from_raw_os_error(24))).unwrap();
    let (desk, phone) = tokio::io::duplex(1 << 16);
    queue.send(Ok(desk)).unwrap();
    let code = host.open_pairing(Vec::new());
    let first = Remote::pair(phone, &Identity::generate(), &code, "Phone").await;
    assert!(first.is_ok(), "served after the error");

    // Twin: once the host is gone the loop stops, and a stream accepted then is never answered.
    let code = host.open_pairing(Vec::new());
    drop((first, host, events));
    timeout(Duration::from_secs(5), async {
        while weak.strong_count() > 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let (desk, phone) = tokio::io::duplex(1 << 16);
    let _ = queue.send(Ok(desk));
    assert!(Remote::pair(phone, &Identity::generate(), &code, "Phone").await.is_err());
}

#[tokio::test]
async fn a_flood_of_silent_connections_closes_the_next_until_one_ends() {
    let (host, _events) = Host::new(Identity::generate(), "Studio".into(), []);
    let mut silent: Vec<DuplexStream> = (0..HANDSHAKES)
        .map(|_| {
            let (desk, phone) = tokio::io::duplex(1 << 16);
            host.serve(desk);
            phone
        })
        .collect();
    let code = host.open_pairing(Vec::new());
    let (desk, phone) = tokio::io::duplex(1 << 16);
    host.serve(desk);
    assert!(Remote::pair(phone, &Identity::generate(), &code, "Phone").await.is_err(), "past the cap, closed unheard");

    // Twin: one silent connection gone frees its place, and a phone pairs.
    drop(silent.pop());
    tokio::time::timeout(Duration::from_secs(5), async {
        while host.shared.handshakes.available_permits() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let (desk, phone) = tokio::io::duplex(1 << 16);
    host.serve(desk);
    assert!(Remote::pair(phone, &Identity::generate(), &code, "Phone").await.is_ok());
}

#[tokio::test]
async fn a_reconnection_and_its_old_link_carry_their_own_connection() {
    let (host, mut events) = Host::new(Identity::generate(), "Studio".into(), []);
    let me = Identity::generate();
    let code = host.open_pairing(Vec::new());
    let (desk, phone) = tokio::io::duplex(1 << 16);
    host.serve(desk);
    let old = Remote::pair(phone, &me, &code, "Phone").await.unwrap();
    let (desk, phone) = tokio::io::duplex(1 << 16);
    host.serve(desk);
    let new = Remote::connect(phone, &me, host.shared.identity.public(), "Phone").await.unwrap();
    drop(old);
    let mut connected = Vec::new();
    let gone = loop {
        match timeout(Duration::from_secs(5), events.next()).await.unwrap().unwrap() {
            HostEvent::Connected { connection, .. } => connected.push(connection),
            HostEvent::Disconnected { connection, .. } => break connection,
            _ => {}
        }
    };
    assert_eq!(connected.len(), 2);
    assert_eq!(gone, connected[0], "the old link's end names the old link");
    // Twin: the new link is another connection, and it is the one still up.
    assert_ne!(gone, connected[1]);
    assert!(new.display().borrow().connected);
}

#[test]
fn revoke_all_forgets_a_phone_trusted_in_memory_only() {
    let phone = Identity::generate().public();
    let (host, _events) = Host::new(Identity::generate(), "Studio".into(), [phone]);
    let generation = *host.shared.revoked.borrow();
    host.revoke_all();
    assert!(!host.trusts(&phone));
    assert_eq!(*host.shared.revoked.borrow(), generation + 1, "every connection checks its trust again");
    // Twin: revoking another phone leaves this one trusted.
    let (host, _events) = Host::new(Identity::generate(), "Studio".into(), [phone]);
    host.revoke(Identity::generate().public());
    assert!(host.trusts(&phone));
}
