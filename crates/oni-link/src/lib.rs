//! ONI Link: the scanner keypad of Open Negative Initiative, pressed from a phone.
//! One protocol over any byte stream (TCP on the local network, a Bluetooth channel), encrypted by Noise.

pub mod host;
pub mod noise;
pub mod pairing;
pub mod remote;
pub mod scanner;
pub mod wire;

pub use host::{Host, HostEvent, HostEvents, Receipt};
pub use noise::{Identity, PublicKey};
pub use pairing::PairingCode;
pub use remote::{Display, Remote};
pub use scanner::{Channel, Clicks, Command, Effect, Letter, Machine, State};
pub use wire::Import;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
    #[error("noise: {0}")]
    Noise(#[from] snow::Error),
    #[error("encoding: {0}")]
    Encoding(#[from] postcard::Error),
    #[error("message of {0} bytes exceeds one frame")]
    TooLarge(usize),
    #[error("pairing code: bad {0}")]
    BadCode(&'static str),
    #[error("peer speaks protocol {0}")]
    Version(u16),
    #[error("refused by the peer")]
    Refused,
    #[error("no address answered")]
    Unreachable,
}
