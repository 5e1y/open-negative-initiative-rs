//! The messages, and how they cross a byte stream: a big-endian u16 length, then the bytes.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::Error;
use crate::panel::Panel;
use crate::scanner::{Clicks, Command, State};

/// Raised on any change of meaning. A desktop answers every phone from `OLDEST` on in the phone's
/// own version; a phone wants the desktop's to be its own.
pub const PROTOCOL: u16 = 4;
pub const OLDEST: u16 = 1;
/// From this protocol on, a letter's clicks and steps count half units; before it, whole ones.
pub const HALF_STEPS: u16 = 4;

/// The screen as a phone speaking `protocol` reads it: whole units before `HALF_STEPS`, a half
/// dropped toward zero as the sidecar drops it.
pub fn told(mut state: State, protocol: u16) -> State {
    if protocol < HALF_STEPS {
        let Clicks { d, c, m, y, w } = state.clicks;
        state.clicks = Clicks { d: d / 2, c: c / 2, m: m / 2, y: y / 2, w: w / 2 };
    }
    state
}

/// A press from a phone speaking `protocol`, in this protocol's steps.
pub fn heard(command: Command, protocol: u16) -> Command {
    match command {
        Command::Step { letter, by } if protocol < HALF_STEPS => Command::Step { letter, by: by.saturating_mul(2) },
        other => other,
    }
}

/// A Noise message is at most 65 535 bytes, its tag 16 of them.
pub const MAX_PLAINTEXT: usize = 65_535 - 16;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToHost {
    Hello { protocol: u16, name: String },
    Press { seq: u64, command: Command },
    Ping { sent: u64 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToPhone {
    Welcome {
        protocol: u16,
        name: String,
        import: Option<Import>,
    },
    State(State),
    /// A small JPEG of the rendered frame, for a transport fast enough to carry it.
    Preview {
        revision: u64,
        jpeg: Vec<u8>,
    },
    Pong {
        sent: u64,
    },
    /// The screen with the panel, which replaces `State` from protocol 2 on.
    Screen {
        state: State,
        panel: Panel,
    },
}

/// Where the phone sends photos: the desktop's LocalSend receiver, pinned by its certificate.
/// `pin` is this phone's own secret; the desktop accepts its files without asking.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Import {
    pub fingerprint: String,
    pub port: u16,
    pub pin: String,
}

pub fn encode<T: Serialize>(message: &T) -> Result<Vec<u8>, Error> {
    let bytes = postcard::to_stdvec(message)?;
    if bytes.len() > MAX_PLAINTEXT {
        return Err(Error::TooLarge(bytes.len()));
    }
    Ok(bytes)
}

pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    Ok(postcard::from_bytes(bytes)?)
}

pub async fn write_frame<S: AsyncWrite + Unpin>(stream: &mut S, bytes: &[u8]) -> Result<(), Error> {
    let len = u16::try_from(bytes.len()).map_err(|_| Error::TooLarge(bytes.len()))?;
    let mut frame = Vec::with_capacity(2 + bytes.len());
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(bytes);
    stream.write_all(&frame).await?;
    stream.flush().await?;
    Ok(())
}

pub async fn read_frame<S: AsyncRead + Unpin>(stream: &mut S) -> Result<Vec<u8>, Error> {
    let mut len = [0u8; 2];
    stream.read_exact(&mut len).await?;
    let mut bytes = vec![0u8; usize::from(u16::from_be_bytes(len))];
    stream.read_exact(&mut bytes).await?;
    Ok(bytes)
}
