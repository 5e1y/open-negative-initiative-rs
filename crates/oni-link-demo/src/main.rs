//! `host` stands in for the desktop, `phone` for ONI Mobile, `bench` times a press over loopback.

use std::net::{SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use futures_util::future::BoxFuture;
use oni_link::{
    Channel, Command, Effect, Host, HostEvent, Identity, Letter, Machine, PairingCode, PublicKey, Remote, State,
};
use oni_localsend::{
    AcceptPolicy, Decision, DeviceInfo, Discovery, DiscoveryConfig, Event as LsEvent, FileMeta, Identity as LsIdentity,
    Peers, Receiver, ReceiverConfig,
};
use qrcode::QrCode;
use qrcode::render::unicode::Dense1x2;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("host") => {
            host(args.get(1).and_then(|p| p.parse().ok()).unwrap_or(53318), args.get(2).map(PathBuf::from)).await
        }
        Some("phone") if args.len() > 1 => phone(&args[1]).await,
        Some("bench") => bench(args.get(1).and_then(|n| n.parse().ok()).unwrap_or(1000)).await,
        _ => eprintln!("usage: oni-link-demo host [port] [inbox] | phone <oni-link://pair?...> | bench [presses]"),
    }
}

fn folder() -> PathBuf {
    let dir = PathBuf::from(".oni-link-demo");
    std::fs::create_dir_all(&dir).expect("demo folder");
    dir
}

fn identity(name: &str) -> Identity {
    let path = folder().join(name);
    if let Ok(bytes) = std::fs::read(&path) {
        return Identity::from_bytes(&bytes).expect("identity file");
    }
    let me = Identity::generate();
    std::fs::write(&path, me.to_bytes()).expect("identity written");
    me
}

/// The address other machines reach this one on; connecting a UDP socket sends nothing.
fn lan_address() -> Option<std::net::IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    Some(socket.local_addr().ok()?.ip())
}

/// A paired phone's files land at once; anyone else's wait for a yes typed here.
struct DesktopPolicy(Host);

impl AcceptPolicy for DesktopPolicy {
    fn decide<'a>(
        &'a self,
        sender: &'a DeviceInfo,
        files: &'a [FileMeta],
        pin: Option<&'a str>,
    ) -> BoxFuture<'a, Decision> {
        Box::pin(async move {
            if pin.is_some_and(|pin| self.0.accepts_import_pin(pin)) {
                return Decision::AcceptAll;
            }
            println!("Accept {} file(s) from {}? [y/N]", files.len(), sender.alias);
            let line = tokio::task::spawn_blocking(|| {
                let mut line = String::new();
                std::io::stdin().read_line(&mut line).map(|_| line)
            });
            match line.await {
                Ok(Ok(answer)) if answer.trim().eq_ignore_ascii_case("y") => Decision::AcceptAll,
                _ => Decision::Reject,
            }
        })
    }
}

/// Receives into `inbox` as a LocalSend target, announced to phones through the pairing.
async fn receive(host: &Host, inbox: PathBuf) -> (Receiver, Discovery) {
    let identity = LsIdentity::load_or_create(&folder()).expect("LocalSend identity");
    let peers = Peers::new(identity.fingerprint());
    let mut config = ReceiverConfig::new("ONI demo", identity, &inbox);
    config.policy = Arc::new(DesktopPolicy(host.clone()));
    config.peers = Some(peers.clone());
    let (receiver, mut events) = Receiver::start(config).await.expect("LocalSend port free");
    host.offer_import(receiver.info().fingerprint.clone(), receiver.local_addr().port());
    let discovery = Discovery::start(DiscoveryConfig::new(receiver.info().clone()), peers).await.expect("multicast");
    println!("Receiving photos into {} on port {}.", inbox.display(), receiver.local_addr().port());
    tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            if let LsEvent::FileDone { path, .. } = event {
                println!("received {}", path.display());
            }
        }
    });
    (receiver, discovery)
}

