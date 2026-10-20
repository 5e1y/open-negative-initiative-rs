//! `ColorSyncIterateInstalledProfiles`: the profiles the system knows of beyond the folders walked,
//! as the Swift app adds them to its walk.

use std::path::PathBuf;

/// Every profile file ColorSync lists; empty off the Mac.
pub fn installed() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    return mac::installed();
    #[cfg(not(target_os = "macos"))]
    Vec::new()
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::c_void;
    use std::os::unix::ffi::OsStrExt;
    use std::path::PathBuf;

    type CFTypeRef = *const c_void;

    #[link(name = "ColorSync", kind = "framework")]
    unsafe extern "C" {
        static kColorSyncProfileURL: CFTypeRef;
        fn ColorSyncIterateInstalledProfiles(
            callback: extern "C" fn(info: CFTypeRef, user: *mut c_void) -> bool,
            seed: *mut u32,
            user: *mut c_void,
            error: *mut CFTypeRef,
        );
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFDictionaryGetValue(dict: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
        fn CFURLGetFileSystemRepresentation(url: CFTypeRef, resolve: bool, buffer: *mut u8, length: isize) -> bool;
    }

    /// One profile's URL as a path, appended to the list `user` points at; the iteration goes on.
    extern "C" fn found(info: CFTypeRef, user: *mut c_void) -> bool {
        if info.is_null() || user.is_null() {
            return true;
        }
        let paths = unsafe { &mut *user.cast::<Vec<PathBuf>>() };
        let url = unsafe { CFDictionaryGetValue(info, kColorSyncProfileURL) };
        let mut buffer = [0u8; 4096];
        if !url.is_null() && unsafe { CFURLGetFileSystemRepresentation(url, true, buffer.as_mut_ptr(), buffer.len() as isize) } {
            let end = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
            paths.push(PathBuf::from(std::ffi::OsStr::from_bytes(&buffer[..end])));
        }
        true
    }

    pub fn installed() -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = Vec::new();
        unsafe {
            ColorSyncIterateInstalledProfiles(found, std::ptr::null_mut(), (&mut paths as *mut Vec<PathBuf>).cast(), std::ptr::null_mut());
        }
        paths
    }
}

#[cfg(test)]
mod tests {
    /// On a Mac ColorSync lists at least the system's own profiles, each a file that exists.
    #[cfg(target_os = "macos")]
    #[test]
    fn colorsync_lists_profile_files_that_exist() {
        let listed = super::installed();
        println!("{} profiles from ColorSync", listed.len());
        assert!(!listed.is_empty(), "the system ships profiles");
        assert!(listed.iter().all(|p| p.is_file()), "{listed:?}");
        // Twin: the system's sRGB profile is among them by its folder.
        assert!(listed.iter().any(|p| p.starts_with("/System/Library/ColorSync/Profiles")));
    }
}
