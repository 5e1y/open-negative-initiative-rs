//! A file written whole or not at all: a temporary beside the target, handed to the drive, then renamed.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

/// The pauses between tries, about half a second in all: an indexer, a cloud client or an
/// antivirus lets a file go within that, and a hold any longer is said instead.
pub const PAUSES: [Duration; 4] = [Duration::from_millis(30), Duration::from_millis(60), Duration::from_millis(120), Duration::from_millis(240)];

/// Whether a refused rename may pass once another program lets go: busy anywhere; on Windows a
/// sharing or lock violation (32, 33) or an access denied (5), as a file held open leaves it.
pub fn held(error: &io::Error, windows: bool) -> bool {
    error.kind() == io::ErrorKind::ResourceBusy || (windows && matches!(error.raw_os_error(), Some(5 | 32 | 33)))
}

/// `rename` tried again after each pause while the file is held; any other refusal, or the last
/// one, is returned at once.
pub fn retried(from: &Path, to: &Path, rename: &dyn Fn(&Path, &Path) -> io::Result<()>, pause: &dyn Fn(Duration), windows: bool) -> io::Result<()> {
    let mut pauses = PAUSES.iter();
    loop {
        match rename(from, to) {
            Err(e) if held(&e, windows) => match pauses.next() {
                Some(wait) => pause(*wait),
                None => return Err(e),
            },
            done => return done,
        }
    }
}

/// `std::fs::rename` through `retried`, on this system's rules.
pub fn renamed(from: &Path, to: &Path) -> io::Result<()> {
    retried(from, to, &|f, t| std::fs::rename(f, t), &std::thread::sleep, cfg!(windows))
}

/// A temporary file beside the target, then a rename: a reader never sees half a document.
pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    written(path, bytes, &renamed)
}

/// `write_atomically` through its own rename, which a check makes fail.
fn written(path: &Path, bytes: &[u8], rename: &dyn Fn(&Path, &Path) -> io::Result<()>) -> io::Result<()> {
    let temporary = temporary(path);
    let result = (|| {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        flushed(&file)?;
        rename(&temporary, path)
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

    /// A rename refused `refusals` times with `code`, as a file another program holds is on
    /// Windows, then done; the pauses it waited counted in `paused`.
    fn held_for(refusals: usize, code: i32) -> (impl Fn(&Path, &Path) -> io::Result<()>, std::rc::Rc<std::cell::Cell<usize>>) {
        let tries = std::rc::Rc::new(std::cell::Cell::new(0));
        let counted = tries.clone();
        let rename = move |from: &Path, to: &Path| {
            counted.set(counted.get() + 1);
            if counted.get() <= refusals { Err(io::Error::from_raw_os_error(code)) } else { std::fs::rename(from, to) }
        };
        (rename, tries)
    }

    #[test]
    fn a_file_held_for_a_moment_on_windows_is_written_once_it_is_let_go() {
        let dir = std::env::temp_dir().join(format!("oni-atomic-held-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("A.NEF.json");
        std::fs::write(&target, b"old").unwrap();
        let paused = std::cell::Cell::new(Duration::ZERO);
        let pause = |d: Duration| paused.set(paused.get() + d);
        for code in [5, 32, 33] {
            let (rename, tries) = held_for(3, code);
            written(&target, b"new", &|f, t| retried(f, t, &rename, &pause, true)).unwrap();
            assert_eq!((std::fs::read(&target).unwrap(), tries.get()), (b"new".to_vec(), 4), "error {code}");
            std::fs::write(&target, b"old").unwrap();
        }
        assert_eq!(paused.get(), 3 * (PAUSES[0] + PAUSES[1] + PAUSES[2]));
        // Twin: held past every pause, or a refusal of another kind, fails and leaves no temporary.
        let (rename, tries) = held_for(PAUSES.len() + 1, 32);
        assert!(written(&target, b"new", &|f, t| retried(f, t, &rename, &pause, true)).is_err());
        assert_eq!(tries.get(), PAUSES.len() + 1);
        let (rename, tries) = held_for(1, 2);
        assert!(written(&target, b"new", &|f, t| retried(f, t, &rename, &pause, true)).is_err());
        assert_eq!(tries.get(), 1, "a missing folder is no hold");
        // Twin: the same violation off Windows is no hold either.
        let (rename, tries) = held_for(1, 32);
        assert!(written(&target, b"new", &|f, t| retried(f, t, &rename, &pause, false)).is_err());
        assert_eq!(tries.get(), 1);
        assert_eq!(std::fs::read(&target).unwrap(), b"old");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "every temporary removed");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