async fn host(port: u16, inbox: Option<PathBuf>) {
    let trusted_path = folder().join("trusted.txt");
    let trusted: Vec<PublicKey> = std::fs::read_to_string(&trusted_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| PublicKey::from_base64(line.trim()).ok())
        .collect();
    let (host, mut events) = Host::new(identity("host.key"), "ONI demo".into(), trusted);
    let bound = host.listen(SocketAddr::from(([0, 0, 0, 0], port))).await.expect("port free");
    let _import = match inbox {
        Some(inbox) => Some(receive(&host, inbox).await),
        None => None,
    };
    let addresses: Vec<SocketAddr> = lan_address().map(|ip| SocketAddr::new(ip, bound.port())).into_iter().collect();
    let code = host.open_pairing(addresses);
    let uri = code.to_uri();
    let qr = QrCode::new(uri.as_bytes()).expect("fits a QR code");
    println!("{}", qr.render::<Dense1x2>().quiet_zone(true).build());
    println!("{uri}\nPairing open for two minutes. Listening on {bound}.");

    let mut state = State { frame: Some((1, 36)), chrome: Some(0.0), ..State::default() };
    let (mut undo, mut redo): (Vec<State>, Vec<State>) = (vec![], vec![]);
    host.publish(state.clone());
    while let Some(event) = events.next().await {
        match event {
            HostEvent::Paired { phone, name } => {
                let mut keys = std::fs::read_to_string(&trusted_path).unwrap_or_default();
                keys.push_str(&format!("{}\n", phone.to_base64()));
                std::fs::write(&trusted_path, keys).expect("trusted written");
                println!("paired with {name}");
            }
            HostEvent::Connected { name, .. } => println!("{name} connected"),
            HostEvent::Disconnected { .. } => println!("phone disconnected"),
            HostEvent::Command { command, .. } => {
                let next = match command {
                    Command::Undo => undo.pop().inspect(|_| redo.push(state.clone())),
                    Command::Redo => redo.pop().inspect(|_| undo.push(state.clone())),
                    Command::Walk { by } => state.frame.map(|(at, count)| {
                        let at = (i32::from(at) + i32::from(by)).clamp(1, i32::from(count)) as u16;
                        State { frame: Some((at, count)), ..state.clone() }
                    }),
                    other => {
                        undo.push(state.clone());
                        redo.clear();
                        Some(state.predicted(other))
                    }
                };
                // History holds edits; the open frame is navigation and stays put.
                if let Some(next) = next {
                    state = State {
                        frame: next.frame.filter(|_| matches!(command, Command::Walk { .. })).or(state.frame),
                        ..next
                    };
                }
                state.revision += 1;
                state.can_undo = !undo.is_empty();
                state.can_redo = !redo.is_empty();
                host.publish(state.clone());
                println!("{}", screen(&state));
            }
        }
    }
}

async fn phone(uri: &str) {
    let me = identity("phone.key");
    let code = PairingCode::parse(uri).expect("an oni-link pairing URI");
    let stream = Remote::dial(&code.addresses).await.expect("desktop reachable");
    let desktop = stream.peer_addr().expect("connected").ip();
    let remote = Remote::pair(stream, &me, &code, "Terminal phone").await.expect("paired");
    println!(
        "paired with {}. Keys: d+1 c-3 m0 temp+1 ct+1 vig-1 chr+3 next prev undo redo oni frontier, send <file>",
        remote.host_name()
    );
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(path) = line.trim().strip_prefix("send ") {
            send(&remote, desktop, PathBuf::from(path)).await;
            continue;
        }
        match parse_key(line.trim()) {
            Some(command) => println!("{}", screen(&remote.press(command))),
            None => println!("unknown key"),
        }
    }
}

/// Sends a file the way ONI Mobile will: to the paired desktop's receiver, pinned, with this phone's pin.
async fn send(remote: &Remote, desktop: std::net::IpAddr, path: PathBuf) {
    let Some(import) = remote.import() else {
        println!("this desktop does not receive files");
        return;
    };
    let identity = LsIdentity::load_or_create(&folder().join("phone-localsend")).expect("LocalSend identity");
    let me = DeviceInfo {
        alias: "Terminal phone".into(),
        version: oni_localsend::PROTOCOL_VERSION.into(),
        device_model: None,
        device_type: Some(oni_localsend::DeviceType::Mobile),
        fingerprint: identity.fingerprint().into(),
        port: oni_localsend::DEFAULT_PORT,
        protocol: oni_localsend::Protocol::Https,
        download: false,
    };
    let target = oni_localsend::Target {
        addr: SocketAddr::new(desktop, import.port),
        protocol: oni_localsend::Protocol::Https,
        fingerprint: Some(import.fingerprint.clone()),
    };
    match oni_localsend::send_files(&me, &target, &[path], Some(&import.pin)).await {
        Ok(report) => println!("sent {} file(s)", report.sent.len()),
        Err(error) => println!("not sent: {error}"),
    }
}

