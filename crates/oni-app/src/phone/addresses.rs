//! The addresses a pairing code names: every interface's a phone may dial, the default route's first.

use std::net::{IpAddr, Ipv4Addr, UdpSocket};

/// Addresses a code names at most, the phone dialling each in turn, two seconds a dead one.
pub(super) const ADDRESSES: usize = 4;

/// The address other machines on the network reach this one on; connecting a UDP socket sends nothing.
pub(super) fn lan_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_unspecified() && !ip.is_loopback()).then_some(ip)
}

/// Every address a phone may reach this computer on, the default route's first: under a VPN, or
/// with Ethernet and Wi-Fi on two networks, the phone's network is often another interface's.
pub(super) fn lan_addresses() -> Vec<IpAddr> {
    reachable(lan_address(), interface_addresses())
}

/// The default route's address, then each other one a phone could dial, once, at most `ADDRESSES`.
pub(super) fn reachable(default: Option<IpAddr>, interfaces: Vec<Ipv4Addr>) -> Vec<IpAddr> {
    let mut found: Vec<IpAddr> = default.into_iter().collect();
    for ip in interfaces {
        let ip = IpAddr::V4(ip);
        let dialable = !ip.is_unspecified() && !ip.is_loopback() && !matches!(ip, IpAddr::V4(v4) if v4.is_link_local());
        if dialable && !found.contains(&ip) {
            found.push(ip);
        }
    }
    found.truncate(ADDRESSES);
    found
}

/// The IPv4 addresses of the interfaces that are up.
#[cfg(target_os = "macos")]
pub(super) fn interface_addresses() -> Vec<Ipv4Addr> {
    let mut list: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: on success `list` heads a linked list the system allocated, freed below.
    if unsafe { libc::getifaddrs(&mut list) } != 0 {
        return Vec::new();
    }
    let mut found = Vec::new();
    let mut at = list;
    // SAFETY: each node and its address stay valid until `freeifaddrs`.
    unsafe {
        while let Some(entry) = at.as_ref() {
            let up = entry.ifa_flags & libc::IFF_UP as u32 != 0;
            if up && !entry.ifa_addr.is_null() && i32::from((*entry.ifa_addr).sa_family) == libc::AF_INET {
                let sin = &*entry.ifa_addr.cast::<libc::sockaddr_in>();
                found.push(Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr)));
            }
            at = entry.ifa_next;
        }
        libc::freeifaddrs(list);
    }
    found
}

/// The IPv4 addresses of the adapters that are up.
#[cfg(target_os = "windows")]
pub(super) fn interface_addresses() -> Vec<Ipv4Addr> {
    use windows_sys::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, NO_ERROR};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses,
        IP_ADAPTER_ADDRESSES_LH,
    };
    use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows_sys::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};
    let mut size: u32 = 16 * 1024;
    for _ in 0..3 {
        // u64 words keep the adapter records aligned.
        let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
        let first = buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
        // SAFETY: `first` points at `size` writable bytes, where the call writes its list.
        let status = unsafe { GetAdaptersAddresses(u32::from(AF_INET), flags, std::ptr::null(), first, &mut size) };
        if status == ERROR_BUFFER_OVERFLOW {
            continue;
        }
        if status != NO_ERROR {
            return Vec::new();
        }
        let mut found = Vec::new();
        let mut adapter = first.cast_const();
        // SAFETY: the adapters and their unicast lists live in `buffer`, linked by the call's pointers.
        unsafe {
            while let Some(a) = adapter.as_ref() {
                let mut unicast = if a.OperStatus == IfOperStatusUp { a.FirstUnicastAddress } else { std::ptr::null_mut() };
                while let Some(u) = unicast.as_ref() {
                    let address = u.Address.lpSockaddr;
                    if !address.is_null() && (*address).sa_family == AF_INET {
                        let sin = &*address.cast::<SOCKADDR_IN>();
                        found.push(Ipv4Addr::from(u32::from_be(sin.sin_addr.S_un.S_addr)));
                    }
                    unicast = u.Next;
                }
                adapter = a.Next;
            }
        }
        return found;
    }
    Vec::new()
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(super) fn interface_addresses() -> Vec<Ipv4Addr> {
    Vec::new()
}
