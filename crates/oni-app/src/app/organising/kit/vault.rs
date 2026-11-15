//! Where the token lies: the macOS Keychain, the Windows Credential Manager, never a file of the support
//! folder; the simulated service's token, worth nothing, lies in that folder so a check never meets a keychain.

use std::path::Path;

const SERVICE: &str = "Open Negative Initiative — The Lens DB";
const ACCOUNT: &str = "kit";
const FAKE_FILE: &str = "tldb-fake-token";

pub fn read(support: &Path, fake: bool) -> Option<String> {
    if fake {
        return std::fs::read_to_string(support.join(FAKE_FILE)).ok().filter(|t| !t.is_empty());
    }
    system::read()
}

pub fn write(support: &Path, fake: bool, token: &str) -> Result<(), String> {
    if fake {
        return std::fs::create_dir_all(support).and_then(|_| std::fs::write(support.join(FAKE_FILE), token)).map_err(|e| format!("{:?}", e.kind()));
    }
    system::write(token)
}

pub fn erase(support: &Path, fake: bool) {
    if fake {
        let _ = std::fs::remove_file(support.join(FAKE_FILE));
    } else {
        system::erase();
    }
}

#[cfg(target_os = "macos")]
mod system {
    use security_framework::passwords;

    pub fn read() -> Option<String> {
        passwords::get_generic_password(super::SERVICE, super::ACCOUNT).ok().and_then(|b| String::from_utf8(b).ok())
    }

    pub fn write(token: &str) -> Result<(), String> {
        passwords::set_generic_password(super::SERVICE, super::ACCOUNT, token.as_bytes()).map_err(|e| e.code().to_string())
    }

    pub fn erase() {
        let _ = passwords::delete_generic_password(super::SERVICE, super::ACCOUNT);
    }
}

#[cfg(windows)]
mod system {
    use windows_sys::Win32::Security::Credentials::{CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree, CredReadW, CredWriteW};

    fn target() -> Vec<u16> {
        format!("{}/{}", super::SERVICE, super::ACCOUNT).encode_utf16().chain([0]).collect()
    }

    pub fn read() -> Option<String> {
        let name = target();
        let mut found: *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY: `found` is filled by CredReadW and freed by CredFree once its blob is copied.
        unsafe {
            if CredReadW(name.as_ptr(), CRED_TYPE_GENERIC, 0, &mut found) == 0 {
                return None;
            }
            let c = &*found;
            let blob = std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize).to_vec();
            CredFree(found as *const _);
            String::from_utf8(blob).ok()
        }
    }

    pub fn write(token: &str) -> Result<(), String> {
        let mut name = target();
        let mut user: Vec<u16> = super::ACCOUNT.encode_utf16().chain([0]).collect();
        let mut blob = token.as_bytes().to_vec();
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: name.as_mut_ptr(),
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: user.as_mut_ptr(),
            // SAFETY: every other field of CREDENTIALW is a number or a pointer for which zero means none.
            ..unsafe { std::mem::zeroed() }
        };
        // SAFETY: the credential's pointers outlive the call, which copies what they point at.
        if unsafe { CredWriteW(&credential, 0) } == 0 { Err(std::io::Error::last_os_error().to_string()) } else { Ok(()) }
    }

    pub fn erase() {
        let name = target();
        // SAFETY: a nul-terminated name; a credential already gone answers false, which changes nothing.
        unsafe { CredDeleteW(name.as_ptr(), CRED_TYPE_GENERIC, 0) };
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod system {
    pub fn read() -> Option<String> {
        None
    }

    pub fn write(_token: &str) -> Result<(), String> {
        Err("no keychain on this system".into())
    }

    pub fn erase() {}
}
