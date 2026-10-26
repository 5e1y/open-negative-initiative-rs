//! The sending side: prepare an upload with every file hashed, then stream the accepted files.

use std::collections::HashMap;
use std::io::{self, Read};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{StreamExt, TryStreamExt};
use reqwest::StatusCode;
use reqwest::header::CONTENT_LENGTH;
use sha2::{Digest, Sha256};
use tokio_util::io::ReaderStream;

use crate::model::*;

const PARALLEL_UPLOADS: usize = 3;
const READ_CHUNK: usize = 1 << 20;
/// An upload whose body stops draining, or whose answer stops coming, this long has lost its
/// receiver: a phone leaving Wi-Fi mid-send hears of it then, not at TCP's minutes.
const STALL: Duration = Duration::from_secs(30);

/// Where to send. `fingerprint`, when set, is pinned: a peer presenting another certificate is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub addr: SocketAddr,
    pub protocol: Protocol,
    pub fingerprint: Option<String>,
}

impl Target {
    pub fn new(addr: SocketAddr) -> Self {
        Self { addr, protocol: Protocol::Https, fingerprint: None }
    }

    fn url(&self, path: &str) -> String {
        format!("{}://{}/api/localsend/v2/{path}", self.protocol.scheme(), self.addr)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SendError {
    #[error("the receiver's certificate does not match the expected fingerprint")]
    FingerprintMismatch,
    #[error("PIN required or wrong")]
    PinRequired,
    #[error("the receiver rejected the transfer")]
    Rejected,
    #[error("the receiver is busy with another session")]
    Busy,
    #[error("the receiver found a checksum mismatch on {0}")]
    ChecksumMismatch(PathBuf),
    #[error("unexpected HTTP status {0}")]
    Status(u16),
    #[error("the receiver stopped answering")]
    Stalled,
    #[error("not a file: {0}")]
    NotAFile(PathBuf),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Http(reqwest::Error),
}

impl From<reqwest::Error> for SendError {
    fn from(e: reqwest::Error) -> Self {
        if crate::tls::is_fingerprint_mismatch(&e) { Self::FingerprintMismatch } else { Self::Http(e) }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SendReport {
    /// None when the receiver needed no transfer (204).
    pub session_id: Option<String>,
    pub sent: Vec<PathBuf>,
    /// Files the receiver declined.
    pub skipped: Vec<PathBuf>,
}

/// Announces `me` to the target and returns its info.
pub async fn register(me: &DeviceInfo, target: &Target) -> Result<DeviceInfo, SendError> {
    let client = crate::tls::https_client(target.fingerprint.as_deref())?;
    let response = client.post(target.url("register")).json(me).send().await?;
    if !response.status().is_success() {
        return Err(SendError::Status(response.status().as_u16()));
    }
    Ok(response.json().await?)
}

/// Sends `files` to `target` as `me`. On the first failed upload the session is cancelled.
pub async fn send_files(
    me: &DeviceInfo,
    target: &Target,
    files: &[PathBuf],
    pin: Option<&str>,
) -> Result<SendReport, SendError> {
    let client = crate::tls::https_client(target.fingerprint.as_deref())?;
    let mut by_id = HashMap::new();
    for (i, path) in files.iter().enumerate() {
        by_id.insert(format!("{i}-{}", crate::random_id()), (path.clone(), describe(path).await?));
    }
    let request = PrepareUploadRequest {
        info: me.clone(),
        files: by_id.iter().map(|(id, (_, meta))| (id.clone(), FileMeta { id: id.clone(), ..meta.clone() })).collect(),
    };
    let mut prepare = client.post(target.url("prepare-upload")).json(&request);
    if let Some(pin) = pin {
        prepare = prepare.query(&[("pin", pin)]);
    }
    let response = prepare.send().await?;
    let accepted: PrepareUploadResponse = match response.status() {
        StatusCode::OK => response.json().await?,
        StatusCode::NO_CONTENT => {
            return Ok(SendReport { skipped: files.to_vec(), ..Default::default() });
        }
        StatusCode::UNAUTHORIZED => return Err(SendError::PinRequired),
        StatusCode::FORBIDDEN => return Err(SendError::Rejected),
        StatusCode::CONFLICT => return Err(SendError::Busy),
        other => return Err(SendError::Status(other.as_u16())),
    };

    let session = accepted.session_id.as_str();
    // Owned per upload and moved into each future: borrowing closures keep the whole future from being `Send`.
    let uploads: Vec<(String, String, PathBuf, u64)> = accepted
        .files
        .iter()
        .filter_map(|(id, token)| {
            by_id.get(id).map(|(path, meta)| (id.clone(), token.clone(), path.clone(), meta.size))
        })
        .collect();
    let sent: Result<Vec<PathBuf>, SendError> = futures_util::stream::iter(uploads)
        .map(|(id, token, path, size)| {
            let request = client
                .post(target.url("upload"))
                .query(&[("sessionId", session), ("fileId", &id), ("token", &token)])
                .header(CONTENT_LENGTH, size);
            upload(request, path, STALL)
        })
        .buffer_unordered(PARALLEL_UPLOADS)
        .try_collect()
        .await;
    let sent = match sent {
        Ok(sent) => sent,
        Err(e) => {
            let _ = client.post(target.url("cancel")).query(&[("sessionId", session)]).send().await;
            return Err(e);
        }
    };
    let skipped = files.iter().filter(|p| !sent.contains(p)).cloned().collect();
    Ok(SendReport { session_id: Some(accepted.session_id), sent, skipped })
}

/// Streams one file; each chunk taken by the connection puts the `stall` deadline off.
async fn upload(request: reqwest::RequestBuilder, path: PathBuf, stall: Duration) -> Result<PathBuf, SendError> {
    let file = tokio::fs::File::open(&path).await?;
    let moved = Arc::new(Mutex::new(tokio::time::Instant::now()));
    let touched = moved.clone();
    let body = ReaderStream::with_capacity(file, READ_CHUNK).inspect(move |_| {
        *touched.lock().unwrap_or_else(|e| e.into_inner()) = tokio::time::Instant::now();
    });
    let send = request.body(reqwest::Body::wrap_stream(body)).send();
    tokio::pin!(send);
    let response = loop {
        let deadline = *moved.lock().unwrap_or_else(|e| e.into_inner()) + stall;
        tokio::select! {
            response = &mut send => break response?,
            _ = tokio::time::sleep_until(deadline) => {
                if moved.lock().unwrap_or_else(|e| e.into_inner()).elapsed() >= stall {
                    return Err(SendError::Stalled);
                }
            }
        }
    };
    match response.status() {
        s if s.is_success() => Ok(path),
        StatusCode::UNPROCESSABLE_ENTITY => Err(SendError::ChecksumMismatch(path)),
        s => Err(SendError::Status(s.as_u16())),
    }
}

/// The metadata of one file, its SHA-256 streamed from disk off the async threads.
async fn describe(path: &Path) -> Result<FileMeta, SendError> {
    let owned = path.to_path_buf();
    let (size, sha256) = tokio::task::spawn_blocking(move || sha256_file(&owned)).await.map_err(io::Error::other)??;
    let file_name = path.file_name().ok_or_else(|| SendError::NotAFile(path.into()))?.to_string_lossy().into_owned();
    Ok(FileMeta {
        id: String::new(),
        file_type: mime_type(&file_name).into(),
        file_name,
        size,
        sha256: Some(sha256),
        preview: None,
        metadata: None,
    })
}

fn sha256_file(path: &Path) -> io::Result<(u64, String)> {
    let mut file = std::fs::File::open(path)?;
    let (mut hasher, mut size, mut buf) = (Sha256::new(), 0u64, vec![0u8; READ_CHUNK]);
    loop {
        match file.read(&mut buf)? {
            0 => return Ok((size, crate::identity::hex(&hasher.finalize()))),
            n => {
                hasher.update(&buf[..n]);
                size += n as u64;
            }
        }
    }
}

/// Receivers use the type to pick an icon or a gallery; anything unlisted is plain bytes.
fn mime_type(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "tif" | "tiff" => "image/tiff",
        "heic" => "image/heic",
        "dng" => "image/x-adobe-dng",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// A plain HTTP peer that reads the request head, then drains the body and answers, or stops reading.
    async fn peer(drains: bool) -> SocketAddr {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut tcp, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 1 << 16];
            let n = tcp.read(&mut buf).await.unwrap_or(0);
            if !drains {
                return tokio::time::sleep(Duration::from_secs(3600)).await;
            }
            let head = buf[..n].windows(4).position(|w| w == b"\r\n\r\n").map_or(n, |at| at + 4);
            let mut body = n - head;
            while body < BODY {
                match tcp.read(&mut buf).await {
                    Ok(0) | Err(_) => return,
                    Ok(n) => body += n,
                }
            }
            let _ = tcp.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n").await;
            tokio::time::sleep(Duration::from_secs(3600)).await;
        });
        addr
    }

    /// More than every socket buffer between the two ends holds.
    const BODY: usize = 64 << 20;

    async fn sent_to(drains: bool) -> Result<PathBuf, SendError> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scan.dng");
        std::fs::write(&path, vec![7u8; BODY]).unwrap();
        let addr = peer(drains).await;
        let request = crate::tls::https_client(None).unwrap().post(format!("http://{addr}/upload")).header(CONTENT_LENGTH, BODY);
        tokio::time::timeout(Duration::from_secs(60), upload(request, path, Duration::from_millis(500))).await.unwrap()
    }

    #[tokio::test]
    async fn an_upload_to_a_peer_that_stops_reading_ends_as_stalled() {
        assert!(matches!(sent_to(false).await, Err(SendError::Stalled)));
        // Twin: a peer that drains the body and answers takes the file.
        assert!(sent_to(true).await.is_ok());
    }
}
