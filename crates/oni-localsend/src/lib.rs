//! LocalSend protocol v2 (https://github.com/localsend/protocol): a receiver any LocalSend app can
//! send to, a sender, and multicast discovery.

mod discovery;
mod identity;
mod inbox;
mod model;
mod receiver;
mod sender;
mod tls;

pub use discovery::{Discovery, DiscoveryConfig, MULTICAST_GROUP, Peer, Peers};
pub use identity::{Identity, fingerprint};
pub use inbox::{refused_name, sanitize_file_name};
pub use model::*;
pub use receiver::{AcceptAll, AcceptFingerprints, AcceptPolicy, Decision, Event, Receiver, ReceiverConfig};
pub use sender::{SendError, SendReport, Target, register, send_files};
pub use tls::https_client;

/// 128 random bits in hex: session ids, tokens, file ids, part names.
pub(crate) fn random_id() -> String {
    identity::hex(&rand::random::<[u8; 16]>())
}
