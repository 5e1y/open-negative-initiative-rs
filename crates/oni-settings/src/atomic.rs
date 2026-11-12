//! A file written whole or not at all: a temporary beside the target, handed to the drive, then renamed.
//! The one writer of the app's files, sidecars, presets, preferences, index and exports alike.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

/// The bytes handed to the drive before the rename. A plain `fsync`: `sync_all`'s F_FULLFSYNC on macOS
/// costs nine times as much, 117 ms against 13 for a 38-frame roll's sidecars.
#[cfg(unix)]
fn flushed(file: &File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    unsafe extern "C" {
        fn fsync(fd: i32) -> i32;
    }
    // SAFETY: the descriptor belongs to `file`, open for the whole call.
    if unsafe { fsync(file.as_raw_fd()) } == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

#[cfg(not(unix))]
fn flushed(file: &File) -> io::Result<()> {
    file.sync_all()
}

/// The temporaries of the writes in flight in this process, so two writes of one file never share one.
static IN_FLIGHT: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());

fn in_flight() -> std::sync::MutexGuard<'static, BTreeSet<PathBuf>> {
    IN_FLIGHT.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// `.oni-<hash of the target's name>-<rank>.tmp` beside `path`, about 30 bytes whatever the
/// target's: a name of 220 bytes or more plus its own would pass the 255 every file system allows.
fn temporary(path: &Path, rank: usize) -> PathBuf {
    let directory = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let hash = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    directory.join(format!(".oni-{hash:016x}-{rank}.tmp"))
}

/// Whether `name` is a temporary's, whatever its target.
fn temporary_named(name: &str) -> bool {
    let Some(inner) = name.strip_prefix(".oni-").and_then(|n| n.strip_suffix(".tmp")) else { return false };
    let Some((hash, rank)) = inner.split_once('-') else { return false };
    hash.len() == 16 && hash.bytes().all(|b| b.is_ascii_hexdigit()) && !rank.is_empty() && rank.bytes().all(|b| b.is_ascii_digit())
}

