//! A receiver for trying ONI against the LocalSend app on a phone:
//! `cargo run -p oni-localsend --example receive -- <dir>`.

use oni_localsend::*;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let dir = std::env::args().nth(1).expect("usage: receive <destination dir>");
    let identity = Identity::load_or_create(&std::env::temp_dir().join("oni-localsend-example"))?;
    let peers = Peers::new(identity.fingerprint());

    let mut config = ReceiverConfig::new("ONI", identity, &dir);
    config.device_model = Some("ONI".into());
    config.peers = Some(peers.clone());
    let (receiver, mut events) = Receiver::start(config).await?;
    let _discovery = Discovery::start(DiscoveryConfig::new(receiver.info().clone()), peers.clone()).await?;
    println!("receiving into {dir} on {} as {}", receiver.local_addr(), receiver.info().fingerprint);

    let mut found = peers.subscribe();
    loop {
        tokio::select! {
            Some(event) = events.recv() => println!("{event:?}"),
            Ok(peer) = found.recv() => println!("peer {} ({}) at {}", peer.info.alias, peer.info.fingerprint, peer.ip),
            _ = tokio::signal::ctrl_c() => break,
        }
    }
    receiver.shutdown().await;
    Ok(())
}