fn parse_key(text: &str) -> Option<Command> {
    match text {
        "next" => return Some(Command::Walk { by: 1 }),
        "prev" => return Some(Command::Walk { by: -1 }),
        "undo" => return Some(Command::Undo),
        "redo" => return Some(Command::Redo),
        "oni" => return Some(Command::Machine(Machine::Oni)),
        "frontier" => return Some(Command::Machine(Machine::Frontier)),
        _ => {}
    }
    let split = text.find(['+', '-', '0'])?;
    let (name, amount) = text.split_at(split);
    let by: i8 = amount.parse().ok()?;
    let letter = match name {
        "d" => Some(Letter::D),
        "c" => Some(Letter::C),
        "m" => Some(Letter::M),
        "y" => Some(Letter::Y),
        "temp" => Some(Letter::W),
        _ => None,
    };
    let effect = match name {
        "ct" => Some(Effect::Contrast),
        "vig" => Some(Effect::Vignette),
        "chr" => Some(Effect::Chrome),
        _ => None,
    };
    match (letter, effect, by) {
        (Some(letter), _, 0) => Some(Command::Reset { letter }),
        (Some(letter), _, by) => Some(Command::Step { letter, by }),
        (_, Some(effect), 0) => Some(Command::ResetEffect { effect }),
        (_, Some(effect), by) => Some(Command::StepEffect { effect, by }),
        _ => None,
    }
}

/// The scanner's screen as the app's `LabScreen` prints it.
fn screen(state: &State) -> String {
    let spelled = |n: i8| oni_link::scanner::spelled(n.into());
    let percent = |v: f32| {
        let shown = (v * 100.0).round() as i32;
        if shown == 0 { "0 %".into() } else { format!("{}{} %", if shown < 0 { "−" } else { "+" }, shown.abs()) }
    };
    let frame = state.frame.map_or("FRAME --/--".into(), |(at, count)| format!("FRAME {at:02}/{count:02}"));
    let machine = if state.machine == Machine::Oni { "ONI" } else { "FRONTIER" };
    let channel = match state.channel {
        Channel::Negative => "NEGA",
        Channel::Positive => "POSI",
        Channel::Monochrome => "B&W",
    };
    let counts: Vec<String> =
        Letter::ALL.iter().map(|l| format!("{} {}", l.title(), spelled(state.clicks.get(*l)))).collect();
    let vignette = if state.vignette.abs() < 0.005 {
        "0 EV".into()
    } else {
        format!("{}{:.1} EV", if state.vignette < 0.0 { "−" } else { "+" }, state.vignette.abs())
    };
    let chrome = state.chrome.map(|c| format!("  CHR {}", percent(c))).unwrap_or_default();
    format!(
        "{frame}  {machine} · {channel}\n{}\nCT {}  VIG {vignette}{chrome}",
        counts.join("  "),
        percent(state.contrast)
    )
}

/// Press after press over loopback TCP, each timed from the call to the desktop's screen coming back.
async fn bench(presses: usize) {
    let (host, mut events) = Host::new(Identity::generate(), "Bench".into(), []);
    let addr = host.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
    let app = host.clone();
    tokio::spawn(async move {
        let mut state = State::default();
        while let Some(event) = events.next().await {
            if let HostEvent::Command { command, .. } = event {
                state = state.predicted(command);
                state.revision += 1;
                app.publish(state.clone());
            }
        }
    });
    let code = host.open_pairing(vec![addr]);
    let remote =
        Remote::pair(Remote::dial(&[addr]).await.unwrap(), &Identity::generate(), &code, "Bench").await.unwrap();
    let mut display = remote.display();
    let mut times = Vec::with_capacity(presses);
    for i in 0..presses {
        let by = if i % 2 == 0 { 1 } else { -1 };
        let start = Instant::now();
        remote.press(Command::Step { letter: Letter::D, by });
        let seq = i as u64 + 1;
        while display.borrow_and_update().state.applied < seq {
            display.changed().await.unwrap();
        }
        times.push(start.elapsed());
    }
    times.sort();
    let at = |q: f64| times[((times.len() as f64 - 1.0) * q).round() as usize];
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    println!(
        "{presses} presses over loopback: p50 {:.3} ms, p99 {:.3} ms, max {:.3} ms",
        ms(at(0.5)),
        ms(at(0.99)),
        ms(*times.last().unwrap())
    );
}
