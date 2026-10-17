#![allow(dead_code)]

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::time::Duration;

use oni_localsend::*;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc::UnboundedReceiver;

pub fn me(fingerprint: &str) -> DeviceInfo {
    DeviceInfo {
        alias: "Sender".into(),
        version: PROTOCOL_VERSION.into(),
        device_model: Some("Test".into()),
        device_type: Some(DeviceType::Mobile),
        fingerprint: fingerprint.into(),
        port: DEFAULT_PORT,
        protocol: Protocol::Https,
        download: false,
    }
}

pub async fn start(dest: &Path, tweak: impl FnOnce(&mut ReceiverConfig)) -> (Receiver, UnboundedReceiver<Event>) {
    let mut config = ReceiverConfig::new("ONI", Identity::generate().unwrap(), dest);
    config.bind = IpAddr::V4(Ipv4Addr::LOCALHOST);
    config.port = 0;
    tweak(&mut config);
    Receiver::start(config).await.unwrap()
}

pub fn target(receiver: &Receiver) -> Target {
    Target::new(receiver.local_addr())
}

/// Sorted names in a directory, hidden part files included.
pub fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> =
        std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

pub fn meta(id: &str, name: &str, size: u64, sha256: Option<String>) -> FileMeta {
    FileMeta {
        id: id.into(),
        file_name: name.into(),
        size,
        file_type: "application/octet-stream".into(),
        sha256,
        preview: None,
        metadata: None,
    }
}

/// Hand-built protocol calls, for what the crate's sender refuses to do.
pub struct Raw {
    pub client: reqwest::Client,
    pub addr: SocketAddr,
}

impl Raw {
    pub fn new(addr: SocketAddr) -> Self {
        Self { client: https_client(None).unwrap(), addr }
    }

    fn url(&self, path: &str) -> String {
        format!("https://{}/api/localsend/v2/{path}", self.addr)
    }

    pub async fn prepare(&self, files: Vec<FileMeta>, pin: Option<&str>) -> reqwest::Response {
        let request = PrepareUploadRequest {
            info: me("RAW"),
            files: files.into_iter().map(|f| (f.id.clone(), f)).collect::<HashMap<_, _>>(),
        };
        let mut url = self.url("prepare-upload");
        if let Some(pin) = pin {
            url.push_str(&format!("?pin={pin}"));
        }
        self.client.post(url).json(&request).send().await.unwrap()
    }

    pub async fn prepare_ok(&self, files: Vec<FileMeta>) -> PrepareUploadResponse {
        let response = self.prepare(files, None).await;
        assert_eq!(response.status(), 200);
        response.json().await.unwrap()
    }

    pub fn upload_url(&self, session: &PrepareUploadResponse, file_id: &str) -> String {
        format!(
            "{}?sessionId={}&fileId={file_id}&token={}",
            self.url("upload"),
            session.session_id,
            session.files[file_id]
        )
    }

    pub async fn upload(&self, session: &PrepareUploadResponse, file_id: &str, bytes: Vec<u8>) -> u16 {
        self.client.post(self.upload_url(session, file_id)).body(bytes).send().await.unwrap().status().as_u16()
    }

    pub async fn cancel(&self, session_id: &str) -> u16 {
        let url = format!("{}?sessionId={session_id}", self.url("cancel"));
        self.client.post(url).send().await.unwrap().status().as_u16()
    }
}

/// The next event matching `pick`, skipping the others; panics after five seconds.
pub async fn wait_for<T>(events: &mut UnboundedReceiver<Event>, mut pick: impl FnMut(&Event) -> Option<T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let event = events.recv().await.expect("receiver alive");
            if let Some(t) = pick(&event) {
                return t;
            }
        }
    })
    .await
    .expect("event within five seconds")
}

/// Polls until `cond` holds; panics after five seconds.
pub async fn eventually(mut cond: impl FnMut() -> bool) {
    for _ in 0..500 {
        if cond() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("condition not met within five seconds");
}