/// Every temporary in `dir` no write of this process holds, as a write cut short leaves one: an
/// export reclaims its folder's, whose next write may never come under the same name.
pub fn reclaimed(dir: &Path) {
    let flying = in_flight();
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let held = flying.iter().any(|t| t.file_name() == Some(name.as_os_str()));
        if temporary_named(&name.to_string_lossy()) && !held {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// A temporary held for its write until dropped, named from its target and the lowest rank no
/// write of that target in flight holds: an interrupted write's leftover is taken by the next one.
pub struct Staged(PathBuf);

impl Staged {
    fn held(path: &Path) -> Staged {
        let mut flying = in_flight();
        let free = (0..).map(|rank| temporary(path, rank)).find(|t| !flying.contains(t)).unwrap_or_default();
        flying.insert(free.clone());
        Staged(free)
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        in_flight().remove(&self.0);
    }
}

impl std::ops::Deref for Staged {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Staged {
    fn as_ref(&self) -> &Path {
        &self.0
    }
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

/// `write` into a temporary beside `path`, owner-only on Unix when `private`, handed to the drive
/// when `flush`; the temporary removed on any failure.
fn staged_with(path: &Path, private: bool, flush: bool, write: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<Staged> {
    let temporary = Staged::held(path);
    // A leftover goes first, so neither its mode nor a link at its name reaches the new file. Another
    // ONI process writing the same file at the same instant can take the same name: a known limit.
    let _ = std::fs::remove_file(&temporary);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if private {
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    }
    #[cfg(not(unix))]
    let _ = private;
    let result = options.open(&temporary).and_then(|mut file| {
        write(&mut file)?;
        if flush { flushed(&file) } else { Ok(()) }
    });
    match result {
        Ok(()) => Ok(temporary),
        Err(e) => {
            let _ = std::fs::remove_file(&temporary);
            Err(e)
        }
    }
}

/// A staged temporary renamed over `path`, or removed when the rename is refused.
fn landed(path: &Path, staged: io::Result<Staged>, rename: &dyn Fn(&Path, &Path) -> io::Result<()>) -> io::Result<()> {
    let temporary = staged?;
    rename(&temporary, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temporary);
    })
}

/// `path` written whole by `write`, handed to the drive, then renamed: a reader never sees half a file.
pub fn write_with(path: &Path, write: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<()> {
    landed(path, staged_with(path, false, true, write), &renamed)
}

/// `write_with` for bytes in hand.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    written(path, bytes, &renamed)
}

/// `write_atomically` through its own rename, which a check makes fail.
pub fn written(path: &Path, bytes: &[u8], rename: &dyn Fn(&Path, &Path) -> io::Result<()>) -> io::Result<()> {
    landed(path, staged_with(path, false, true, |file| file.write_all(bytes)), rename)
}

/// `write_atomically` readable by its owner alone on Unix; on Windows the support folder sits in
/// the user's profile, whose ACL already keeps others out.
pub fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    landed(path, staged_with(path, true, true, |file| file.write_all(bytes)), &renamed)
}

/// The bytes written and handed to the drive beside `path` under a hidden name no archive lists,
/// for a caller that lands them by its own rule while it holds them.
pub fn staged(path: &Path, bytes: &[u8]) -> io::Result<Staged> {
    staged_with(path, false, true, |file| file.write_all(bytes))
}

/// `write_with` without handing the bytes to the drive, for a file made again when lost.
pub fn cached_with(path: &Path, write: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<()> {
    landed(path, staged_with(path, false, false, write), &renamed)
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
        assert!(temporary(&long, 0).file_name().unwrap().len() < 60);
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

    #[test]
    fn a_writer_that_fails_leaves_the_old_file_and_no_temporary() {
        let dir = std::env::temp_dir().join(format!("oni-atomic-writer-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("patch.tiff");
        std::fs::write(&target, b"old").unwrap();
        for land in [write_with as fn(&Path, fn(&mut File) -> io::Result<()>) -> io::Result<()>, cached_with] {
            assert!(land(&target, |f| f.write_all(b"half").and(Err(io::Error::other("stopped")))).is_err());
            assert_eq!(std::fs::read(&target).unwrap(), b"old");
            assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "the temporary removed");
            // Twin: the same writer finishing lands its bytes whole.
            land(&target, |f| f.write_all(b"new")).unwrap();
            assert_eq!(std::fs::read(&target).unwrap(), b"new");
            std::fs::write(&target, b"old").unwrap();
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The names in `dir`, sorted.
    fn listing(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    /// Set in a child of the check below: the target of a write it quits midway.
    const QUITTING: &str = "ONI_ATOMIC_QUITTING";

    #[test]
    fn a_write_cut_short_by_a_quit_leaves_nothing_once_its_file_is_written_again() {
        if let Ok(target) = std::env::var(QUITTING) {
            // Half the bytes on the drive, then gone without unwinding, as a crash or a force-quit.
            let _ = write_with(Path::new(&target), |file| {
                file.write_all(b"half")?;
                std::process::exit(0)
            });
            return;
        }
        let dir = std::env::temp_dir().join(format!("oni-atomic-quit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (key, list) = (dir.join("host.key"), dir.join("trusted.txt"));
        let quit = |target: &Path| {
            let test = "atomic::tests::a_write_cut_short_by_a_quit_leaves_nothing_once_its_file_is_written_again";
            let child = std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact", test]).env(QUITTING, target).output().unwrap();
            assert!(child.status.success(), "{child:?}");
        };
        quit(&key);
        quit(&key);
        quit(&list);
        assert_eq!(listing(&dir).len(), 2, "one leftover a file, however many quits: {:?}", listing(&dir));
        write_private(&key, b"secret").unwrap();
        let left = listing(&dir);
        assert_eq!(left.len(), 2, "the key's leftover gone: {left:?}");
        // Twin: another file's leftover is not the key's, and stays until that file is written.
        assert!(left.contains(&"host.key".to_string()) && left.iter().any(|n| n.ends_with(".tmp")), "{left:?}");
        write_atomically(&list, b"").unwrap();
        assert_eq!(listing(&dir), ["host.key", "trusted.txt"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_write_in_flight_keeps_its_temporary_while_its_file_is_written_again() {
        let dir = std::env::temp_dir().join(format!("oni-atomic-flight-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("Portra.json");
        let first = staged(&target, b"first").unwrap();
        write_atomically(&target, b"second").unwrap();
        renamed(&first, &target).unwrap();
        drop(first);
        assert_eq!((std::fs::read(&target).unwrap(), listing(&dir)), (b"first".to_vec(), vec!["Portra.json".to_string()]));
        // Twin: a write let go before its rename, its rank free again, loses its temporary to the next write.
        let third = staged(&target, b"third").unwrap().to_path_buf();
        write_atomically(&target, b"first").unwrap();
        assert!(renamed(&third, &target).is_err());
        assert_eq!((std::fs::read(&target).unwrap(), listing(&dir)), (b"first".to_vec(), vec!["Portra.json".to_string()]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_folder_reclaimed_loses_every_leftover_and_keeps_the_writes_in_flight() {
        let dir = std::env::temp_dir().join(format!("oni-atomic-reclaimed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let flying = staged(&dir.join("P1.tif"), b"in flight").unwrap();
        let left = staged(&dir.join("P2.tif"), b"cut").unwrap().to_path_buf();
        for kept in ["P3.tif", ".oni-roll.json", ".oni-0123-0.tmp", ".oni-0123456789abcdef-.tmp"] {
            std::fs::write(dir.join(kept), b"").unwrap();
        }
        assert!(left.exists());
        reclaimed(&dir);
        let names = listing(&dir);
        assert!(!left.exists() && flying.exists(), "{names:?}");
        // Twin: nothing but a temporary's name goes, whatever it opens on.
        assert_eq!(names.len(), 5, "{names:?}");
        drop(flying);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_private_file_is_readable_by_its_owner_alone() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("oni-atomic-private-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (key, plain) = (dir.join("host.key"), dir.join("plain.json"));
        write_private(&key, b"secret").unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!((std::fs::read(&key).unwrap(), mode(&key)), (b"secret".to_vec(), 0o600));
        // Twin: a plain write takes the mask's mode as any new file does, open to others under the usual one.
        write_atomically(&plain, b"{}").unwrap();
        let reference = dir.join("reference");
        File::create(&reference).unwrap();
        assert_eq!(mode(&plain), mode(&reference));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
