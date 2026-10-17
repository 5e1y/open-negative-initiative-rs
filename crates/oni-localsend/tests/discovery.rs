mod common;

use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use oni_localsend::*;

async fn node(dst: &std::path::Path, alias: &str, multicast_port: u16) -> (Receiver, Discovery, Peers) {
    let identity = Identity::generate().unwrap();
    let peers = Peers::new(identity.fingerprint());
    let mut config = ReceiverConfig::new(alias, identity, dst);
    config.port = 0;
    config.bind = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
    config.peers = Some(peers.clone());
    let (receiver, _events) = Receiver::start(config).await.unwrap();
    let mut discovery = DiscoveryConfig::new(receiver.info().clone());
    discovery.multicast_port = multicast_port;
    let discovery = Discovery::start(discovery, peers.clone()).await.unwrap();
    (receiver, discovery, peers)
}

async fn until_found(peers: &Peers, fingerprint: &str) -> Peer {
    for _ in 0..100 {
        if let Some(p) = peers.list().into_iter().find(|p| p.info.fingerprint == fingerprint) {
            return p;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("{fingerprint} not discovered within five seconds");
}

/// B starts after A's announcements are over, so B can only learn of A through A's answer to B.
#[tokio::test]
async fn two_nodes_find_each_other_by_announcement_and_answer() {
    let port = 40000 + rand::random::<u16>() % 20000;
    let (da, db) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (a, _da, peers_a) = node(da.path(), "A", port).await;
    tokio::time::sleep(Duration::from_millis(2600)).await;
    let (b, _db, peers_b) = node(db.path(), "B", port).await;
    let mut changes_b = peers_b.subscribe();

    let b_seen_by_a = until_found(&peers_a, &b.info().fingerprint).await;
    let a_seen_by_b = until_found(&peers_b, &a.info().fingerprint).await;
    assert_eq!((b_seen_by_a.info.alias.as_str(), b_seen_by_a.info.port), ("B", b.local_addr().port()));
    assert_eq!((a_seen_by_b.info.alias.as_str(), a_seen_by_b.info.port), ("A", a.local_addr().port()));
    assert_eq!(changes_b.recv().await.unwrap().info.fingerprint, a.info().fingerprint);
    assert!(peers_a.list().iter().all(|p| p.info.fingerprint != a.info().fingerprint), "no self-discovery");

    // A discovered peer is a pinned target files can go to.
    let src = tempfile::tempdir().unwrap();
    let file = src.path().join("found.dng");
    std::fs::write(&file, b"via discovery").unwrap();
    send_files(b.info(), &a_seen_by_b.target(), &[file], None).await.unwrap();
    assert_eq!(common::names(da.path()), ["found.dng"]);
}
