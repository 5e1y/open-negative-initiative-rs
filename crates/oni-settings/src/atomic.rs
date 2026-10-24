//! A file written whole or not at all: a temporary beside the target, handed to the drive, then renamed.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
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

/// A hidden name beside `path` for its temporary, about 45 bytes whatever the target's: a name of
/// 220 bytes or more plus its own would pass the 255 every file system allows.
pub fn temporary(path: &Path) -> PathBuf {
    let directory = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let hash = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() % 1_000_000_000_000).unwrap_or(0);
    directory.join(format!(".oni-{hash:016x}-{}-{stamp}.tmp", std::process::id()))
}

/// A temporary file beside the target, then a rename: a reader never sees half a document.
pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = temporary(path);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_name_writes_through_a_short_temporary() {
        let dir = std::env::temp_dir().join(format!("oni-atomic-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // 240 characters with `.json`: within the limit, though its name and a temporary's suffix are not.
        let long = dir.join(format!("{}.NEF.json", "a".repeat(231)));
        assert_eq!(long.file_name().unwrap().len(), 240);
        write_atomically(&long, b"{}").unwrap();
        assert_eq!(std::fs::read(&long).unwrap(), b"{}");
        assert!(temporary(&long).file_name().unwrap().len() < 60);
        // Twin: the former temporary's name, the target's own plus 37 bytes, is refused by the disk.
        let former = dir.join(format!(".{}.{}.1761400000000000000.tmp", long.file_name().unwrap().to_string_lossy(), std::process::id()));
        assert!(std::fs::File::create(&former).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
