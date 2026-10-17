//! The phone remote: `oni-link`'s Host on a thread of its own, the desktop's key and the phones it
//! trusts in the support folder, and the pairing QR code the sheet shows.

use oni_core::link::Link;
use oni_link::{Host, HostEvent, Identity, PairingCode, PublicKey};
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use tokio::sync::oneshot;

/// A fixed port, so a paired phone finds the desktop at the same address after a restart; when it
/// is taken, any free port, which only a new pairing then tells the phone.
pub const PORT: u16 = 53318;
/// QR code pixels per module, and the four-module margin readers need around it.
const MODULE: usize = 8;
const QUIET: usize = 4;

/// What the link thread tells the interface thread.
#[derive(Debug)]
pub enum Event {
    Listening(SocketAddr),
    Failed(String),
    Host(HostEvent),
}

pub struct Phone {
    host: Host,
    pub link: Link,
    bound: Option<SocketAddr>,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

fn folder(support: &Path) -> PathBuf {
    support.join("Link")
}

fn trusted_path(support: &Path) -> PathBuf {
    folder(support).join("trusted.txt")
}

/// The phones paired before, one base64 key per line.
pub fn trusted(support: &Path) -> Vec<PublicKey> {
    std::fs::read_to_string(trusted_path(support))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| PublicKey::from_base64(line.trim()).ok())
        .collect()
}

pub fn has_trusted(support: &Path) -> bool {
    !trusted(support).is_empty()
}

/// Remembers a phone that passed the QR code, so it reconnects without one.
pub fn trust(support: &Path, phone: PublicKey) -> Result<(), String> {
    let mut keys = trusted(support);
    if keys.contains(&phone) {
        return Ok(());
    }
    keys.push(phone);
    let text: String = keys.iter().map(|k| format!("{}\n", k.to_base64())).collect();
    std::fs::create_dir_all(folder(support)).map_err(|e| e.to_string())?;
    std::fs::write(trusted_path(support), text).map_err(|e| e.to_string())
}

/// The desktop's key pair, made once and kept, since every paired phone holds its public half.
fn identity(support: &Path) -> Result<Identity, String> {
    let path = folder(support).join("host.key");
    if let Ok(bytes) = std::fs::read(&path) {
        return Identity::from_bytes(&bytes).map_err(|e| format!("{}: {e}", path.display()));
    }
    let me = Identity::generate();
    std::fs::create_dir_all(folder(support)).map_err(|e| e.to_string())?;
    std::fs::write(&path, me.to_bytes()).map_err(|e| e.to_string())?;
    Ok(me)
}

/// The name the phone shows for this computer.
pub fn device_name() -> String {
    let asked = if cfg!(windows) {
        std::env::var("COMPUTERNAME").ok()
    } else {
        let (program, args): (&str, &[&str]) = if cfg!(target_os = "macos") { ("scutil", &["--get", "ComputerName"]) } else { ("hostname", &[]) };
        std::process::Command::new(program).args(args).output().ok().and_then(|o| String::from_utf8(o.stdout).ok())
    };
    asked.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| crate::timing::FOLDER.to_string())
}

/// The address other machines on the network reach this one on; connecting a UDP socket sends nothing.
pub fn lan_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_unspecified() && !ip.is_loopback()).then_some(ip)
}

impl Phone {
    /// The Host listening on every interface, on its own thread; `notify` runs there, once per event.
    pub fn start(support: &Path, notify: impl Fn(Event) + Send + 'static) -> Result<Phone, String> {
        let identity = identity(support)?;
        let (host, mut events) = Host::new(identity, device_name(), trusted(support));
        let (stop, mut stopped) = oneshot::channel::<()>();
        let listener = host.clone();
        let thread = std::thread::Builder::new()
            .name("phone link".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                    Ok(r) => r,
                    Err(e) => return notify(Event::Failed(e.to_string())),
                };
                runtime.block_on(async move {
                    let any = |port| SocketAddr::from(([0, 0, 0, 0], port));
                    let bound = match listener.listen(any(PORT)).await {
                        Ok(a) => Ok(a),
                        Err(_) => listener.listen(any(0)).await,
                    };
                    drop(listener);
                    match bound {
                        Ok(a) => notify(Event::Listening(a)),
                        Err(e) => return notify(Event::Failed(e.to_string())),
                    }
                    loop {
                        tokio::select! {
                            event = events.next() => match event {
                                Some(e) => notify(Event::Host(e)),
                                None => break,
                            },
                            _ = &mut stopped => break,
                        }
                    }
                });
            })
            .map_err(|e| e.to_string())?;
        Ok(Phone { host: host.clone(), link: Link::new(host), bound: None, stop: Some(stop), thread: Some(thread) })
    }

    pub fn listening(&mut self, at: SocketAddr) {
        self.bound = Some(at);
    }

    pub fn bound(&self) -> Option<SocketAddr> {
        self.bound
    }

    /// A one-time code naming this computer's network address, or `None` before the Host listens.
    pub fn open_pairing(&self) -> Option<PairingCode> {
        let port = self.bound?.port();
        let addresses = lan_address().map(|ip| SocketAddr::new(ip, port)).into_iter().collect();
        Some(self.host.open_pairing(addresses))
    }

    pub fn close_pairing(&self) {
        self.host.close_pairing();
    }
}

impl Drop for Phone {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// The URI as a QR code in opaque RGBA8, dark modules on white, and its side in pixels.
pub fn qr(uri: &str) -> Result<(Vec<u8>, usize), String> {
    let code = qrcode::QrCode::new(uri.as_bytes()).map_err(|e| e.to_string())?;
    let n = code.width();
    let colors = code.to_colors();
    let side = (n + 2 * QUIET) * MODULE;
    let mut out = vec![255u8; side * side * 4];
    for (i, c) in colors.iter().enumerate() {
        if *c != qrcode::Color::Dark {
            continue;
        }
        let (mx, my) = (i % n + QUIET, i / n + QUIET);
        for y in my * MODULE..(my + 1) * MODULE {
            for x in mx * MODULE..(mx + 1) * MODULE {
                out[(y * side + x) * 4..(y * side + x) * 4 + 3].fill(0);
            }
        }
    }
    Ok((out, side))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_qr_code_draws_its_finder_in_the_corner_inside_a_white_margin() {
        let (rgba, side) = qr("oni-link://pair?v=1").unwrap();
        assert_eq!(side % MODULE, 0);
        let at = |x: usize, y: usize| rgba[(y * side + x) * 4];
        let edge = QUIET * MODULE;
        assert_eq!(at(edge - 1, edge - 1), 255, "the margin is white");
        assert_eq!(at(edge, edge), 0, "the finder's corner is dark");
        assert_eq!(at(edge + 3 * MODULE, edge + 3 * MODULE), 0, "the finder's centre is dark");
        // Twin: the ring between the finder's border and its centre is light.
        assert_eq!(at(edge + MODULE + 1, edge + MODULE + 1), 255);
    }

    #[test]
    fn a_trusted_phone_is_written_once_and_read_back() {
        let dir = std::env::temp_dir().join(format!("oni-app-phone-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!has_trusted(&dir));
        let key = Identity::generate().public();
        trust(&dir, key).unwrap();
        trust(&dir, key).unwrap();
        assert_eq!(trusted(&dir), vec![key]);
        // Twin: the desktop's own key is kept between two starts, so it is never a new one.
        let a = identity(&dir).unwrap().public();
        assert_eq!(identity(&dir).unwrap().public(), a);
        assert_ne!(Identity::generate().public(), a);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
