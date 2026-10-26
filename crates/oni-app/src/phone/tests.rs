use super::addresses::*;
use super::*;
use std::net::{IpAddr, Ipv4Addr};

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
fn the_link_listens_on_the_port_the_last_code_named() {
    let dir = std::env::temp_dir().join(format!("oni-app-phone-port-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(folder(&dir)).unwrap();
    let free = std::net::TcpListener::bind("0.0.0.0:0").unwrap().local_addr().unwrap().port();
    std::fs::write(port_path(&dir), format!("{free}\n")).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let listening = move |e| {
        if let Event::Listening(at) = e {
            let _ = tx.send(at);
        }
    };
    let mut phone = Phone::start(&dir, listening).unwrap();
    let at = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
    assert_eq!(at.port(), free);
    // A code that names an address keeps its port for the next launch.
    std::fs::remove_file(port_path(&dir)).unwrap();
    phone.listening(at);
    let code = phone.open_pairing().unwrap();
    assert_eq!(given_port(&dir), (!code.addresses.is_empty()).then_some(free));
    drop(phone);
    // Twin: a file unreadable as a port names none, so the fixed port is wanted again.
    std::fs::write(port_path(&dir), "53318x").unwrap();
    assert_eq!(given_port(&dir), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_key_and_the_trusted_phones_are_written_whole_and_owner_only() {
    let dir = std::env::temp_dir().join(format!("oni-app-phone-private-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let first = Identity::generate().public();
    trust(&dir, first).unwrap();
    identity(&dir).unwrap();
    for path in [trusted_path(&dir), folder(&dir).join("host.key")] {
        assert!(!sibling(&path).exists(), "no sibling stays behind {}", path.display());
        #[cfg(unix)]
        assert_eq!(mode(&path), 0o600, "{}", path.display());
    }
    // A write that cannot finish leaves the list as it was, never half of it.
    std::fs::create_dir_all(sibling(&trusted_path(&dir)).join("blocked")).unwrap();
    assert!(trust(&dir, Identity::generate().public()).is_err());
    assert_eq!(trusted(&dir), vec![first]);
    std::fs::remove_dir_all(sibling(&trusted_path(&dir))).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let key = folder(&dir).join("host.key");
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o644)).unwrap();
        identity(&dir).unwrap();
        assert_eq!(mode(&key), 0o600, "a key left readable by others is closed on the next start");
        // Twin: a plain write, as the key was written before, is readable by others.
        let plain = folder(&dir).join("plain");
        std::fs::write(&plain, b"key").unwrap();
        assert_ne!(mode(&plain), 0o600);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
fn mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn an_old_link_ending_after_the_reconnection_leaves_the_phone_connected() {
    let (phone, other) = (Identity::generate().public(), Identity::generate().public());
    let mut on = Connections::default();
    assert!(on.connected(phone, 1, "Phone".into()));
    assert!(!on.connected(phone, 2, "Phone".into()), "back on a new link, it is not new");
    assert!(on.connected(other, 3, "Tablet".into()));
    assert!(!on.disconnected(phone, 1), "the old link's end leaves the new one");
    assert_eq!(on.names(), ["Phone", "Tablet"]);
    // Twin: the new link's end is the phone gone.
    assert!(on.disconnected(phone, 2));
    assert_eq!(on.names(), ["Tablet"]);
}

#[test]
fn the_code_names_every_interface_after_the_default_route() {
    let lan = |d| Ipv4Addr::new(192, 168, 1, d);
    let vpn = IpAddr::V4(Ipv4Addr::new(10, 8, 0, 2));
    let interfaces = vec![Ipv4Addr::LOCALHOST, Ipv4Addr::new(10, 8, 0, 2), lan(20), Ipv4Addr::new(169, 254, 3, 4), lan(20)];
    assert_eq!(reachable(Some(vpn), interfaces), vec![vpn, IpAddr::V4(lan(20))], "the Wi-Fi after the tunnel");
    let many = (1..=9).map(lan).collect();
    assert_eq!(reachable(None, many).len(), ADDRESSES);
    // Twin: the default route alone misses the phone's network.
    assert!(!reachable(Some(vpn), Vec::new()).contains(&IpAddr::V4(lan(20))));
}

#[test]
fn this_computer_lists_its_default_route_first() {
    let all = lan_addresses();
    assert_eq!(all.first().copied(), lan_address());
    assert!(all.iter().all(|ip| !ip.is_loopback()), "{all:?}");
    // Twin: the loopback interface is up, and listed by the system all the same.
    #[cfg(target_os = "macos")]
    assert!(interface_addresses().contains(&Ipv4Addr::LOCALHOST));
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
