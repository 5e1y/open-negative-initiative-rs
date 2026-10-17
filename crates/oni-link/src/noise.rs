//! The encrypted channel: Noise IK, where the phone already knows the desktop's key.
//! Pairing adds the one-time secret from the QR code as a pre-shared key on the first message.

use std::fmt;
use std::sync::{Arc, Mutex};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Serialize, de::DeserializeOwned};
use snow::{Builder, HandshakeState, TransportState};
use tokio::io::{AsyncRead, AsyncWrite, ReadHalf, WriteHalf};

use crate::Error;
use crate::wire::{self, MAX_PLAINTEXT};

const PAIRED: &str = "Noise_IK_25519_ChaChaPoly_BLAKE2s";
const PAIRING: &str = "Noise_IKpsk1_25519_ChaChaPoly_BLAKE2s";

/// The first byte on the wire, before the handshake: the responder must know the pattern to read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Paired = 0,
    Pairing = 1,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PublicKey(pub [u8; 32]);

impl PublicKey {
    pub fn to_base64(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0)
    }

    pub fn from_base64(text: &str) -> Result<Self, Error> {
        let bytes = URL_SAFE_NO_PAD.decode(text).map_err(|_| Error::BadCode("key"))?;
        Ok(PublicKey(bytes.try_into().map_err(|_| Error::BadCode("key"))?))
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicKey({})", &self.to_base64()[..8])
    }
}

/// A device's long-lived key pair, kept by the app in its own secure storage.
#[derive(Clone)]
pub struct Identity {
    private: [u8; 32],
    public: PublicKey,
}

impl Identity {
    pub fn generate() -> Self {
        let pair = Builder::new(PAIRED.parse().expect("valid pattern"))
            .generate_keypair()
            .expect("the default resolver generates keys");
        Identity {
            private: pair.private.try_into().expect("32-byte key"),
            public: PublicKey(pair.public.try_into().expect("32-byte key")),
        }
    }

    /// The 64 bytes `to_bytes` wrote: the private key, then the public one.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != 64 {
            return Err(Error::BadCode("identity"));
        }
        Ok(Identity {
            private: bytes[..32].try_into().expect("checked length"),
            public: PublicKey(bytes[32..].try_into().expect("checked length")),
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        [self.private.as_slice(), self.public.0.as_slice()].concat()
    }

    pub fn public(&self) -> PublicKey {
        self.public
    }

    pub(crate) fn private(&self) -> &[u8; 32] {
        &self.private
    }
}

pub async fn initiate<S>(
    mut stream: S,
    me: &Identity,
    host: PublicKey,
    secret: Option<[u8; 32]>,
) -> Result<(Reader<S>, Writer<S>), Error>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mode = if secret.is_some() { Mode::Pairing } else { Mode::Paired };
    let builder = Builder::new(pattern(mode).parse()?).local_private_key(&me.private)?.remote_public_key(&host.0)?;
    let mut handshake = match &secret {
        Some(secret) => builder.psk(1, secret)?.build_initiator()?,
        None => builder.build_initiator()?,
    };
    let mut buf = vec![0u8; 65_535];
    let len = handshake.write_message(&[], &mut buf)?;
    let mut first = vec![mode as u8];
    first.extend_from_slice(&buf[..len]);
    wire::write_frame(&mut stream, &first).await?;
    // The desktop hangs up rather than answer a wrong secret or a stranger's key.
    let reply = wire::read_frame(&mut stream).await.map_err(|_| Error::Refused)?;
    handshake.read_message(&reply, &mut buf).map_err(|_| Error::Refused)?;
    split(stream, handshake)
}

/// The desktop's side. `secret` is the open pairing window's, `None` when no window is open.
pub async fn respond<S>(
    mut stream: S,
    me: &Identity,
    secret: Option<[u8; 32]>,
) -> Result<(Mode, PublicKey, Reader<S>, Writer<S>), Error>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let first = wire::read_frame(&mut stream).await?;
    let (&byte, message) = first.split_first().ok_or(Error::Refused)?;
    let mode = match byte {
        0 => Mode::Paired,
        1 => Mode::Pairing,
        _ => return Err(Error::Refused),
    };
    let builder = Builder::new(pattern(mode).parse()?).local_private_key(&me.private)?;
    let mut handshake = match (mode, secret) {
        (Mode::Paired, _) => builder.build_responder()?,
        (Mode::Pairing, Some(secret)) => builder.psk(1, &secret)?.build_responder()?,
        (Mode::Pairing, None) => return Err(Error::Refused),
    };
    let mut buf = vec![0u8; 65_535];
    handshake.read_message(message, &mut buf).map_err(|_| Error::Refused)?;
    let remote = handshake.get_remote_static().ok_or(Error::Refused)?;
    let remote = PublicKey(remote.try_into().map_err(|_| Error::Refused)?);
    let len = handshake.write_message(&[], &mut buf)?;
    wire::write_frame(&mut stream, &buf[..len]).await?;
    let (reader, writer) = split(stream, handshake)?;
    Ok((mode, remote, reader, writer))
}

fn pattern(mode: Mode) -> &'static str {
    match mode {
        Mode::Paired => PAIRED,
        Mode::Pairing => PAIRING,
    }
}

fn split<S: AsyncRead + AsyncWrite>(stream: S, handshake: HandshakeState) -> Result<(Reader<S>, Writer<S>), Error> {
    let transport = Arc::new(Mutex::new(handshake.into_transport_mode()?));
    let (read, write) = tokio::io::split(stream);
    Ok((Reader { stream: read, transport: transport.clone() }, Writer { stream: write, transport }))
}

/// Each direction keeps its own nonce, so the lock is held for the cipher only, never for I/O.
pub struct Reader<S> {
    stream: ReadHalf<S>,
    transport: Arc<Mutex<TransportState>>,
}

pub struct Writer<S> {
    stream: WriteHalf<S>,
    transport: Arc<Mutex<TransportState>>,
}

impl<S: AsyncRead> Reader<S> {
    pub async fn recv<T: DeserializeOwned>(&mut self) -> Result<T, Error> {
        let frame = wire::read_frame(&mut self.stream).await?;
        let mut plain = vec![0u8; frame.len()];
        let len = self.transport.lock().expect("cipher lock").read_message(&frame, &mut plain)?;
        wire::decode(&plain[..len])
    }
}

impl<S: AsyncWrite> Writer<S> {
    pub async fn send<T: Serialize>(&mut self, message: &T) -> Result<(), Error> {
        let plain = wire::encode(message)?;
        let mut frame = vec![0u8; MAX_PLAINTEXT + 16];
        let len = self.transport.lock().expect("cipher lock").write_message(&plain, &mut frame)?;
        wire::write_frame(&mut self.stream, &frame[..len]).await
    }
}
