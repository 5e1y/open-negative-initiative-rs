//! What the Finder knows of a folder and the system answers: whether it is a package, a folder
//! shown as one file whose contents belong to its app, and where a Finder alias leads.

use std::path::{Path, PathBuf};

/// Directories the Finder shows as one file, by extension: the answer where the system gives none
/// (Windows, or a query refused). On macOS the system's own answer wins, which knows each installed app.
pub const PACKAGE_EXTENSIONS: &[&str] = &[
    "app",
    "bundle",
    "framework",
    "plugin",
    "kext",
    "photoslibrary",
    "photolibrary",
    "migratedphotolibrary",
    "aplibrary",
    "fcpbundle",
    "imovielibrary",
    "tvlibrary",
    "musiclibrary",
    "theater",
    "rtfd",
    "pages",
    "numbers",
    "key",
    "xcodeproj",
    "xcworkspace",
    "playground",
    "logicx",
    "band",
    "lrdata",
    "lrlibrary",
];

fn by_extension(path: &Path) -> bool {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|e| PACKAGE_EXTENSIONS.contains(&e.as_str()))
}

/// `URLResourceValues.isPackage`: a declared package extension or the bundle bit, as the Photo
/// Booth library carries with no extension. Reads the disk on macOS.
pub fn is_package(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    if let Some(answer) = macos::is_package(path) {
        return answer;
    }
    by_extension(path)
}

/// The package holding `path`, itself included, nearest first; `None` when no folder above is one.
pub fn package_holding(path: &Path) -> Option<PathBuf> {
    path.ancestors().filter(|a| a.parent().is_some()).find(|a| is_package(a)).map(Path::to_path_buf)
}

/// Where a Finder alias file leads, `None` for any other file or one that no longer resolves.
/// Never mounts a disk nor asks a question to resolve it.
pub fn alias_target(path: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    return macos::alias_target(path);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        None
    }
}

#[cfg(target_os = "macos")]
pub(crate) mod macos {
    use std::ffi::c_void;
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::ptr::{null, null_mut};

    type Ref = *const c_void;
    /// `kCFURLBookmarkResolutionWithoutUIMask | WithoutMountingMask`.
    const QUIET: usize = (1 << 8) | (1 << 9);
    /// `kIsAlias` in the Finder flags, bytes 8 and 9 of `com.apple.FinderInfo`.
    const IS_ALIAS: u16 = 0x8000;

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFURLIsPackageKey: Ref;
        static kCFBooleanTrue: Ref;
        fn CFURLCreateFromFileSystemRepresentation(alloc: Ref, buffer: *const u8, len: isize, directory: u8) -> Ref;
        fn CFURLCopyResourcePropertyForKey(url: Ref, key: Ref, value: *mut Ref, error: *mut Ref) -> u8;
        fn CFURLCreateBookmarkDataFromFile(alloc: Ref, file: Ref, error: *mut Ref) -> Ref;
        fn CFURLCreateByResolvingBookmarkData(alloc: Ref, data: Ref, options: usize, relative: Ref, keys: Ref, stale: *mut u8, error: *mut Ref) -> Ref;
        fn CFURLGetFileSystemRepresentation(url: Ref, resolve: u8, buffer: *mut u8, len: isize) -> u8;
        fn CFURLCreateBookmarkData(alloc: Ref, url: Ref, options: usize, keys: Ref, relative: Ref, error: *mut Ref) -> Ref;
        fn CFURLWriteBookmarkDataToFile(data: Ref, file: Ref, options: usize, error: *mut Ref) -> u8;
        fn CFRelease(cf: Ref);
    }

    /// A URL the caller releases, `None` for a path CoreFoundation refuses.
    fn url(path: &Path, directory: bool) -> Option<Ref> {
        let bytes = path.as_os_str().as_bytes();
        // SAFETY: the bytes outlive the call, which copies them.
        let url = unsafe { CFURLCreateFromFileSystemRepresentation(null(), bytes.as_ptr(), bytes.len() as isize, u8::from(directory)) };
        (!url.is_null()).then_some(url)
    }

    fn released<T>(object: Ref, then: impl FnOnce(Ref) -> T) -> T {
        let out = then(object);
        // SAFETY: `object` came from a Create or Copy call, so it is this caller's to release.
        unsafe { CFRelease(object) };
        out
    }

    pub fn is_package(path: &Path) -> Option<bool> {
        released(url(path, true)?, |url| {
            let mut value: Ref = null();
            // SAFETY: `url` is live, `value` receives a retained boolean or stays null.
            let ok = unsafe { CFURLCopyResourcePropertyForKey(url, kCFURLIsPackageKey, &mut value, null_mut()) };
            if ok == 0 || value.is_null() {
                return None;
            }
            // SAFETY: the booleans are the framework's two constants.
            Some(released(value, |v| v == unsafe { kCFBooleanTrue }))
        })
    }

    /// The Finder's alias flag, one attribute read: no other file pays a bookmark's resolution.
    fn flagged(path: &Path) -> bool {
        let Ok(name) = std::ffi::CString::new(path.as_os_str().as_bytes()) else { return false };
        let mut info = [0u8; 32];
        // SAFETY: `name` is a valid C string and `info` holds the 32 bytes asked for.
        let read = unsafe {
            libc::getxattr(name.as_ptr(), c"com.apple.FinderInfo".as_ptr(), info.as_mut_ptr().cast(), info.len(), 0, libc::XATTR_NOFOLLOW)
        };
        read == 32 && u16::from_be_bytes([info[8], info[9]]) & IS_ALIAS != 0
    }

    pub fn alias_target(path: &Path) -> Option<PathBuf> {
        if !flagged(path) {
            return None;
        }
        let data = released(url(path, false)?, |file| {
            // SAFETY: `file` is live; a null answer is a file with no bookmark.
            let data = unsafe { CFURLCreateBookmarkDataFromFile(null(), file, null_mut()) };
            (!data.is_null()).then_some(data)
        })?;
        let target = released(data, |data| {
            let mut stale = 0u8;
            // SAFETY: `data` is live; the options forbid any question or mount.
            let target = unsafe { CFURLCreateByResolvingBookmarkData(null(), data, QUIET, null(), null(), &mut stale, null_mut()) };
            (!target.is_null()).then_some(target)
        })?;
        released(target, |target| {
            let mut buffer = vec![0u8; 4096];
            // SAFETY: `buffer` holds the length given.
            let ok = unsafe { CFURLGetFileSystemRepresentation(target, 1, buffer.as_mut_ptr(), buffer.len() as isize) };
            let end = buffer.iter().position(|&b| b == 0)?;
            (ok != 0).then(|| PathBuf::from(std::ffi::OsStr::from_bytes(&buffer[..end])))
        })
    }

    /// A Finder alias to `target` written at `alias`, as Make Alias writes one: for the checks.
    pub fn make_alias(target: &Path, alias: &Path) -> bool {
        let Some(data) = url(target, true).map(|t| {
            // SAFETY: `t` is live; 1 << 10 is `kCFURLBookmarkCreationSuitableForBookmarkFile`.
            released(t, |t| unsafe { CFURLCreateBookmarkData(null(), t, 1 << 10, null(), null(), null_mut()) })
        }) else {
            return false;
        };
        if data.is_null() {
            return false;
        }
        released(data, |data| {
            // SAFETY: `data` and the file's URL are live for the call.
            url(alias, false).is_some_and(|file| released(file, |file| unsafe { CFURLWriteBookmarkDataToFile(data, file, 0, null_mut()) } != 0))
        })
    }
}
