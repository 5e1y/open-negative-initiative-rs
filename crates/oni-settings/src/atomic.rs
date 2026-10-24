//! A file written whole or not at all: a temporary beside the target, handed to the drive, then renamed.

use std::io::{self, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// The bytes handed to the drive before the rename. A plain `fsync`: `sync_all`'s F_FULLFSYNC on macOS
/// costs nine times as much, 117 ms against 13 for a 38-frame roll's sidecars.
#[cfg(unix)]
fn flushed(file: &std::fs::File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    unsafe extern "C" {
        fn fsync(fd: i32) -> i32;
    }
    // SAFETY: the descriptor belongs to `file`, open for the whole call.
    if unsafe { fsync(file.as_raw_fd()) } == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

#[cfg(not(unix))]
fn flushed(file: &std::fs::File) -> io::Result<()> {
    file.sync_all()
}

/// A temporary file beside the target, then a rename: a reader never sees half a document.
pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let directory = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let temporary = directory.join(format!(".{name}.{}.{stamp}.tmp", std::process::id()));
    let result = (|| {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        flushed(&file)?;
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}
