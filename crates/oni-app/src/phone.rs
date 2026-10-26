//! The phone remote: `oni-link`'s Host on a thread of its own, the desktop's key and the phones it
//! trusts in the support folder, and the pairing QR code the sheet shows.

use crate::link::Link;
use oni_link::{Host, HostEvent, Identity, PairingCode, PublicKey, Receipt};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use tokio::sync::oneshot;

mod addresses;

use addresses::lan_addresses;

/// A fixed port, so a paired phone finds the desktop at the same address after a restart; when it
/// is taken, any free port, which only a new pairing then tells the phone, and later launches try first.
pub const PORT: u16 = 53318;
/// QR code pixels per module, and the four-module margin readers need around it.
const MODULE: usize = 8;
const QUIET: usize = 4;

/// What the link thread tells the interface thread; a press is marked taken once applied there.
pub enum Event {
    Listening(SocketAddr),
    Failed(String),
    Host(HostEvent, Receipt),
}

pub struct Phone {
    host: Host,
    pub link: Link,
    support: PathBuf,
    bound: Option<SocketAddr>,
    connected: Connections,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

fn folder(support: &Path) -> PathBuf {
    support.join("Link")
}

fn trusted_path(support: &Path) -> PathBuf {
    folder(support).join("trusted.txt")
}

fn port_path(support: &Path) -> PathBuf {
    folder(support).join("port.txt")
}

/// The port the last pairing code named, which the paired phones dial.
fn given_port(support: &Path) -> Option<u16> {
    std::fs::read_to_string(port_path(support)).ok()?.trim().parse().ok()
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
    write_private(&trusted_path(support), text.as_bytes()).map_err(|e| e.to_string())
}

/// No phone trusted any more; a missing list is already that.
pub fn forget(support: &Path) -> Result<(), String> {
    match std::fs::remove_file(trusted_path(support)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
        _ => Ok(()),
    }
}

/// The desktop's key pair, made once and kept, since every paired phone holds its public half.
fn identity(support: &Path) -> Result<Identity, String> {
    let path = folder(support).join("host.key");
    if let Ok(bytes) = std::fs::read(&path) {
        owner_only(&path);
        return Identity::from_bytes(&bytes).map_err(|e| format!("{}: {e}", path.display()));
    }
    let me = Identity::generate();
    write_private(&path, &me.to_bytes()).map_err(|e| e.to_string())?;
    Ok(me)
}

fn sibling(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".tmp");
    PathBuf::from(name)
}

/// Written through a sibling then renamed, so a crash never leaves half a file; owner-only on Unix.
/// On Windows the support folder sits in the user's profile, whose ACL already keeps others out.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temporary = sibling(path);
    let _ = std::fs::remove_file(&temporary);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temporary, path)
}

/// A key written readable by others before is closed to them; elsewhere the profile's ACL holds.
fn owner_only(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o077 != 0) {
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
    }
    #[cfg(not(unix))]
    let _ = path;
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

/// The phones connected now, one entry a connection: a phone back before its old link timed out
/// holds two until the old one ends.
#[derive(Default)]
struct Connections(Vec<(PublicKey, u64, String)>);

impl Connections {
    /// True when the phone was not connected already.
    fn connected(&mut self, phone: PublicKey, connection: u64, name: String) -> bool {
        let new = !self.0.iter().any(|(k, _, _)| *k == phone);
        self.0.push((phone, connection, name));
        new
    }

    /// True when the phone has no connection left.
    fn disconnected(&mut self, phone: PublicKey, connection: u64) -> bool {
        self.0.retain(|(_, c, _)| *c != connection);
        !self.0.iter().any(|(k, _, _)| *k == phone)
    }

    /// Each phone's latest name, once.
    fn names(&self) -> Vec<String> {
        let mut names: Vec<(PublicKey, String)> = Vec::new();
        for (phone, _, name) in &self.0 {
            names.retain(|(k, _)| k != phone);
            names.push((*phone, name.clone()));
        }
        names.into_iter().map(|(_, n)| n).collect()
    }
}

impl Phone {
    /// The Host listening on every interface, on its own thread; `notify` runs there, once per event.
    pub fn start(support: &Path, notify: impl Fn(Event) + Send + 'static) -> Result<Phone, String> {
        let identity = identity(support)?;
        let wanted = given_port(support).unwrap_or(PORT);
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
                    let bound = match listener.listen(any(wanted)).await {
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
                            event = events.receive() => match event {
                                Some((e, receipt)) => notify(Event::Host(e, receipt)),
                                None => break,
                            },
                            _ = &mut stopped => break,
                        }
                    }
                });
            })
            .map_err(|e| e.to_string())?;
        let support = support.to_path_buf();
        Ok(Phone { host: host.clone(), link: Link::new(host), support, bound: None, connected: Connections::default(), stop: Some(stop), thread: Some(thread) })
    }

    pub fn listening(&mut self, at: SocketAddr) {
        self.bound = Some(at);
    }

    pub fn bound(&self) -> Option<SocketAddr> {
        self.bound
    }

    /// A one-time code naming this computer's network address, or `None` before the Host listens;
    /// its port is kept, for the next launch to listen where the phone will dial.
    pub fn open_pairing(&self) -> Option<PairingCode> {
        let port = self.bound?.port();
        let addresses: Vec<SocketAddr> = lan_addresses().into_iter().map(|ip| SocketAddr::new(ip, port)).collect();
        if !addresses.is_empty() && given_port(&self.support) != Some(port) {
            let _ = write_private(&port_path(&self.support), format!("{port}\n").as_bytes());
        }
        Some(self.host.open_pairing(addresses))
    }

    pub fn close_pairing(&self) {
        self.host.close_pairing();
    }

    /// True when the phone was not connected already, on another link.
    pub fn connected(&mut self, phone: PublicKey, connection: u64, name: String) -> bool {
        self.connected.connected(phone, connection, name)
    }

    /// True when the phone is gone: its other link, if one, is still up.
    pub fn disconnected(&mut self, phone: PublicKey, connection: u64) -> bool {
        self.connected.disconnected(phone, connection)
    }

    /// The names of the phones connected now.
    pub fn names(&self) -> Vec<String> {
        self.connected.names()
    }

    /// True while the Host trusts the phone, so a pairing forgotten before it was heard is not kept.
    pub fn trusts(&self, phone: &PublicKey) -> bool {
        self.host.trusts(phone)
    }

    /// Every paired phone forgotten and cut at once, those trusted for this session only included;
    /// each pairs again from a new code.
    pub fn forget_all(&mut self) -> Result<(), String> {
        self.host.revoke_all();
        self.connected = Connections::default();
        forget(&self.support)
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
mod tests;
