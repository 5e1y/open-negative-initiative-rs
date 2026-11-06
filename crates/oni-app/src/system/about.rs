//! What the log says of the system the app runs on: the macOS version, and Rosetta.

/// The system and the binary as the log names them: the macOS version, and whether an Intel build
/// runs translated on Apple silicon.
#[cfg(target_os = "macos")]
pub fn described() -> String {
    let version = sysctl("kern.osproductversion").map(|v| String::from_utf8_lossy(&v).trim_end_matches('\0').to_string());
    let translated = sysctl("sysctl.proc_translated").and_then(|v| v.get(..4).map(|b| i32::from_ne_bytes([b[0], b[1], b[2], b[3]]))) == Some(1);
    format!("macOS {}, {}{}", version.as_deref().unwrap_or("unknown"), std::env::consts::ARCH, if translated { " under Rosetta" } else { "" })
}

#[cfg(not(target_os = "macos"))]
pub fn described() -> String {
    format!("{}, {}", std::env::consts::OS, std::env::consts::ARCH)
}

#[cfg(target_os = "macos")]
fn sysctl(name: &str) -> Option<Vec<u8>> {
    let name = std::ffi::CString::new(name).ok()?;
    let mut size = 0usize;
    // SAFETY: the size is asked first, then the value read into a buffer of that size.
    unsafe {
        if libc::sysctlbyname(name.as_ptr(), std::ptr::null_mut(), &mut size, std::ptr::null_mut(), 0) != 0 {
            return None;
        }
        let mut value = vec![0u8; size];
        if libc::sysctlbyname(name.as_ptr(), value.as_mut_ptr().cast(), &mut size, std::ptr::null_mut(), 0) != 0 {
            return None;
        }
        value.truncate(size);
        Some(value)
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    #[test]
    fn the_log_names_the_macos_version_and_the_architecture() {
        let said = super::described();
        assert!(said.starts_with("macOS ") && said.contains(std::env::consts::ARCH), "{said}");
        // Twin: a version the system did not answer would read unknown.
        assert!(!said.contains("unknown"), "{said}");
        assert_eq!(super::sysctl("no.such.name"), None);
    }
}
